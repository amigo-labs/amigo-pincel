//! wasm-bindgen bindings exposing `pincel-image-core` to JavaScript (spec §17).
//!
//! Document state is owned in Rust: handles are opaque indices into a
//! thread-local arena, and JS only ever receives composited pixel buffers and
//! exported bytes (ADR-001). Commands are passed as JSON strings.

use pincel_effects::EffectSpec;
use pincel_image_core::codec::psd::{PsdLimits, PsdNotice, from_psd_bytes};
use pincel_image_core::codec::{to_ico_bytes, to_png_bytes, to_tga_bytes};
use pincel_image_core::command::live_spec::{
    TextLayoutRequest, live_object_spec, text_layout as layout_text,
};
use pincel_image_core::command::spec::{
    CommandSpec, StrokeScratch, layer_locked, parse_sample, run_command,
};
use pincel_image_core::command::{CommandBus, HistoryKind, PasteLayer, SetPixels};
use pincel_image_core::jobs::{self, EncodeFormat, JobError, decode_region, encode_region};
use pincel_image_core::{
    BlendMode, Color, Document, Eyedropper, ImageBuffer, Point, Rect, RenderCache, SampleSize,
    compose, copy_selection as copy_selection_pixels, delete_selection,
};
use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use wasm_bindgen::Clamped;
use wasm_bindgen::prelude::*;

mod fonts;
mod history;
mod palette;
mod vectorize;

#[cfg(test)]
mod rotate_zoom_tests;

pub use palette::{PaletteFile, read_palette_file, write_palette_file};
pub use vectorize::{VectorizeResult, Vectorizer, vectorizer};

thread_local! {
    /// Open documents by handle. Handles are never reused, so a stale handle
    /// held by JS errors instead of silently aliasing a newer document.
    static DOCUMENTS: RefCell<HashMap<u32, CommandBus>> = RefCell::new(HashMap::new());
    /// Next handle to hand out; monotonically increasing.
    static NEXT_HANDLE: Cell<u32> = const { Cell::new(0) };
    /// What the brush tools carry across the commands of one pointer drag.
    /// A drag arrives as one command per pointer event — merged into a
    /// single undo step by `stroke_id` — so a tool that remembers something
    /// between dabs has to remember it here, not inside one call.
    static STROKES: RefCell<HashMap<u32, StrokeScratch>> = RefCell::new(HashMap::new());
    /// The presented composite of each document (docs/specs/performance-image-cache.md),
    /// created on its first `composite_region` and fed the bus's damage on
    /// every one after.
    static CACHES: RefCell<HashMap<u32, RenderCache>> = RefCell::new(HashMap::new());
}

/// Installs a panic hook that logs Rust panics to the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Registers a TrueType/OpenType font and returns its id for use in `DrawText`
/// commands (spec §9.2 Text; ADR-012, Option B — the caller supplies the font).
///
/// `face_index` names a face inside a font collection (`.ttc` / `.otc`) and
/// defaults to 0 (docs/specs/system-fonts.md AC-08). Re-registering
/// byte-identical data with the same index returns the existing id, so
/// repeated setup (e.g. per document open) does not accumulate copies.
#[wasm_bindgen]
pub fn register_font(data: &[u8], face_index: Option<u32>) -> u32 {
    fonts::register(data, face_index.unwrap_or(0))
}

/// Inserts a bus into the arena and returns its (never reused) handle.
fn insert(bus: CommandBus) -> u32 {
    let handle = NEXT_HANDLE.with(|next| {
        let h = next.get();
        next.set(h + 1);
        h
    });
    DOCUMENTS.with(|docs| docs.borrow_mut().insert(handle, bus));
    handle
}
fn with_bus<T>(
    handle: u32,
    f: impl FnOnce(&mut CommandBus) -> Result<T, JsError>,
) -> Result<T, JsError> {
    DOCUMENTS.with(|docs| {
        let mut docs = docs.borrow_mut();
        match docs.get_mut(&handle) {
            Some(bus) => f(bus),
            None => Err(JsError::new("invalid document handle")),
        }
    })
}

/// Creates a new blank document of `width` × `height`. Returns its handle.
#[wasm_bindgen]
pub fn create_document(width: u32, height: u32) -> Result<u32, JsError> {
    let doc = Document::new(width, height).map_err(to_js)?;
    Ok(insert(CommandBus::new(doc)))
}

/// Opens an encoded image (PNG / JPEG / WebP / BMP / GIF / TIFF / TGA / DDS /
/// ICO) as a single-layer document. Formats are detected by content;
/// `mime_type` (a MIME type or a file name) is the hint TGA needs, since it
/// has no signature.
#[wasm_bindgen]
pub fn open_image(data: &[u8], mime_type: &str) -> Result<u32, JsError> {
    // The MIME type (or file name) is the hint for TGA, which has no
    // signature (docs/specs/image-formats-more.md AC-04).
    let buffer = pincel_image_core::codec::decode_hinted(data, Some(mime_type))
        .map_err(|e| JsError::new(&e.to_string()))?;
    let doc = Document::from_pixels(buffer).map_err(to_js)?;
    Ok(insert(CommandBus::new(doc)))
}

/// Opens an OpenRaster (`.ora`) project — the Image mode's layered save
/// format (spec §15, 2026-10-11) — and returns its handle.
#[wasm_bindgen]
pub fn open_project(data: &[u8]) -> Result<u32, JsError> {
    let doc =
        pincel_image_core::codec::from_ora_bytes(data).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(insert(CommandBus::new(doc)))
}

/// An uncompressed copy of the whole document for another wasm instance —
/// the job worker that encodes autosaves and exports
/// (docs/specs/performance-background-jobs.md). [`open_snapshot`] turns it
/// back into a document.
#[wasm_bindgen]
pub fn snapshot_document(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        Ok(pincel_image_core::codec::snapshot::to_snapshot_bytes(
            &bus.document,
        ))
    })
}

/// Opens bytes from [`snapshot_document`] as a new document and returns
/// its handle; the copy equals the document the snapshot was taken of.
#[wasm_bindgen]
pub fn open_snapshot(data: &[u8]) -> Result<u32, JsError> {
    let doc = pincel_image_core::codec::snapshot::from_snapshot_bytes(data).map_err(to_js)?;
    Ok(insert(CommandBus::new(doc)))
}

/// What [`open_psd`] hands back: the new document's handle and what the
/// import left out or changed (docs/specs/psd-import.md).
#[derive(Debug, Serialize)]
struct PsdOpened {
    handle: u32,
    notices: Vec<PsdNotice>,
}

/// Opens a PSD into the arena, refusing a canvas over `max_dim` per edge
/// or `max_area` pixels before decoding any pixel.
fn open_psd_document(data: &[u8], max_dim: u32, max_area: u32) -> Result<PsdOpened, String> {
    let limits = PsdLimits {
        max_dim,
        max_area: u64::from(max_area),
    };
    let import = from_psd_bytes(data, limits).map_err(|e| e.to_string())?;
    Ok(PsdOpened {
        handle: insert(CommandBus::new(import.document)),
        notices: import.notices,
    })
}

/// Opens a Photoshop (`.psd`) file — 8-bit RGB, its layers and groups, or
/// its flattened composite — and returns `{ handle, notices }`; `notices`
/// lists what the import left out or changed (`kind`-tagged objects, see
/// `PsdNotice`). Fails, opening nothing, for PSB, other depths and color
/// modes, a canvas over `max_dim` per edge or `max_area` pixels, or a file
/// that cannot be read (docs/specs/psd-import.md).
#[wasm_bindgen]
pub fn open_psd(data: &[u8], max_dim: u32, max_area: u32) -> Result<JsValue, JsError> {
    let opened = open_psd_document(data, max_dim, max_area).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&opened).map_err(|e| JsError::new(&e.to_string()))
}

/// Pastes an encoded image (PNG/JPEG/WebP/BMP/GIF/TIFF) as a new layer above
/// the active one, its top-left at canvas `(x, y)` (spec §5.4). With
/// `expand`, the canvas first grows (anchored top-left) to cover the image
/// there; otherwise the image is cropped. One undo step.
#[wasm_bindgen]
pub fn paste_layer(handle: u32, data: &[u8], x: i32, y: i32, expand: bool) -> Result<(), JsError> {
    let buffer =
        pincel_image_core::codec::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
    with_bus(handle, |bus| {
        bus.apply(Box::new(PasteLayer::at(buffer, x, y, expand)))
            .map_err(to_js)
    })
}

/// Copy (spec §5.4): the active layer — with `merged`, the composite of the
/// visible layers — cropped to the selection's bounding box (the whole
/// canvas without a selection), alpha scaled by the selection's coverage,
/// as PNG bytes. The document is unchanged; the copy's canvas origin is
/// `get_selection_bounds`. Errors when the selection is empty.
#[wasm_bindgen]
pub fn copy_selection(handle: u32, merged: bool) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| copy_png(&bus.document, merged))
}

