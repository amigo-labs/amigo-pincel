//! wasm-bindgen bindings exposing `pincel-image-core` to JavaScript (spec §17).
//!
//! Document state is owned in Rust: handles are opaque indices into a
//! thread-local arena, and JS only ever receives composited pixel buffers and
//! exported bytes (ADR-001). Commands are passed as JSON strings.

use pincel_effects::{EffectImage, EffectSpec};
use pincel_image_core::codec::{
    to_bmp_bytes, to_jpeg_bytes, to_png_bytes, to_tiff_bytes, to_webp_bytes,
};
use pincel_image_core::command::spec::{
    CommandSpec, StrokeScratch, layer_locked, parse_sample, run_command,
};
use pincel_image_core::command::{CommandBus, PasteLayer, SetPixels};
use pincel_image_core::{
    BlendMode, Color, Document, Eyedropper, ImageBuffer, Interpolation, Point, Rect, SampleSize,
    compose, compose_region, copy_selection as copy_selection_pixels, delete_selection,
    masked_region, scale,
};
use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use wasm_bindgen::Clamped;
use wasm_bindgen::prelude::*;

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
    /// Registered font byte blobs, indexed by font id (see `register_font`).
    /// The Text tool re-parses these per commit (ADR-012), so JS uploads each
    /// face once and references it by id rather than passing bytes per command.
    static FONTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

/// Installs a panic hook that logs Rust panics to the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Registers a TrueType/OpenType font and returns its id for use in `DrawText`
/// commands (spec §9.2 Text; ADR-012, Option B — the caller supplies the font).
///
/// Re-registering byte-identical data returns the existing id, so repeated
/// setup (e.g. per document open) does not accumulate copies.
#[wasm_bindgen]
pub fn register_font(data: &[u8]) -> u32 {
    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        if let Some(id) = fonts.iter().position(|f| f == data) {
            return id as u32;
        }
        fonts.push(data.to_vec());
        (fonts.len() - 1) as u32
    })
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

/// Opens an encoded image (PNG/JPEG/WebP/BMP/GIF/TIFF) as a single-layer
/// document. `mime_type` is accepted for API parity but format is auto-detected.
#[wasm_bindgen]
pub fn open_image(data: &[u8], _mime_type: &str) -> Result<u32, JsError> {
    let buffer =
        pincel_image_core::codec::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
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
        pincel_image_core::codec::to_ora_bytes(&bus.document, compression)
            .map_err(|e| JsError::new(&e.to_string()))
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
    let target = spec.edited_layer();
    with_bus(handle, |bus| {
        if target.is_some_and(|i| layer_locked(&bus.document, i)) {
            return Err(JsError::new(LOCKED));
        }
        STROKES.with(|strokes| {
            let mut strokes = strokes.borrow_mut();
            let scratch = strokes.entry(handle).or_default();
            FONTS.with(|fonts| {
                let fonts = fonts.borrow();
                let faces: Vec<&[u8]> = fonts.iter().map(Vec::as_slice).collect();
                run_command(bus, spec, scratch, &faces).map_err(to_js)
            })
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
#[wasm_bindgen]
pub fn set_active_layer(handle: u32, index: usize) -> Result<(), JsError> {
    with_bus(handle, |bus| {
        bus.document.set_active_layer(index).map_err(to_js)
    })
}

/// Reads the pixels of the layer at `index` into an [`EffectImage`].
fn layer_effect_image(doc: &Document, index: usize) -> Result<(EffectImage, u32, u32), JsError> {
    let layer = doc
        .layers()
        .get(index)
        .ok_or_else(|| JsError::new("layer index out of bounds"))?;
    let (w, h) = (layer.pixels.width(), layer.pixels.height());
    let img = EffectImage::from_rgba8(w, h, layer.pixels.data().to_vec())
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok((img, w, h))
}

/// Runs the effect `name` with the positional `params` (the shared
/// `pincel_effects::EffectSpec` wire format, also used by Pixel mode — see
/// `EFFECT_NAMES` and `ui/src/lib/effects/catalog.ts`) on the layer at
/// `layer`, limited to the selection, and commits it as one undoable
/// `SetPixels`. Returns `false` (no undo step) when the selection is empty.
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
        let Some((region, after)) = effect_region(&bus.document, layer, &spec)? else {
            return Ok(false);
        };
        let cmd = SetPixels::new(layer, region, after).with_label(spec.label());
        bus.apply(Box::new(cmd)).map_err(to_js)?;
        Ok(true)
    })
}

/// Runs `spec` on the layer at `layer` and keeps the result inside the
/// selection (spec §8.1): the changed region and its new pixels, or `None`
/// when the selection is empty.
fn effect_region(
    doc: &Document,
    layer: usize,
    spec: &EffectSpec,
) -> Result<Option<(Rect, ImageBuffer)>, JsError> {
    let (src, w, h) = layer_effect_image(doc, layer)?;
    let after = ImageBuffer::from_raw(w, h, spec.run(&src).into_raw()).map_err(to_js)?;
    let before = &doc.layers()[layer].pixels;
    Ok(masked_region(before, &after, doc.selection.as_ref()))
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
        let region = effect_region(&bus.document, layer, &spec)?;
        let mut layers = bus.document.layers().to_vec();
        if let Some((rect, after)) = region {
            layers[layer]
                .pixels
                .paste_region(rect, &after)
                .map_err(to_js)?;
        }
        Ok(Clamped(compose(&layers).into_raw()))
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

/// Composites only the canvas rect `(x, y, w, h)` (clipped to the canvas)
/// and resamples it to `out_w × out_h` — the UI's viewport renderer, so
/// a canvas larger than any browser canvas still displays (spec §6.4).
/// Pass the clipped size as `out_w` / `out_h` for 1:1 pixels; smaller
/// sizes downscale bilinearly (zoomed-out views). Returns RGBA8 rows of
/// the output size; an empty region yields an empty array.
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
        let region = compose_region(bus.document.layers(), Rect::new(x, y, w, h));
        if region.width() == 0 || region.height() == 0 {
            return Ok(Clamped(Vec::new()));
        }
        let (ow, oh) = (
            out_w.clamp(1, region.width()),
            out_h.clamp(1, region.height()),
        );
        let out = if (ow, oh) == (region.width(), region.height()) {
            region
        } else {
            scale(&region, ow, oh, Interpolation::Bilinear)
        };
        Ok(Clamped(out.into_raw()))
    })
}