/// Cut (spec §5.4): [`copy_selection`] of the active layer, then its
/// selected pixels (the whole layer without a selection) are cleared in one
/// undo step. Returns the PNG bytes.
#[wasm_bindgen]
pub fn cut_selection(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        let png = copy_png(&bus.document, false)?;
        let layer = bus.document.active_layer_index();
        if let Some(cmd) = delete_selection(layer, &bus.document) {
            bus.apply(Box::new(cmd)).map_err(to_js)?;
        }
        Ok(png)
    })
}

/// The outline polygon (`x, y` pairs) of the box shape `kind` (`triangle`,
/// `diamond`, `star`, `block_arrow`, `heart`, `callout`) over the drag from
/// `(x0, y0)` to `(x1, y1)`, exactly as the core draws it — for the editor's
/// drag preview (docs/specs/image-shapes-curves-shapes.md AC-12). Empty for
/// another kind or a box without area.
#[wasm_bindgen]
pub fn shape_outline(
    kind: &str,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    star_points: u32,
    inner_ratio: f32,
) -> Vec<f32> {
    let Some(kind) = pincel_image_core::command::live_spec::parse_live_shape(kind) else {
        return Vec::new();
    };
    let shape = pincel_image_core::LiveShape {
        kind,
        a: pincel_image_core::Point::new(x0, y0),
        b: pincel_image_core::Point::new(x1, y1),
        style: pincel_image_core::ShapeStyle::default(),
        corner_radius: 0.0,
        sides: 5,
        star_points,
        inner_ratio,
        control: Vec::new(),
        start_cap: pincel_image_core::LineCap::Round,
        end_cap: pincel_image_core::LineCap::Round,
    }
    .shape();
    pincel_image_core::box_outline(&shape)
        .map(|pts| pts.iter().flat_map(|p| [p.x, p.y]).collect())
        .unwrap_or_default()
}

/// Clipboard identity of an encoded image: `"{w}x{h}:{hash}"` of its decoded
/// pixels, hashed premultiplied so a clipboard that re-encodes the image
/// (Chromium does) keeps the key. Tells Pincel's own copy apart.
#[wasm_bindgen]
pub fn image_key(data: &[u8]) -> Result<String, JsError> {
    let buffer =
        pincel_image_core::codec::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(rgba_key(buffer.width(), buffer.height(), buffer.data()))
}

fn copy_png(doc: &Document, merged: bool) -> Result<Vec<u8>, JsError> {
    let (_, pixels) =
        copy_selection_pixels(doc, merged).ok_or_else(|| JsError::new("nothing is selected"))?;
    to_png_bytes(&pixels, 1).map_err(|e| JsError::new(&e.to_string()))
}

/// FNV-1a over premultiplied RGBA8, prefixed with the size. Must match
/// `pincel-wasm`'s `imageKey`.
fn rgba_key(width: u32, height: u32, rgba: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |b: u8| {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    for px in rgba.as_chunks::<4>().0 {
        let a = u32::from(px[3]);
        for &c in &px[..3] {
            feed(((u32::from(c) * a + 127) / 255) as u8);
        }
        feed(px[3]);
    }
    format!("{width}x{height}:{hash:016x}")
}

/// Serializes the document as an OpenRaster (`.ora`) project with every
/// layer, its opacity, blend mode, visibility and lock. `compression` is
/// the 0–9 PNG level for the layer images (autosave uses a fast one).
#[wasm_bindgen]
pub fn save_project(handle: u32, compression: u8) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        jobs::encode(&bus.document, EncodeFormat::Ora, compression, 0).map_err(job_js)
    })
}

/// Releases the document for `handle`. The handle is never reused.
#[wasm_bindgen]
pub fn close_document(handle: u32) {
    DOCUMENTS.with(|docs| {
        docs.borrow_mut().remove(&handle);
    });
    STROKES.with(|strokes| {
        strokes.borrow_mut().remove(&handle);
    });
    CACHES.with(|caches| {
        caches.borrow_mut().remove(&handle);
    });
}

/// Number of documents open in the core. A diagnostic: tests check that
/// closing a tab frees its document (docs/specs/document-tabs.md AC-16).
#[wasm_bindgen]
pub fn document_count() -> u32 {
    DOCUMENTS.with(|docs| u32::try_from(docs.borrow().len()).unwrap_or(u32::MAX))
}

/// Returns the flattened composite as an RGBA8 `Uint8ClampedArray`, ready to
/// wrap in an `ImageData` (spec §17).
#[wasm_bindgen]
pub fn composite(handle: u32) -> Result<Clamped<Vec<u8>>, JsError> {
    with_bus(handle, |bus| {
        Ok(Clamped(compose(bus.document.layers()).into_raw()))
    })
}

/// Maps a [`BlendMode`] to its stable snake_case string for the JS layer.
fn blend_mode_str(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "normal",
        BlendMode::Multiply => "multiply",
        BlendMode::Screen => "screen",
        BlendMode::Overlay => "overlay",
        BlendMode::Darken => "darken",
        BlendMode::Lighten => "lighten",
        BlendMode::ColorDodge => "color_dodge",
        BlendMode::ColorBurn => "color_burn",
        BlendMode::HardLight => "hard_light",
        BlendMode::SoftLight => "soft_light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
    }
}

/// Edge length of layer thumbnails (spec §5.3).
const THUMBNAIL_DIM: u32 = 32;

/// Downscales a layer buffer to a 32×32 RGBA8 thumbnail by nearest-neighbor
/// sampling (spec §5.3).
fn thumbnail(src: &ImageBuffer) -> Vec<u8> {
    let mut out = vec![0u8; (THUMBNAIL_DIM * THUMBNAIL_DIM * 4) as usize];
    let sw = src.width().max(1);
    let sh = src.height().max(1);
    for ty in 0..THUMBNAIL_DIM {
        for tx in 0..THUMBNAIL_DIM {
            let sx = (tx * sw / THUMBNAIL_DIM).min(sw - 1);
            let sy = (ty * sh / THUMBNAIL_DIM).min(sh - 1);
            let c = src.get_pixel(sx, sy).unwrap_or(Color::TRANSPARENT);
            let i = ((ty * THUMBNAIL_DIM + tx) * 4) as usize;
            out[i] = c.r;
            out[i + 1] = c.g;
            out[i + 2] = c.b;
            out[i + 3] = c.a;
        }
    }
    out
}

/// Maps an averaging edge length to a [`SampleSize`], defaulting to 1×1.
fn sample_size(edge: u32) -> SampleSize {
    match edge {
        3 => SampleSize::ThreeByThree,
        5 => SampleSize::FiveByFive,
        11 => SampleSize::ElevenByEleven,
        31 => SampleSize::ThirtyOneByThirtyOne,
        _ => SampleSize::One,
    }
}
/// Applies a JSON-encoded command to the document and records it in history.
#[wasm_bindgen]
pub fn apply_command(handle: u32, command: &str) -> Result<(), JsError> {
    let spec: CommandSpec =
        serde_json::from_str(command).map_err(|e| JsError::new(&e.to_string()))?;
    with_bus(handle, |bus| {
        if spec.refused(&bus.document) {
            return Err(JsError::new(LOCKED));
        }
        STROKES.with(|strokes| {
            let mut strokes = strokes.borrow_mut();
            let scratch = strokes.entry(handle).or_default();
            fonts::with_faces(|faces| run_command(bus, spec, scratch, faces).map_err(to_js))
        })
    })
}

/// Error message for an edit aimed at a locked layer (spec §3.2).
const LOCKED: &str = "layer is locked";

/// Samples a color at canvas point `(x, y)` (the Eyedropper tool).
///
/// `sample` is `"current_layer"` or `"all_layers"`; `size` is the averaging
/// edge length (1, 3, 5, 11, or 31). Returns the 4 RGBA bytes, or an empty
/// array if the point lies off the canvas. Sampling is not undoable, so this is
/// a query, not a command.
#[wasm_bindgen]
pub fn pick_color(
    handle: u32,
    x: f32,
    y: f32,
    sample: &str,
    size: u32,
) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        let eyedropper = Eyedropper::new(parse_sample(sample), sample_size(size));
        match eyedropper.pick(Point::new(x, y), &bus.document) {
            Some(c) => Ok(vec![c.r, c.g, c.b, c.a]),
            None => Ok(Vec::new()),
        }
    })
}

/// Selects the active layer by `index`. Layer selection is UI state, not an
/// undoable edit, so this is a setter rather than a command (spec §5, §7.3).
/// A change to another layer commits the live object first
/// (docs/specs/image-live-objects.md AC-08).
#[wasm_bindgen]
pub fn set_active_layer(handle: u32, index: usize) -> Result<(), JsError> {
    with_bus(handle, |bus| bus.set_active_layer(index).map_err(to_js))
}

/// Lays out a text with the registered font `font_id` as a live text with
/// these settings is drawn (docs/specs/image-live-text.md AC-22): `request`
/// is a JSON `TextLayoutRequest`; returns a `TextLayout` object (line boxes,
/// caret x per character boundary).
#[wasm_bindgen]
pub fn text_layout(font_id: u32, request: &str) -> Result<JsValue, JsError> {
    let req: TextLayoutRequest =
        serde_json::from_str(request).map_err(|e| JsError::new(&e.to_string()))?;
    let layout = fonts::with_faces(|faces| {
        let face = faces
            .get(font_id as usize)
            .ok_or_else(|| JsError::new("unknown font id"))?;
        layout_text(&req, *face).ok_or_else(|| JsError::new("the font could not be read"))
    })?;
    serde_wasm_bindgen::to_value(&layout).map_err(|e| JsError::new(&e.to_string()))
}