/// Exports the flattened composite as PNG bytes. `compression` is 0–9.
#[wasm_bindgen]
pub fn export_png(handle: u32, compression: u8) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        to_png_bytes(&compose(bus.document.layers()), compression)
            .map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Exports the flattened composite as JPEG bytes. `quality` is 1–100.
#[wasm_bindgen]
pub fn export_jpeg(handle: u32, quality: u8) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        to_jpeg_bytes(&compose(bus.document.layers()), quality)
            .map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Exports the flattened composite as lossless WebP bytes (ADR-007).
#[wasm_bindgen]
pub fn export_webp(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        to_webp_bytes(&compose(bus.document.layers())).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Exports the flattened composite as 32-bit RGBA BMP bytes.
#[wasm_bindgen]
pub fn export_bmp(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        to_bmp_bytes(&compose(bus.document.layers())).map_err(|e| JsError::new(&e.to_string()))
    })
}

/// Exports the flattened composite as lossless RGBA TIFF bytes.
#[wasm_bindgen]
pub fn export_tiff(handle: u32) -> Result<Vec<u8>, JsError> {
    with_bus(handle, |bus| {
        to_tiff_bytes(&compose(bus.document.layers())).map_err(|e| JsError::new(&e.to_string()))
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
        let layers = bus
            .document
            .layers()
            .iter()
            .map(|l| LayerInfo {
                id: l.id.to_string(),
                name: l.name.clone(),
                opacity: l.opacity,
                blend_mode: blend_mode_str(l.blend_mode).to_string(),
                visible: l.visible,
                locked: l.locked,
            })
            .collect();
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

/// Returns a 32×32 RGBA8 thumbnail of the layer with `layer_id` as a
/// `Uint8ClampedArray`, ready to wrap in an `ImageData` (spec §5.3, §17).
#[wasm_bindgen]
pub fn get_layer_thumbnail(handle: u32, layer_id: &str) -> Result<Clamped<Vec<u8>>, JsError> {
    with_bus(handle, |bus| {
        let layer = bus
            .document
            .layers()
            .iter()
            .find(|l| l.id.to_string() == layer_id)
            .ok_or_else(|| JsError::new("layer not found"))?;
        Ok(Clamped(thumbnail(&layer.pixels)))
    })
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

#[cfg(test)]
mod ts_bindings {
    use super::CommandSpec;
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
mod region_tests {
    use super::*;

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
}

#[cfg(test)]
mod export_tests {
    use super::*;
    use pincel_image_core::codec::decode;

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