/// The live object (docs/specs/image-live-objects.md) as a
/// `LiveObjectSpec` JS object, or `null` when nothing is live.
#[wasm_bindgen]
pub fn live_object(handle: u32) -> Result<JsValue, JsError> {
    with_bus(handle, |bus| match live_object_spec(bus) {
        Some(spec) => serde_wasm_bindgen::to_value(&spec).map_err(|e| JsError::new(&e.to_string())),
        None => Ok(JsValue::NULL),
    })
}

/// Runs the effect `name` with the positional `params` (the shared
/// `pincel_effects::EffectSpec` wire format, also used by Pixel mode — see
/// `EFFECT_NAMES` and `ui/src/lib/effects/catalog.ts`) on the layer at
/// `layer`, limited to the selection, and commits it as one undoable
/// `SetPixels`. Returns `false` (no undo step) when the selection is empty,
/// or when Rotate / Zoom changes no pixel.
#[wasm_bindgen]
pub fn apply_effect(
    handle: u32,
    layer: usize,
    name: &str,
    params: &[f64],
) -> Result<bool, JsError> {
    let spec = EffectSpec::from_params(name, params).map_err(|e| JsError::new(&e.to_string()))?;
    with_bus(handle, |bus| {
        if layer_locked(&bus.document, layer) {
            return Err(JsError::new(LOCKED));
        }
        let region = effect_region(&bus.document, layer, &spec)?;
        commit_region(bus, layer, name, &spec, region)
    })
}

/// Commits the region of the effect `name` as one undoable `SetPixels` of
/// that effect's history kind; `false` (no undo step) for `None`, and for a
/// Rotate / Zoom region the layer already holds
/// (docs/specs/image-transform-handles-rotate-zoom.md AC-05). Every other
/// effect stays one step per Apply, changed or not (effect-panel AC-04).
fn commit_region(
    bus: &mut CommandBus,
    layer: usize,
    name: &str,
    spec: &EffectSpec,
    region: Option<(Rect, ImageBuffer)>,
) -> Result<bool, JsError> {
    let Some((region, after)) = region else {
        return Ok(false);
    };
    let unchanged = matches!(spec, EffectSpec::RotateZoom(_))
        && bus
            .document
            .layers()
            .get(layer)
            .and_then(|l| l.pixels.copy_region(region).ok())
            .is_some_and(|now| now.data() == after.data());
    if unchanged {
        return Ok(false);
    }
    bus.apply(Box::new(effect_step(layer, region, after, name)?))
        .map_err(to_js)?;
    Ok(true)
}

/// The undo step of an effect's region: a `SetPixels` of the history kind
/// of the effect `name` (e.g. `effect.rotate_zoom`).
fn effect_step(
    layer: usize,
    region: Rect,
    after: ImageBuffer,
    name: &str,
) -> Result<SetPixels, JsError> {
    let kind = HistoryKind::effect(name)
        .ok_or_else(|| JsError::new(&format!("unknown effect `{name}`")))?;
    Ok(SetPixels::new(layer, region, after).with_kind(kind))
}

/// Runs effect `name` / `params` on the layer at `layer` the way
/// [`apply_effect`] does, but returns the result instead of committing it:
/// empty when the selection is empty, otherwise a 16-byte header (x, y as
/// little-endian `i32`, width, height as little-endian `u32`) followed by
/// the region's RGBA8 pixels. The Image job worker computes it on a
/// document snapshot and the page commits it with [`apply_effect_region`]
/// (docs/specs/performance-background-jobs.md AC-07).
#[wasm_bindgen]
pub fn effect_region_bytes(
    handle: u32,
    layer: usize,
    name: &str,
    params: &[f64],
) -> Result<Vec<u8>, JsError> {
    let spec = EffectSpec::from_params(name, params).map_err(|e| JsError::new(&e.to_string()))?;
    with_bus(handle, |bus| {
        Ok(encode_region(effect_region(&bus.document, layer, &spec)?))
    })
}

/// Commits `region` (bytes from [`effect_region_bytes`] for the same
/// effect, layer and document state) to the layer at `layer` as the one
/// undoable step [`apply_effect`] makes. Returns `false` (no undo step)
/// for an empty region, or a Rotate / Zoom region that changes no pixel.
#[wasm_bindgen]
pub fn apply_effect_region(
    handle: u32,
    layer: usize,
    name: &str,
    params: &[f64],
    region: &[u8],
) -> Result<bool, JsError> {
    // Validates the effect name and parameters before touching the document.
    let spec = EffectSpec::from_params(name, params).map_err(|e| JsError::new(&e.to_string()))?;
    let region = decode_region(region).map_err(job_js)?;
    with_bus(handle, |bus| {
        if layer_locked(&bus.document, layer) {
            return Err(JsError::new(LOCKED));
        }
        commit_region(bus, layer, name, &spec, region)
    })
}

/// [`jobs::effect_region`], the function every job backend runs.
fn effect_region(
    doc: &Document,
    layer: usize,
    spec: &EffectSpec,
) -> Result<Option<(Rect, ImageBuffer)>, JsError> {
    jobs::effect_region(doc, layer, spec).map_err(job_js)
}

/// Renders a live preview of effect `name` / `params` on the layer at
/// `layer`: composites the document with that layer's pixels replaced,
/// without mutating document state or the undo stack. Returns canvas-sized
/// RGBA8 (spec §11 live preview).
#[wasm_bindgen]
pub fn preview_effect(
    handle: u32,
    layer: usize,
    name: &str,
    params: &[f64],
) -> Result<Clamped<Vec<u8>>, JsError> {
    let spec = EffectSpec::from_params(name, params).map_err(|e| JsError::new(&e.to_string()))?;
    with_bus(handle, |bus| {
        let out = jobs::preview_effect(&bus.document, layer, &spec).map_err(job_js)?;
        Ok(Clamped(out.into_raw()))
    })
}

/// Undoes the last command. Returns `true` if something was undone.
#[wasm_bindgen]
pub fn undo(handle: u32) -> Result<bool, JsError> {
    with_bus(handle, |bus| bus.undo().map_err(to_js))
}

/// Redoes the last undone command. Returns `true` if something was redone.
#[wasm_bindgen]
pub fn redo(handle: u32) -> Result<bool, JsError> {
    with_bus(handle, |bus| bus.redo().map_err(to_js))
}

/// Presents the canvas rect `(x, y, w, h)` (clipped to the canvas) at
/// `out_w × out_h` — the UI's viewport renderer, so a canvas larger than
/// any browser canvas still displays (spec §6.4). The pixels come from the
/// document's cached composite, updated only where commands changed it
/// (docs/specs/performance-image-cache.md): the clipped size as `out_w` /
/// `out_h` gives 1:1 pixels identical to a fresh compose; smaller sizes
/// (zoomed-out views) sample the cache's downscaled pyramid. Returns RGBA8
/// rows of the output size; an empty region yields an empty array.
#[wasm_bindgen]
pub fn composite_region(
    handle: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    out_w: u32,
    out_h: u32,
) -> Result<Clamped<Vec<u8>>, JsError> {
    with_bus(handle, |bus| {
        let damage = bus.take_damage();
        CACHES.with(|caches| {
            let mut caches = caches.borrow_mut();
            let cache = caches
                .entry(handle)
                .or_insert_with(|| RenderCache::new(&bus.document));
            cache.update(&bus.document, damage);
            let out = cache.region(&bus.document, Rect::new(x, y, w, h), out_w, out_h);
            Ok(Clamped(out.into_raw()))
        })
    })
}

/// Exports the flattened composite as PNG bytes. `compression` is 0–9.
#[wasm_bindgen]
pub fn export_png(handle: u32, compression: u8) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        jobs::encode(&bus.document, EncodeFormat::Png, compression, 0).map_err(job_js)
    })
}

/// Exports the flattened composite as JPEG bytes. `quality` is 1–100.
#[wasm_bindgen]
pub fn export_jpeg(handle: u32, quality: u8) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        jobs::encode(&bus.document, EncodeFormat::Jpeg, 0, quality).map_err(job_js)
    })
}

/// Exports the flattened composite as lossless WebP bytes (ADR-007).
#[wasm_bindgen]
pub fn export_webp(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        jobs::encode(&bus.document, EncodeFormat::Webp, 0, 0).map_err(job_js)
    })
}

/// Exports the flattened composite as 32-bit RGBA BMP bytes.
#[wasm_bindgen]
pub fn export_bmp(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        jobs::encode(&bus.document, EncodeFormat::Bmp, 0, 0).map_err(job_js)
    })
}

/// Exports the flattened composite as RLE-compressed 32-bit TGA bytes
/// (docs/specs/image-formats-more.md).
#[wasm_bindgen]
pub fn export_tga(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        to_tga_bytes(&compose(bus.document.layers())).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Exports the flattened composite as a one-image ICO; fails above
/// 256 × 256 px (docs/specs/image-formats-more.md AC-08).
#[wasm_bindgen]
pub fn export_ico(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        to_ico_bytes(&compose(bus.document.layers())).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Decodes `data` (a format `hint` — MIME type or file name — for TGA) and
/// re-encodes it as PNG, so a dropped TGA / DDS / ICO goes through the
/// same paste path as the browser's own formats (AC-13).
#[wasm_bindgen]
pub fn image_to_png(data: &[u8], hint: &str) -> Result<Vec<u8>, JsError> {
    let buffer = pincel_image_core::codec::decode_hinted(data, Some(hint))
        .map_err(|e| JsError::new(&e.to_string()))?;
    to_png_bytes(&buffer, 1).map_err(|e| JsError::new(&e.to_string()))
}

/// Exports the flattened composite as lossless RGBA TIFF bytes.
#[wasm_bindgen]
pub fn export_tiff(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        jobs::encode(&bus.document, EncodeFormat::Tiff, 0, 0).map_err(job_js)
    })
}

/// Per-layer state for the layers panel (spec §16.5).
#[derive(Debug, Serialize)]
struct LayerInfo {
    /// Stable layer id (string form of the core `Uuid`).
    id: String,
    name: String,
    opacity: f32,
    /// Blend mode as a snake_case string (see [`blend_mode_str`]).
    blend_mode: String,
    visible: bool,
    locked: bool,
    /// `"raster"` or `"group"`.
    kind: &'static str,
    /// Index of the group the layer sits in; `None` at the root.
    parent: Option<usize>,
    /// Number of groups the layer is nested in.
    depth: usize,
    /// Whether a group the layer sits in is locked.
    locked_by_parent: bool,
}

/// The layers panel's view of every layer, bottom first.
fn layer_infos(doc: &Document) -> Vec<LayerInfo> {
    doc.layers()
        .iter()
        .enumerate()
        .map(|(i, l)| LayerInfo {
            id: l.id.to_string(),
            name: l.name.clone(),
            opacity: l.opacity,
            blend_mode: blend_mode_str(l.blend_mode).to_string(),
            visible: l.visible,
            locked: l.locked,
            kind: if l.is_group() { "group" } else { "raster" },
            parent: doc.parent_index(i),
            depth: doc.depth(i),
            locked_by_parent: doc.locked_by_parent(i),
        })
        .collect()
}

/// Lightweight document state for the UI (spec §17 `DocumentInfo`).
#[derive(Debug, Serialize)]
struct DocumentInfo {
    width: u32,
    height: u32,
    layer_count: usize,
    active_layer: usize,
    can_undo: bool,
    can_redo: bool,
    /// Entries on the undo stack; lets the UI tell whether a gesture
    /// pushed one (e.g. to roll back a stroke a pinch interrupted).
    undo_depth: usize,
    /// Whether a selection is currently active (`Some` mask in the document).
    has_selection: bool,
    /// Layers ordered bottom (index 0) to top, matching core storage order.
    layers: Vec<LayerInfo>,
}

/// Returns the current document state as a plain JS object.
#[wasm_bindgen]
pub fn get_document_info(handle: u32) -> Result<JsValue, JsError> {
    with_bus(handle, |bus| {
        let layers = layer_infos(&bus.document);
        let info = DocumentInfo {
            width: bus.document.canvas.width(),
            height: bus.document.canvas.height(),
            layer_count: bus.document.layer_count(),
            active_layer: bus.document.active_layer_index(),
            can_undo: bus.history.can_undo(),
            can_redo: bus.history.can_redo(),
            undo_depth: bus.history.undo_depth(),
            has_selection: bus.document.selection.is_some(),
            layers,
        };
        serde_wasm_bindgen::to_value(&info).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// The document's stored resolution in pixels per inch (`DocumentMetadata::dpi`),
/// read-only (docs/specs/image-view-aids.md AC-23; reused by
/// docs/specs/image-resize-autocrop.md AC-01).
#[wasm_bindgen]
pub fn document_dpi(handle: u32) -> Result<f32, JsError> {
    with_bus(handle, |bus| Ok(bus.document.metadata.dpi))
}

/// Returns a 32×32 RGBA8 thumbnail of the layer with `layer_id` as a
/// `Uint8ClampedArray`, ready to wrap in an `ImageData` (spec §5.3, §17).
#[wasm_bindgen]
pub fn get_layer_thumbnail(handle: u32, layer_id: &str) -> Result<Clamped<Vec<u8>>, JsError> {
    with_bus(handle, |bus| {
        layer_thumbnail_pixels(&bus.document, layer_id)
            .map(Clamped)
            .ok_or_else(|| JsError::new("layer not found"))
    })
}

/// The 32×32 thumbnail of the layer with `layer_id`: its pixels, or for a
/// group the composite of its subtree, hidden or not. `None` for an
/// unknown id.
fn layer_thumbnail_pixels(doc: &Document, layer_id: &str) -> Option<Vec<u8>> {
    let index = doc
        .layers()
        .iter()
        .position(|l| l.id.to_string() == layer_id)?;
    let layer = &doc.layers()[index];
    if !layer.is_group() {
        return Some(thumbnail(&layer.pixels));
    }
    let mut subtree = doc.layers()[index..doc.subtree_end(index)].to_vec();
    subtree[0].visible = true;
    Some(thumbnail(&compose(&subtree)))
}

/// Returns the selection's bounding box as `[x, y, w, h]`, or an empty array
/// when there is no active selection. Used to position the marching-ants
/// overlay (spec §8.5).
#[wasm_bindgen]
pub fn get_selection_bounds(handle: u32) -> Result<Vec<u32>, JsError> {
    with_bus(handle, |bus| {
        match bus
            .document
            .selection
            .as_ref()
            .and_then(|s| s.bounding_box())
        {
            Some(r) => Ok(vec![r.x.max(0) as u32, r.y.max(0) as u32, r.w, r.h]),
            None => Ok(Vec::new()),
        }
    })
}

/// Returns the raw selection coverage mask (one byte per pixel, row-major,
/// canvas-sized), or an empty array when there is no active selection. The UI
/// overlay derives the marching-ants outline from it (spec §8.5).
#[wasm_bindgen]
pub fn get_selection_mask(handle: u32) -> Result<Clamped<Vec<u8>>, JsError> {
    with_bus(handle, |bus| {
        Ok(Clamped(
            bus.document
                .selection
                .as_ref()
                .map(|s| s.data().to_vec())
                .unwrap_or_default(),
        ))
    })
}

/// Converts a core error into a JS error.
fn to_js(e: pincel_image_core::DocumentError) -> JsError {
    JsError::new(&e.to_string())
}

fn job_js(e: JobError) -> JsError {
    JsError::new(&e.to_string())
}

#[cfg(test)]
mod ts_bindings {
    use super::CommandSpec;
    use pincel_image_core::TextLayout;
    use pincel_image_core::command::live_spec::{LiveObjectSpec, TextLayoutRequest};
    use ts_rs::{Config, TS};

    /// Regenerates the TypeScript mirror of `CommandSpec` (Fineliner
    /// ADR-014). Effects use the shared positional `pincel_effects`
    /// wire format instead (spec §15, 2026-10-11). CI checks that the committed files under
    /// `ui/src/modes/image/core/generated/` are in sync.
    #[test]
    fn export_command_spec_bindings() {
        // u64 fields (stroke_id) export as `number`: the UI
        // generates small values, far below Number.MAX_SAFE_INTEGER.
        let cfg = Config::new()
            .with_large_int("number")
            .with_out_dir("../../ui/src/modes/image/core/generated");
        CommandSpec::export_all(&cfg).expect("export CommandSpec bindings");
        LiveObjectSpec::export_all(&cfg).expect("export LiveObjectSpec bindings");
        TextLayoutRequest::export_all(&cfg).expect("export TextLayoutRequest bindings");
        TextLayout::export_all(&cfg).expect("export TextLayout bindings");
    }
}

#[cfg(test)]
mod group_tests {
    use super::*;
    use pincel_image_core::Layer;

    /// bg, G { a } with a red pixel at (0, 0) on `a`.
    fn grouped() -> Document {
        let mut doc = Document::new(4, 4).expect("doc");
        let group = Layer::new_group("G");
        let gid = group.id;
        doc.insert_layer(1, group).expect("group");
        let mut a = Layer::new_transparent("a", 4, 4).with_parent(Some(gid));
        a.pixels.set_pixel(0, 0, Color::rgba(255, 0, 0, 255));
        doc.insert_layer(2, a).expect("a");
        doc
    }

    #[test]
    fn layer_infos_report_kind_depth_parent_and_inherited_lock() {
        let mut doc = grouped();
        doc.set_active_layer(1).expect("active");
        doc.active_layer_mut().locked = true;
        let infos = layer_infos(&doc);
        assert_eq!(infos[0].kind, "raster");
        assert_eq!(infos[1].kind, "group");
        assert_eq!((infos[2].depth, infos[2].parent), (1, Some(1)));
        assert!(infos[2].locked_by_parent && !infos[2].locked);
        assert!(!infos[1].locked_by_parent);
    }

    #[test]
    fn group_thumbnail_shows_its_subtree() {
        let mut doc = grouped();
        doc.set_active_layer(1).expect("active");
        doc.active_layer_mut().visible = false;
        let id = doc.layers()[1].id.to_string();
        let thumb = layer_thumbnail_pixels(&doc, &id).expect("thumbnail");
        assert_eq!(&thumb[..4], &[255, 0, 0, 255], "hidden or not");
    }
}

#[cfg(test)]
mod project_tests {
    use super::*;

    #[test]
    fn save_project_then_open_project_keeps_the_layers() {
        let mut doc = Document::new(5, 4).expect("doc");
        doc.add_layer("Top").expect("layer");
        let handle = insert(CommandBus::new(doc));
        let bytes = save_project(handle, 1).expect("save");
        let reopened = open_project(&bytes).expect("open");
        let names: Vec<String> = with_bus(reopened, |bus| {
            Ok(bus
                .document
                .layers()
                .iter()
                .map(|l| l.name.clone())
                .collect())
        })
        .expect("read");
        assert_eq!(names, vec!["Layer 1".to_string(), "Top".to_string()]);
    }
}

#[cfg(test)]
mod psd_tests {
    use super::*;

    const SAMPLE: &[u8] = include_bytes!("../../pincel-image-core/tests/fixtures/sample.psd");

    #[test]
    fn open_psd_document_opens_layers_and_reports_notices() {
        let opened = open_psd_document(SAMPLE, 16384, 8192 * 8192).expect("opens");
        let names: Vec<String> = with_bus(opened.handle, |bus| {
            Ok(bus
                .document
                .layers()
                .iter()
                .map(|l| l.name.clone())
                .collect())
        })
        .expect("read");
        assert_eq!(names, ["Base", "Folder", "Ink", "Title"]);
        assert!(opened.notices.contains(&PsdNotice::Adjustment {
            layer: "Curves".into()
        }));
    }

    #[test]
    fn open_psd_document_over_the_canvas_limits_opens_nothing() {
        let before = DOCUMENTS.with(|d| d.borrow().len());
        let err = open_psd_document(SAMPLE, 39, 8192 * 8192).expect_err("too wide");
        assert!(err.contains("40 × 30"), "{err}");
        let err = open_psd_document(SAMPLE, 16384, 1199).expect_err("too large");
        assert!(err.contains("size limit"), "{err}");
        assert_eq!(DOCUMENTS.with(|d| d.borrow().len()), before);
    }
}

#[cfg(test)]
mod paste_tests {
    use super::*;
    use pincel_image_core::SelectionMask;
    use pincel_image_core::command::AddLayer;

    #[test]
    fn paste_layer_expands_canvas_and_undo_restores_it() {
        let handle = insert(CommandBus::new(Document::new(4, 4).expect("doc")));
        let png = to_png_bytes(&ImageBuffer::new_transparent(6, 5), 1).expect("png");
        paste_layer(handle, &png, 0, 0, true).expect("paste");
        let after = with_bus(handle, |bus| {
            let doc = &bus.document;
            Ok((doc.canvas.width(), doc.canvas.height(), doc.layers().len()))
        })
        .expect("read");
        assert_eq!(after, (6, 5, 2));
        let undone = with_bus(handle, |bus| {
            bus.undo().map_err(to_js)?;
            let doc = &bus.document;
            Ok((doc.canvas.width(), doc.canvas.height(), doc.layers().len()))
        })
        .expect("undo");
        assert_eq!(undone, (4, 4, 1));
    }

    fn layer_pixel(handle: u32, layer: usize, x: u32, y: u32) -> Color {
        with_bus(handle, |bus| {
            Ok(bus.document.layers()[layer]
                .pixels
                .get_pixel(x, y)
                .expect("in bounds"))
        })
        .expect("read")
    }

    /// 4×4 document with red at (1, 1) and blue at (3, 3) on its layer.
    fn painted() -> u32 {
        let mut doc = Document::new(4, 4).expect("doc");
        let px = &mut doc.active_layer_mut().pixels;
        px.set_pixel(1, 1, Color::rgba(255, 0, 0, 255));
        px.set_pixel(3, 3, Color::rgba(0, 0, 255, 255));
        insert(CommandBus::new(doc))
    }

    fn select(handle: u32, x: i32, y: i32, w: u32, h: u32) {
        with_bus(handle, |bus| {
            bus.document.selection = Some(SelectionMask::rectangle(4, 4, Rect::new(x, y, w, h)));
            Ok(())
        })
        .expect("select");
    }

    #[test]
    fn paste_layer_lands_at_the_position_and_expands_to_cover_it() {
        let handle = insert(CommandBus::new(Document::new(4, 4).expect("doc")));
        let mut img = ImageBuffer::new_transparent(6, 5);
        img.set_pixel(0, 0, Color::WHITE);
        let png = to_png_bytes(&img, 1).expect("png");
        paste_layer(handle, &png, 1, 2, true).expect("paste");
        let size = with_bus(handle, |bus| {
            Ok((bus.document.canvas.width(), bus.document.canvas.height()))
        })
        .expect("read");
        assert_eq!(size, (7, 7));
        assert_eq!(layer_pixel(handle, 1, 1, 2), Color::WHITE);
    }

    #[test]
    fn copy_selection_is_the_selected_box_and_leaves_the_document_alone() {
        let handle = painted();
        select(handle, 1, 1, 2, 2);
        let png = copy_selection(handle, false).expect("copy");
        let img = pincel_image_core::codec::decode(&png).expect("decode");
        assert_eq!((img.width(), img.height()), (2, 2));
        assert_eq!(img.get_pixel(0, 0), Some(Color::rgba(255, 0, 0, 255)));
        assert_eq!(img.get_pixel(1, 1), Some(Color::TRANSPARENT));
        let undoable = with_bus(handle, |bus| Ok(bus.history.can_undo())).expect("read");
        assert!(!undoable);
        assert_eq!(layer_pixel(handle, 0, 1, 1), Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn copy_selection_merged_and_whole_layer_without_selection() {
        let handle = painted();
        with_bus(handle, |bus| {
            bus.apply(Box::new(AddLayer::above(0))).map_err(to_js)
        })
        .expect("layer");
        let own = pincel_image_core::codec::decode(&copy_selection(handle, false).expect("copy"))
            .expect("decode");
        assert_eq!((own.width(), own.height()), (4, 4));
        assert_eq!(
            own.get_pixel(1, 1),
            Some(Color::TRANSPARENT),
            "empty top layer"
        );
        let merged =
            pincel_image_core::codec::decode(&copy_selection(handle, true).expect("copy merged"))
                .expect("decode");
        assert_eq!(merged.get_pixel(1, 1), Some(Color::rgba(255, 0, 0, 255)));
    }

    #[test]
    fn shape_outline_gives_the_core_polygon_of_a_box_shape() {
        let tri = shape_outline("triangle", 0.0, 0.0, 10.0, 8.0, 5, 0.38);
        assert_eq!(tri, vec![0.0, 8.0, 5.0, 0.0, 10.0, 8.0]);
        assert_eq!(
            shape_outline("star", 0.0, 0.0, 10.0, 10.0, 7, 0.5).len(),
            28
        );
        assert!(shape_outline("rectangle", 0.0, 0.0, 10.0, 8.0, 5, 0.38).is_empty());
        assert!(shape_outline("heart", 0.0, 0.0, 0.0, 8.0, 5, 0.38).is_empty());
    }

    #[test]
    fn cut_selection_copies_clears_and_one_undo_restores() {
        let handle = painted();
        select(handle, 1, 1, 2, 2);
        let copied = copy_selection(handle, false).expect("copy");
        let cut = cut_selection(handle).expect("cut");
        assert_eq!(image_key(&cut).ok(), image_key(&copied).ok());
        assert_eq!(layer_pixel(handle, 0, 1, 1), Color::TRANSPARENT);
        assert_eq!(layer_pixel(handle, 0, 3, 3), Color::rgba(0, 0, 255, 255));
        let restored = with_bus(handle, |bus| {
            bus.undo().map_err(to_js)?;
            Ok(bus.history.can_undo())
        })
        .expect("undo");
        assert!(!restored, "one undo step");
        assert_eq!(layer_pixel(handle, 0, 1, 1), Color::rgba(255, 0, 0, 255));
    }

    #[test]
    fn cut_selection_without_selection_clears_the_layer() {
        let handle = painted();
        let cut =
            pincel_image_core::codec::decode(&cut_selection(handle).expect("cut")).expect("decode");
        assert_eq!((cut.width(), cut.height()), (4, 4));
        assert_eq!(layer_pixel(handle, 0, 3, 3), Color::TRANSPARENT);
        with_bus(handle, |bus| bus.undo().map_err(to_js)).expect("undo");
        assert_eq!(layer_pixel(handle, 0, 3, 3), Color::rgba(0, 0, 255, 255));
    }

    #[test]
    fn image_key_survives_reencoding_and_ignores_hidden_color() {
        let mut img = ImageBuffer::new_transparent(2, 1);
        img.set_pixel(0, 0, Color::rgba(10, 20, 30, 128));
        let a = to_png_bytes(&img, 1).expect("png");
        let b = to_png_bytes(&img, 9).expect("png");
        assert_ne!(a, b);
        assert_eq!(image_key(&a).ok(), image_key(&b).ok());
        assert_eq!(
            rgba_key(1, 1, &[200, 100, 50, 0]),
            rgba_key(1, 1, &[0, 0, 0, 0])
        );
    }
}

#[cfg(test)]
mod effect_tests {
    use super::*;
    use pincel_image_core::SelectionMask;

    fn gray_row(values: [u8; 4]) -> ImageBuffer {
        let data = values.iter().flat_map(|&v| [v, v, v, 255]).collect();
        ImageBuffer::from_raw(4, 1, data).expect("row")
    }

    /// adjustments-paintnet AC-05 / AC-10 in the Image editor: Auto-Level
    /// takes its range from the selection, changes only the selected pixels
    /// of the target layer, and is one undo step.
    #[test]
    fn apply_effect_auto_level_stretches_the_selection_only() {
        let mut doc = Document::new(4, 1).expect("doc");
        doc.add_layer("top").expect("layer");
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(SetPixels::new(
            0,
            Rect::new(0, 0, 4, 1),
            gray_row([0, 80, 160, 255]),
        )))
        .expect("paint");
        bus.apply(Box::new(SetPixels::new(
            1,
            Rect::new(0, 0, 4, 1),
            gray_row([40, 40, 200, 200]),
        )))
        .expect("paint");
        bus.document.selection = Some(SelectionMask::rectangle(4, 1, Rect::new(1, 0, 2, 1)));
        let other_layer = bus.document.layers()[1].pixels.clone();
        let depth = bus.history.undo_depth();
        let handle = insert(bus);

        assert!(apply_effect(handle, 0, "auto_level", &[]).expect("apply"));
        with_bus(handle, |bus| {
            let px = bus.document.layers()[0].pixels.data().to_vec();
            let reds: Vec<u8> = px.iter().step_by(4).copied().collect();
            assert_eq!(reds, [0, 0, 255, 255], "80 / 160 stretch to 0 / 255");
            assert_eq!(bus.document.layers()[1].pixels, other_layer, "other layer");
            assert_eq!(bus.history.undo_depth(), depth + 1, "one undo step");
            Ok(())
        })
        .expect("bus");
    }
}

#[cfg(test)]
mod outline_tests {
    use super::*;
    use pincel_image_core::SelectionMask;

    const COLOR: [u8; 4] = [0xFF, 0x80, 0x10, 0xC0];
    const OUTLINE: [f64; 4] = [0.0, 1.0, 1.0, 0xFF80_10C0_u32 as f64];

    fn px(bus: &CommandBus, x: u32, y: u32) -> [u8; 4] {
        let p = &bus.document.layers()[0].pixels;
        let o = ((y * p.width() + x) * 4) as usize;
        let d = p.data();
        [d[o], d[o + 1], d[o + 2], d[o + 3]]
    }

    /// One opaque pixel at (2, 1) on a 6 × 3 layer.
    fn dot() -> CommandBus {
        let mut data = vec![0; 6 * 3 * 4];
        data[(6 + 2) * 4..(6 + 2) * 4 + 4].copy_from_slice(&[10, 20, 30, 255]);
        let mut bus = CommandBus::new(Document::new(6, 3).expect("doc"));
        bus.apply(Box::new(SetPixels::new(
            0,
            Rect::new(0, 0, 6, 3),
            ImageBuffer::from_raw(6, 3, data).expect("buf"),
        )))
        .expect("paint");
        bus
    }

    /// outline-effect AC-03 / AC-06 / AC-09 in Image mode: the neighbours
    /// take the colour exactly, in one undo step that redo repeats.
    #[test]
    fn apply_effect_outline_colours_neighbours_and_undoes_in_one_step() {
        let bus = dot();
        let before = bus.document.layers()[0].pixels.clone();
        let depth = bus.history.undo_depth();
        let handle = insert(bus);
        assert!(apply_effect(handle, 0, "outline", &OUTLINE).expect("apply"));
        with_bus(handle, |bus| {
            assert_eq!(bus.history.undo_depth(), depth + 1);
            for (x, y) in [(1, 0), (3, 2), (1, 1), (3, 1)] {
                assert_eq!(px(bus, x, y), COLOR, "({x}, {y})");
            }
            assert_eq!(px(bus, 2, 1), [10, 20, 30, 255]);
            assert_eq!(px(bus, 4, 1), [0, 0, 0, 0]);
            Ok(())
        })
        .expect("bus");
        assert!(undo(handle).expect("undo"));
        with_bus(handle, |bus| {
            assert_eq!(bus.document.layers()[0].pixels, before, "undo restores");
            Ok(())
        })
        .expect("bus");
        assert!(redo(handle).expect("redo"));
        with_bus(handle, |bus| {
            assert_eq!(px(bus, 3, 1), COLOR, "redo repeats");
            Ok(())
        })
        .expect("bus");
    }

    /// outline-effect AC-07 in Image mode: only the selection changes, an
    /// opaque pixel outside it outlines into it, and a partly selected
    /// pixel blends the outline by its coverage.
    #[test]
    fn apply_effect_outline_blends_a_feathered_selection_by_coverage() {
        let mut bus = dot();
        let mut mask = SelectionMask::rectangle(6, 3, Rect::new(3, 0, 3, 3));
        mask.set(3, 0, 128);
        bus.document.selection = Some(mask);
        let handle = insert(bus);
        assert!(apply_effect(handle, 0, "outline", &OUTLINE).expect("apply"));
        with_bus(handle, |bus| {
            assert_eq!(px(bus, 3, 1), COLOR, "fully selected: exact colour");
            assert_eq!(px(bus, 3, 2), COLOR);
            // 0 + (c - 0) * 128 / 255, rounded.
            let half = COLOR.map(|c| ((u32::from(c) * 128 + 127) / 255) as u8);
            assert_eq!(px(bus, 3, 0), half, "coverage 128 blends");
            for y in 0..3 {
                assert_eq!(px(bus, 1, y), [0, 0, 0, 0], "(1, {y}) outside");
            }
            Ok(())
        })
        .expect("bus");
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use pincel_image_core::SelectionMask;

    /// effects-render: the four render effects, Clouds and Turbulence
    /// at a small scale with black / white.
    const RENDERS: [(&str, &[f64]); 4] = [
        ("clouds", &[3.0, 50.0, 1.0, 255.0, 4_294_967_295.0, 0.0]),
        ("turbulence", &[3.0, 5.0, 1.0, 255.0, 4_294_967_295.0, 0.0]),
        ("julia_fractal", &[4.0, 1.0, 0.0, 2.0]),
        ("mandelbrot_fractal", &[1.0, 10.0, 0.0, 2.0, 0.0]),
    ];

    const W: u32 = 6;
    const H: u32 = 4;

    fn px(buf: &ImageBuffer, x: u32, y: u32) -> [u8; 4] {
        let o = ((y * W + x) * 4) as usize;
        let d = buf.data();
        [d[o], d[o + 1], d[o + 2], d[o + 3]]
    }

    /// Two painted layers, a selection of columns 2..5 with (2, 0) half
    /// selected.
    fn layered() -> CommandBus {
        let mut doc = Document::new(W, H).expect("doc");
        doc.add_layer("top").expect("layer");
        let mut bus = CommandBus::new(doc);
        for (layer, v) in [(0, 90u8), (1, 200)] {
            let data = (0..W * H).flat_map(|i| [v, i as u8 * 9, 40, 255]).collect();
            let buf = ImageBuffer::from_raw(W, H, data).expect("buf");
            bus.apply(Box::new(SetPixels::new(layer, Rect::new(0, 0, W, H), buf)))
                .expect("paint");
        }
        let mut mask = SelectionMask::rectangle(W, H, Rect::new(2, 0, 3, H));
        mask.set(2, 0, 128);
        bus.document.selection = Some(mask);
        bus
    }

    /// effects-render AC-09 in Image mode: only the selection changes
    /// (fully selected pixels take the rendering exactly, a partly selected
    /// one blends by its coverage), the other layer stays, one undo step
    /// restores the target.
    #[test]
    fn apply_effect_render_changes_only_the_selection_in_one_undo_step() {
        for (name, params) in RENDERS {
            let bus = layered();
            let before = bus.document.layers()[0].pixels.clone();
            let other = bus.document.layers()[1].pixels.clone();
            let src =
                pincel_effects::EffectImage::from_rgba8(W, H, before.data().to_vec()).expect("img");
            let full = ImageBuffer::from_raw(
                W,
                H,
                EffectSpec::from_params(name, params)
                    .expect(name)
                    .run(&src)
                    .into_raw(),
            )
            .expect("buf");
            let depth = bus.history.undo_depth();
            let handle = insert(bus);
            assert!(apply_effect(handle, 0, name, params).expect(name), "{name}");
            with_bus(handle, |bus| {
                let after = &bus.document.layers()[0].pixels;
                assert_eq!(bus.history.undo_depth(), depth + 1, "{name}: one undo step");
                for y in 0..H {
                    for x in 0..W {
                        let (a, b, f) = (px(after, x, y), px(&before, x, y), px(&full, x, y));
                        if !(2..5).contains(&x) {
                            assert_eq!(a, b, "{name} ({x}, {y}) outside");
                        } else if (x, y) == (2, 0) {
                            for ch in 0..4 {
                                let (lo, hi) = (b[ch].min(f[ch]), b[ch].max(f[ch]));
                                assert!((lo..=hi).contains(&a[ch]), "{name} partial ch {ch}");
                            }
                        } else {
                            assert_eq!(a, f, "{name} ({x}, {y}) selected");
                        }
                    }
                }
                assert_ne!(after, &before, "{name} changed nothing");
                assert_eq!(
                    bus.document.layers()[1].pixels,
                    other,
                    "{name}: other layer"
                );
                Ok(())
            })
            .expect("bus");
            assert!(undo(handle).expect("undo"));
            with_bus(handle, |bus| {
                assert_eq!(bus.document.layers()[0].pixels, before, "{name}: undo");
                Ok(())
            })
            .expect("bus");
        }
    }
}

#[cfg(test)]
mod region_tests {
    use super::*;
    use pincel_image_core::compose_region;

    #[test]
    fn composite_region_crops_and_downscales() {
        let doc = Document::new(8, 6).expect("doc");
        let handle = insert(CommandBus::new(doc));
        let crop = composite_region(handle, 2, 1, 4, 4, 4, 4).expect("crop");
        assert_eq!(crop.0.len(), 4 * 4 * 4);
        let small = composite_region(handle, 0, 0, 8, 6, 4, 3).expect("scaled");
        assert_eq!(small.0.len(), 4 * 3 * 4);
        let outside = composite_region(handle, 20, 20, 4, 4, 4, 4).expect("empty");
        assert!(outside.0.is_empty());
    }

    /// A 3-layer 40×30 document with differently colored, half-transparent
    /// upper layers.
    fn layered() -> u32 {
        let handle = insert(CommandBus::new(Document::new(40, 30).expect("doc")));
        for (layer, color) in [(0, [200, 40, 10, 255]), (1, [10, 90, 220, 140])] {
            if layer > 0 {
                apply_command(
                    handle,
                    &format!(r#"{{"type":"add_layer","active":{}}}"#, layer - 1),
                )
                .expect("add layer");
            }
            let dab = format!(
                r#"{{"type":"pencil_stroke","layer":{layer},"size":30,"color":{color:?},"opacity":1,"points":[[{x},15]],"stroke_id":{layer}}}"#,
                x = 12 + layer * 14
            );
            apply_command(handle, &dab).expect("paint");
        }
        apply_command(handle, r#"{"type":"add_layer","active":1}"#).expect("add layer");
        handle
    }

    fn fresh_compose(handle: u32, x: i32, y: i32, w: u32, h: u32) -> Vec<u8> {
        with_bus(handle, |bus| {
            Ok(compose_region(bus.document.layers(), Rect::new(x, y, w, h)).into_raw())
        })
        .expect("compose")
    }

    fn canvas(handle: u32) -> (u32, u32) {
        with_bus(handle, |bus| {
            Ok((bus.document.canvas.width(), bus.document.canvas.height()))
        })
        .expect("size")
    }

    #[test]
    fn composite_region_matches_a_fresh_compose_after_edits_undo_and_redo() {
        // AC-01, AC-06: each step is presented (so the cache is warm) and
        // must equal a fresh compose at zoom 1, whole canvas and a crop.
        let handle = layered();
        let steps = [
            r#"{"type":"pencil_stroke","layer":2,"size":6,"color":[0,0,0,255],"opacity":1,"points":[[5,5],[30,20]],"stroke_id":9}"#,
            r#"{"type":"pencil_stroke","layer":2,"size":6,"color":[0,0,0,255],"opacity":1,"points":[[30,20],[35,4]],"stroke_id":9}"#,
            "undo",
            "redo",
            r#"{"type":"set_layer_opacity","index":1,"opacity":0.3}"#,
            r#"{"type":"set_layer_blend_mode","index":1,"mode":"multiply"}"#,
            r#"{"type":"set_layer_visible","index":0,"visible":false}"#,
            "undo",
            r#"{"type":"move_layer","from":0,"to":2}"#,
            r#"{"type":"pencil_stroke","layer":0,"size":9,"color":[0,255,0,90],"opacity":1,"points":[[20,10]],"stroke_id":10}"#,
            "undo",
            "undo",
            r#"{"type":"resize_canvas","width":33,"height":41}"#,
            r#"{"type":"pencil_stroke","layer":1,"size":5,"color":[255,255,0,255],"opacity":1,"points":[[3,38]],"stroke_id":11}"#,
            "undo",
            "undo",
        ];
        composite_region(handle, 0, 0, 40, 30, 40, 30).expect("warm the cache");
        for step in steps {
            match step {
                "undo" => assert!(undo(handle).expect("undo")),
                "redo" => assert!(redo(handle).expect("redo")),
                json => apply_command(handle, json).expect(json),
            }
            let (w, h) = canvas(handle);
            let whole = composite_region(handle, 0, 0, w, h, w, h).expect("whole");
            assert_eq!(whole.0, fresh_compose(handle, 0, 0, w, h), "after {step}");
            let crop = composite_region(handle, 7, 3, 20, 11, 20, 11).expect("crop");
            assert_eq!(crop.0, fresh_compose(handle, 7, 3, 20, 11), "after {step}");
        }
    }

    #[test]
    fn composite_region_below_zoom_one_samples_the_pyramid_of_a_fresh_compose() {
        // AC-07: zoomed out, the presented pixels are the pyramid sample of
        // the current document, also after edits and an undo.
        let handle = layered();
        let views = [
            (0, 0, 40, 30, 13, 10),
            (5, 4, 30, 20, 11, 7),
            (0, 0, 40, 30, 39, 29),
        ];
        let fresh = |handle: u32, (x, y, w, h, ow, oh): (i32, i32, u32, u32, u32, u32)| {
            with_bus(handle, |bus| {
                let mut cache = pincel_image_core::RenderCache::new(&bus.document);
                Ok(cache
                    .region(&bus.document, Rect::new(x, y, w, h), ow, oh)
                    .into_raw())
            })
            .expect("fresh pyramid")
        };
        let stroke = r#"{"type":"pencil_stroke","layer":2,"size":7,"color":[0,0,0,255],"opacity":1,"points":[[8,8],[33,22]],"stroke_id":5}"#;
        for step in ["none", stroke, "undo"] {
            match step {
                "none" => {}
                "undo" => assert!(undo(handle).expect("undo")),
                json => apply_command(handle, json).expect(json),
            }
            for view @ (x, y, w, h, ow, oh) in views {
                let shown = composite_region(handle, x, y, w, h, ow, oh).expect("shown");
                assert_eq!(shown.0, fresh(handle, view), "{view:?} after {step}");
            }
        }
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;
    use pincel_image_core::codec::decode;

    #[test]
    fn export_tga_ico_and_image_to_png_round_trip() {
        let handle = insert(CommandBus::new(Document::new(5, 3).expect("doc")));
        let tga = export_tga(handle).expect("tga");
        let back = pincel_image_core::codec::decode_hinted(&tga, Some("a.tga")).expect("decode");
        assert_eq!((back.width(), back.height()), (5, 3));
        let ico = export_ico(handle).expect("ico");
        assert!(ico.starts_with(&[0, 0, 1, 0]));
        let png = image_to_png(&tga, "image/x-targa").expect("png");
        assert!(png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn export_bmp_and_export_tiff_write_the_composite() {
        let handle = insert(CommandBus::new(Document::new(5, 3).expect("doc")));
        let bmp = export_bmp(handle).expect("bmp");
        assert!(bmp.starts_with(b"BM"));
        let tiff = export_tiff(handle).expect("tiff");
        assert!(tiff.starts_with(b"II*\0") || tiff.starts_with(b"MM\0*"));
        for bytes in [&bmp, &tiff] {
            let back = decode(bytes).expect("decode");
            assert_eq!((back.width(), back.height()), (5, 3));
        }
    }
}

#[cfg(test)]
mod document_dpi_tests {
    use super::*;

    #[test]
    fn document_dpi_reports_the_stored_resolution() {
        let handle = create_document(4, 4).expect("doc");
        assert_eq!(document_dpi(handle).expect("dpi"), 96.0);
        let mut doc = Document::new(4, 4).expect("doc");
        doc.metadata.dpi = 300.0;
        let other = insert(CommandBus::new(doc));
        assert_eq!(document_dpi(other).expect("dpi"), 300.0);
        close_document(handle);
        close_document(other);
    }
}

#[cfg(test)]
mod document_count_tests {
    use super::*;

    #[test]
    fn document_count_follows_create_and_close() {
        let before = document_count();
        let a = create_document(4, 4).expect("a");
        let b = create_document(4, 4).expect("b");
        assert_eq!(document_count(), before + 2);
        close_document(a);
        assert_eq!(document_count(), before + 1);
        close_document(b);
        // Closing twice is a no-op.
        close_document(b);
        assert_eq!(document_count(), before);
    }
}

/// Effects through the handle API (`apply_effect` / `preview_effect`).
#[cfg(test)]
mod effect_handle_tests {
    use super::*;
    use pincel_image_core::SelectionMask;

    fn red() -> Color {
        Color::rgba(255, 0, 0, 255)
    }

    /// 4×4 document with red at (1, 1) on its only layer.
    fn painted() -> u32 {
        let mut doc = Document::new(4, 4).expect("doc");
        doc.active_layer_mut().pixels.set_pixel(1, 1, red());
        insert(CommandBus::new(doc))
    }

    fn layer_pixel(handle: u32, x: u32, y: u32) -> Color {
        with_bus(handle, |bus| {
            Ok(bus.document.layers()[0]
                .pixels
                .get_pixel(x, y)
                .expect("in bounds"))
        })
        .expect("read")
    }

    fn can_undo(handle: u32) -> bool {
        with_bus(handle, |bus| Ok(bus.history.can_undo())).expect("read")
    }

    #[test]
    fn preview_effect_shows_the_effect_and_leaves_document_and_history_untouched() {
        let handle = painted();
        let frame = preview_effect(handle, 0, "invert", &[]).expect("preview");
        let i = (4 + 1) * 4;
        assert_eq!(
            &frame.0[i..i + 4],
            &[0, 255, 255, 255],
            "preview is inverted"
        );
        assert_eq!(layer_pixel(handle, 1, 1), red(), "document unchanged");
        assert!(!can_undo(handle), "no undo step");
    }

    #[test]
    fn apply_effect_with_an_empty_selection_changes_nothing() {
        let handle = painted();
        with_bus(handle, |bus| {
            bus.document.selection = Some(SelectionMask::new_empty(4, 4));
            Ok(())
        })
        .expect("select");
        assert!(!apply_effect(handle, 0, "invert", &[]).expect("apply"));
        assert_eq!(layer_pixel(handle, 1, 1), red());
        assert!(!can_undo(handle), "no undo step");
    }

    /// Outline wire params: Outside, Square, thickness 1, blue at alpha 0x80.
    const OUTLINE: [f64; 4] = [0.0, 1.0, 1.0, 0x0000_FF80 as f64];

    fn blue() -> Color {
        Color::rgba(0, 0, 255, 0x80)
    }

    #[test]
    fn apply_effect_outline_is_one_undo_step_and_redo_reapplies_it() {
        let handle = painted();
        assert!(apply_effect(handle, 0, "outline", &OUTLINE).expect("apply"));
        assert_eq!(layer_pixel(handle, 0, 0), blue(), "neighbour, exact colour");
        assert_eq!(layer_pixel(handle, 2, 2), blue());
        assert_eq!(
            layer_pixel(handle, 3, 3),
            Color::TRANSPARENT,
            "not a neighbour"
        );
        assert_eq!(layer_pixel(handle, 1, 1), red());
        assert!(undo(handle).expect("undo"));
        assert_eq!(layer_pixel(handle, 0, 0), Color::TRANSPARENT);
        assert_eq!(layer_pixel(handle, 2, 2), Color::TRANSPARENT);
        assert!(!can_undo(handle), "the outline was one undo step");
        assert!(redo(handle).expect("redo"));
        assert_eq!(layer_pixel(handle, 2, 2), blue());
    }

    #[test]
    fn apply_effect_outline_blends_a_feathered_selection_by_coverage() {
        let handle = painted();
        with_bus(handle, |bus| {
            let mut mask = SelectionMask::new_empty(4, 4);
            mask.set(2, 1, 255);
            mask.set(2, 2, 128);
            bus.document.selection = Some(mask);
            Ok(())
        })
        .expect("select");
        assert!(apply_effect(handle, 0, "outline", &OUTLINE).expect("apply"));
        assert_eq!(layer_pixel(handle, 2, 1), blue(), "fully selected");
        // Rounded lerp from transparent at coverage 128 (masked_region).
        let half = |c: u8| ((u32::from(c) * 128 + 127) / 255) as u8;
        assert_eq!(
            layer_pixel(handle, 2, 2),
            Color::rgba(0, 0, half(255), half(0x80)),
            "half selected"
        );
        assert_eq!(layer_pixel(handle, 0, 0), Color::TRANSPARENT, "unselected");
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    const BEGIN: &str = r#"{"type":"begin_live_shape","layer":1,"shape":"rectangle",
        "points":[[1,1],[6,5]],"mode":"fill","fill_color":[255,0,0,255]}"#;

    /// A document with a live rectangle on its top layer.
    fn live_doc() -> u32 {
        let mut doc = Document::new(8, 8).expect("doc");
        doc.add_layer("Top").expect("layer");
        let handle = insert(CommandBus::new(doc));
        apply_command(handle, BEGIN).expect("begin");
        handle
    }

    fn is_live(handle: u32) -> bool {
        with_bus(handle, |bus| Ok(bus.live_object().is_some())).expect("bus")
    }

    fn depth(handle: u32) -> usize {
        with_bus(handle, |bus| Ok(bus.history.undo_depth())).expect("bus")
    }

    /// An editor action run on a document handle, given PNG bytes to paste.
    type Action = fn(u32, &[u8]);

    #[test]
    fn paths_beside_apply_command_commit_the_live_object_first() {
        let png = pincel_image_core::codec::to_png_bytes(&ImageBuffer::new_transparent(2, 2), 1)
            .expect("png");
        let actions: [(&str, Action); 4] = [
            ("effect", |h, _| {
                apply_effect(h, 1, "invert", &[]).expect("effect");
            }),
            ("paste", |h, png| {
                paste_layer(h, png, 0, 0, false).expect("paste")
            }),
            ("cut", |h, _| {
                cut_selection(h).expect("cut");
            }),
            ("active layer", |h, _| {
                set_active_layer(h, 0).expect("active")
            }),
        ];
        for (name, action) in actions {
            let handle = live_doc();
            assert!(is_live(handle), "{name}");
            action(handle, &png);
            assert!(!is_live(handle), "{name} commits");
            let committed = with_bus(handle, |bus| {
                Ok(bus.history.undo_depth() >= 1 && !bus.history.can_redo())
            })
            .expect("bus");
            assert!(committed, "{name}");
        }
    }

    #[test]
    fn save_and_export_while_live_do_not_commit() {
        let handle = live_doc();
        save_project(handle, 1).expect("save");
        export_png(handle, 1).expect("export");
        copy_selection(handle, true).expect("copy");
        composite(handle).expect("composite");
        assert!(is_live(handle));
        assert_eq!(depth(handle), 1);
    }
}

#[cfg(test)]
mod effect_region_tests {
    use super::*;

    /// A 6 × 5 document with a pattern on its single layer and a 3 × 2
    /// selection, open twice.
    fn twins() -> (u32, u32) {
        let make = || {
            let mut doc = Document::new(6, 5).expect("doc");
            for y in 0..5 {
                for x in 0..6 {
                    let v = (x * 40 + y * 13) as u8;
                    doc.active_layer_mut().pixels.set_pixel(
                        x,
                        y,
                        Color::rgba(v, 255 - v, v / 2, 255),
                    );
                }
            }
            insert(CommandBus::new(doc))
        };
        let (a, b) = (make(), make());
        for h in [a, b] {
            with_bus(h, |bus| {
                bus.document.selection = Some(pincel_image_core::SelectionMask::rectangle(
                    6,
                    5,
                    Rect {
                        x: 1,
                        y: 1,
                        w: 3,
                        h: 2,
                    },
                ));
                Ok(())
            })
            .expect("select");
        }
        (a, b)
    }

    fn pixels_and_depth(handle: u32) -> (Vec<u8>, usize) {
        with_bus(handle, |bus| {
            Ok((
                bus.document.layers()[0].pixels.data().to_vec(),
                bus.history.undo_depth(),
            ))
        })
        .expect("read")
    }

    #[test]
    fn apply_effect_region_matches_apply_effect() {
        for (name, params) in [("invert", vec![]), ("gaussian_blur", vec![2.0])] {
            let (a, b) = twins();
            assert!(apply_effect(a, 0, name, &params).expect("apply"));
            let region = effect_region_bytes(b, 0, name, &params).expect("region");
            assert!(apply_effect_region(b, 0, name, &params, &region).expect("commit"));
            assert_eq!(pixels_and_depth(a), pixels_and_depth(b), "{name}");
        }
    }

    #[test]
    fn effect_region_bytes_round_trips_through_decode() {
        let rect = Rect {
            x: -2,
            y: 3,
            w: 2,
            h: 1,
        };
        let pixels = ImageBuffer::from_raw(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]).expect("buf");
        let bytes = encode_region(Some((rect, pixels.clone())));
        let (r, p) = decode_region(&bytes).expect("decode").expect("some");
        assert_eq!((r.x, r.y, r.w, r.h), (-2, 3, 2, 1));
        assert_eq!(p.data(), pixels.data());
        assert!(
            decode_region(&encode_region(None))
                .expect("empty")
                .is_none()
        );
        assert!(decode_region(&bytes[..10]).is_err());
    }
}
