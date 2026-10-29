//! wasm-bindgen bindings exposing `pincel-image-core` to JavaScript (spec §17).
//!
//! Document state is owned in Rust: handles are opaque indices into a
//! thread-local arena, and JS only ever receives composited pixel buffers and
//! exported bytes (ADR-001). Commands are passed as JSON strings.

use pincel_effects::{EffectImage, EffectSpec};
use pincel_image_core::codec::{to_jpeg_bytes, to_png_bytes, to_webp_bytes};
use pincel_image_core::command::{
    AddLayer, Anchor, CanvasRotation, CommandBus, CropToSelection, DuplicateLayer, FlattenImage,
    FlipCanvas, LayerTransform, MergeDown, MergeVisible, MoveLayer, MoveSelection, PasteLayer,
    RemoveLayer, RenameLayer, ResizeCanvas, RotateCanvas, RotateLayer90, ScaleImage,
    SetLayerBlendMode, SetLayerLocked, SetLayerOpacity, SetLayerVisible, SetPixels, SetSelection,
    TransformLayer,
};
use pincel_image_core::{
    BlendMode, Brush, BrushShape, Color, DashPattern, Document, Eraser, EraserMode, Eyedropper,
    Fill, FillOptions, Gradient, GradientChannel, GradientKind, ImageBuffer, Interpolation, Move,
    PenSamples, Pencil, Point, PressureDynamics, Recolor, RecolorOrigins, Rect, SampleSize,
    SampleSource, SelectionMask, SelectionMode, Shape, ShapeMode, ShapeStyle, Shapes, Smudge,
    SmudgeCarry, Text, TextAlign, TextStyle, apply_mode, compose, compose_region, delete_selection,
    magic_wand, masked_region, scale,
};
use serde::{Deserialize, Serialize};
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

/// One drag's worth of tool memory, reset when a new `stroke_id` arrives.
#[derive(Default)]
struct StrokeScratch {
    id: u64,
    smudge: SmudgeCarry,
    recolor: RecolorOrigins,
}

impl StrokeScratch {
    /// Start over when this is a different drag than the one we remember.
    fn rewind(&mut self, id: u64) {
        if self.id != id {
            self.id = id;
            self.smudge.clear();
            self.recolor.clear();
        }
    }
}

/// Runs `f` against this document's scratch for `stroke_id`, starting a fresh
/// one when the drag has changed.
fn with_stroke<T>(handle: u32, stroke_id: u64, f: impl FnOnce(&mut StrokeScratch) -> T) -> T {
    STROKES.with(|strokes| {
        let mut strokes = strokes.borrow_mut();
        let scratch = strokes.entry(handle).or_default();
        scratch.rewind(stroke_id);
        f(scratch)
    })
}

/// Runs `f` against the bus for `handle`, mapping a missing handle to a JS error.
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
/// the active one, at the top-left (spec §5.4). With `expand`, a smaller
/// canvas grows to fit it first; otherwise the image is cropped. One undo step.
#[wasm_bindgen]
pub fn paste_layer(handle: u32, data: &[u8], expand: bool) -> Result<(), JsError> {
    let buffer =
        pincel_image_core::codec::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
    with_bus(handle, |bus| {
        bus.apply(Box::new(PasteLayer::new(buffer, expand)))
            .map_err(to_js)
    })
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

/// Default brush shape when JS omits it (hard round preserves M5 behavior).
fn default_shape() -> String {
    "hard_round".to_string()
}

/// Default brush hardness when JS omits it.
fn default_hardness() -> f32 {
    1.0
}

fn default_angle() -> f32 {
    pincel_image_core::DEFAULT_BRUSH_ANGLE
}

/// Pen azimuth per point for the core: a missing reading becomes NaN.
fn azimuth_samples(azimuth: &[Option<f32>]) -> Vec<f32> {
    azimuth.iter().map(|a| a.unwrap_or(f32::NAN)).collect()
}

/// Default eraser mode when JS omits it.
fn default_eraser_mode() -> String {
    "to_transparent".to_string()
}

/// Default eraser background when JS omits it: opaque white, matching the core
/// [`Eraser`] default rather than serde's transparent-black zero value.
fn default_background() -> [u8; 4] {
    [255, 255, 255, 255]
}

/// Default color sample source when JS omits it.
fn default_sample() -> String {
    "current_layer".to_string()
}

/// Default selection mode when JS omits it.
fn default_selection_mode() -> String {
    "replace".to_string()
}

/// Default resampling when JS omits it.
fn default_interpolation() -> String {
    "bilinear".to_string()
}

/// Default resize anchor when JS omits it.
fn default_anchor() -> String {
    "top_left".to_string()
}

/// Default shape mode when JS omits it.
fn default_shape_mode() -> String {
    "outline".to_string()
}

/// Default shape stroke width when JS omits it.
fn default_stroke_width() -> f32 {
    1.0
}

/// Default text size (px) when JS omits it.
fn default_text_size() -> f32 {
    24.0
}

/// Default text alignment when JS omits it.
fn default_text_align() -> String {
    "left".to_string()
}

/// Maps a text-align string to a [`TextAlign`], defaulting to left.
fn parse_text_align(s: &str) -> TextAlign {
    match s {
        "center" => TextAlign::Center,
        "right" => TextAlign::Right,
        _ => TextAlign::Left,
    }
}

/// Maps a brush-shape string to a [`BrushShape`], defaulting to hard round.
fn parse_shape(s: &str) -> BrushShape {
    match s {
        "soft_round" => BrushShape::SoftRound,
        "flat" => BrushShape::Flat,
        _ => BrushShape::HardRound,
    }
}

/// Maps an eraser-mode string to an [`EraserMode`], defaulting to transparent.
fn parse_eraser_mode(s: &str) -> EraserMode {
    match s {
        "to_background" => EraserMode::ToBackground,
        _ => EraserMode::ToTransparent,
    }
}

/// Maps a snake_case blend-mode string to a [`BlendMode`], defaulting to Normal.
fn parse_blend_mode(s: &str) -> BlendMode {
    match s {
        "multiply" => BlendMode::Multiply,
        "screen" => BlendMode::Screen,
        "overlay" => BlendMode::Overlay,
        "darken" => BlendMode::Darken,
        "lighten" => BlendMode::Lighten,
        "color_dodge" => BlendMode::ColorDodge,
        "color_burn" => BlendMode::ColorBurn,
        "hard_light" => BlendMode::HardLight,
        "soft_light" => BlendMode::SoftLight,
        "difference" => BlendMode::Difference,
        "exclusion" => BlendMode::Exclusion,
        _ => BlendMode::Normal,
    }
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

/// Maps a selection-mode string to a [`SelectionMode`], defaulting to Replace.
fn parse_selection_mode(s: &str) -> SelectionMode {
    match s {
        "add" => SelectionMode::Add,
        "subtract" => SelectionMode::Subtract,
        "intersect" => SelectionMode::Intersect,
        _ => SelectionMode::Replace,
    }
}

/// Maps a shape-mode string to a [`ShapeMode`], defaulting to outline.
fn parse_shape_mode(s: &str) -> ShapeMode {
    match s {
        "fill" => ShapeMode::Fill,
        "fill_and_outline" => ShapeMode::FillAndOutline,
        _ => ShapeMode::Outline,
    }
}

fn parse_gradient_kind(s: &str) -> GradientKind {
    match s {
        "reflected" => GradientKind::Reflected,
        "radial" => GradientKind::Radial,
        "conical" => GradientKind::Conical,
        "diamond" => GradientKind::Diamond,
        _ => GradientKind::Linear,
    }
}

fn parse_gradient_channel(s: &str) -> GradientChannel {
    match s {
        "transparency" => GradientChannel::Transparency,
        _ => GradientChannel::Color,
    }
}

/// Full strength when JS omits the gradient's opacity.
fn default_opacity() -> f32 {
    1.0
}

/// Default gradient kind when JS omits it.
fn default_gradient_kind() -> String {
    "linear".to_string()
}

/// Default gradient channel when JS omits it.
fn default_gradient_channel() -> String {
    "color".to_string()
}

/// Default shape dash pattern when JS omits it.
fn default_dash() -> String {
    "solid".to_string()
}

/// Maps a dash-pattern string to a [`DashPattern`], defaulting to solid.
fn parse_dash(s: &str) -> DashPattern {
    match s {
        "dashed" => DashPattern::Dashed,
        "dotted" => DashPattern::Dotted,
        _ => DashPattern::Solid,
    }
}

/// Maps an interpolation string to an [`Interpolation`], defaulting to bilinear.
fn parse_interpolation(s: &str) -> Interpolation {
    match s {
        "nearest" => Interpolation::Nearest,
        "bicubic" => Interpolation::Bicubic,
        _ => Interpolation::Bilinear,
    }
}

/// Maps a 9-grid anchor string to an [`Anchor`], defaulting to top-left.
fn parse_anchor(s: &str) -> Anchor {
    match s {
        "top_center" => Anchor::TopCenter,
        "top_right" => Anchor::TopRight,
        "center_left" => Anchor::CenterLeft,
        "center" => Anchor::Center,
        "center_right" => Anchor::CenterRight,
        "bottom_left" => Anchor::BottomLeft,
        "bottom_center" => Anchor::BottomCenter,
        "bottom_right" => Anchor::BottomRight,
        _ => Anchor::TopLeft,
    }
}

/// Maps a sample-source string to a [`SampleSource`], defaulting to the layer.
fn parse_sample(s: &str) -> SampleSource {
    match s {
        "all_layers" => SampleSource::AllLayers,
        _ => SampleSource::CurrentLayer,
    }
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

/// A JSON-serializable command from JS (spec §17 `SerializedCommand`).
///
/// The TypeScript mirror of this enum is generated from it via ts-rs
/// (ADR-014): `cargo test -p pincel-image-wasm export_command_spec` writes
/// `ui/src/modes/image/core/generated/CommandSpec.ts`, and the UI's command types are
/// checked against it at `pnpm check` time.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
enum CommandSpec {
    /// A pencil stroke over a polyline of `[x, y]` points.
    PencilStroke {
        layer: usize,
        size: u32,
        color: [u8; 4],
        opacity: f32,
        #[serde(default = "default_shape")]
        shape: String,
        #[serde(default = "default_hardness")]
        hardness: f32,
        points: Vec<[f32; 2]>,
        /// Pen pressure in `[0, 1]` per point (spec §17.4); empty or short
        /// means full pressure.
        #[serde(default)]
        pressure: Vec<f32>,
        /// Pressure scales the dab size.
        #[serde(default)]
        pressure_size: bool,
        /// Pressure scales the dab opacity.
        #[serde(default)]
        pressure_opacity: bool,
        /// Pen azimuth in radians per point (spec §17.4); `null` where the
        /// pen reported no tilt. Empty means no tilt readings.
        #[serde(default)]
        azimuth: Vec<Option<f32>>,
        /// Flat-nib angle in degrees.
        #[serde(default = "default_angle")]
        angle: f32,
        /// The flat nib follows the pen azimuth.
        #[serde(default)]
        tilt_angle: bool,
        /// Identifies the pointer drag; segments sharing it merge into one undo
        /// step. The UI assigns a fresh id per pointer-down.
        stroke_id: u64,
    },
    /// A recolor stroke: paints `to` only over pixels within `tolerance`
    /// of `from` (spec §5.4).
    RecolorStroke {
        layer: usize,
        size: u32,
        opacity: f32,
        #[serde(default = "default_shape")]
        shape: String,
        #[serde(default = "default_hardness")]
        hardness: f32,
        /// The color being replaced.
        #[serde(default)]
        from: [u8; 4],
        /// The color painted in its place.
        #[serde(default)]
        to: [u8; 4],
        /// How far a pixel may differ from `from` and still count, 0–255.
        #[serde(default)]
        tolerance: u8,
        points: Vec<[f32; 2]>,
        /// Pen input, as for `PencilStroke`.
        #[serde(default)]
        pressure: Vec<f32>,
        #[serde(default)]
        pressure_size: bool,
        #[serde(default)]
        pressure_opacity: bool,
        #[serde(default)]
        azimuth: Vec<Option<f32>>,
        #[serde(default = "default_angle")]
        angle: f32,
        #[serde(default)]
        tilt_angle: bool,
        stroke_id: u64,
    },
    /// A smudge stroke: carries color along the stroke (spec §5.4).
    SmudgeStroke {
        layer: usize,
        size: u32,
        opacity: f32,
        #[serde(default = "default_shape")]
        shape: String,
        #[serde(default = "default_hardness")]
        hardness: f32,
        /// How far a color is dragged, 0.0–1.0.
        #[serde(default = "default_opacity")]
        strength: f32,
        points: Vec<[f32; 2]>,
        /// Pen input, as for `PencilStroke`.
        #[serde(default)]
        pressure: Vec<f32>,
        #[serde(default)]
        pressure_size: bool,
        #[serde(default)]
        pressure_opacity: bool,
        #[serde(default)]
        azimuth: Vec<Option<f32>>,
        #[serde(default = "default_angle")]
        angle: f32,
        #[serde(default)]
        tilt_angle: bool,
        stroke_id: u64,
    },
    /// An eraser stroke over a polyline of `[x, y]` points.
    EraserStroke {
        layer: usize,
        size: u32,
        opacity: f32,
        #[serde(default = "default_shape")]
        shape: String,
        #[serde(default = "default_hardness")]
        hardness: f32,
        #[serde(default = "default_eraser_mode")]
        mode: String,
        #[serde(default = "default_background")]
        background: [u8; 4],
        points: Vec<[f32; 2]>,
        /// Pen pressure per point, as for `PencilStroke`.
        #[serde(default)]
        pressure: Vec<f32>,
        #[serde(default)]
        pressure_size: bool,
        #[serde(default)]
        pressure_opacity: bool,
        /// Pen azimuth and nib angle, as for `PencilStroke`.
        #[serde(default)]
        azimuth: Vec<Option<f32>>,
        #[serde(default = "default_angle")]
        angle: f32,
        #[serde(default)]
        tilt_angle: bool,
        stroke_id: u64,
    },
    /// A flood fill seeded at `[x, y]`.
    FillBucket {
        layer: usize,
        color: [u8; 4],
        opacity: f32,
        tolerance: u8,
        contiguous: bool,
        #[serde(default = "default_sample")]
        sample: String,
        x: f32,
        y: f32,
    },
    /// Translate a layer's contents by `(dx, dy)` pixels (the Move tool).
    TranslateLayer { layer: usize, dx: i32, dy: i32 },
    /// Move the selected pixels of `layer` by `(dx, dy)`, leaving
    /// transparency behind. Without a selection this moves the whole layer.
    TranslateSelection { layer: usize, dx: i32, dy: i32 },
    /// Move the selection outline by `(dx, dy)`, leaving the pixels alone.
    TranslateSelectionMask { dx: i32, dy: i32 },
    /// Erase the selected pixels of `layer` to transparent; the whole layer
    /// when no selection is active (Edit ▸ Clear / Delete key).
    DeleteSelection { layer: usize },
    /// Add a transparent layer above `active`.
    AddLayer { active: usize },
    /// Remove the layer at `index`.
    RemoveLayer { index: usize },
    /// Reorder the layer at `from` to position `to`.
    MoveLayer { from: usize, to: usize },
    /// Duplicate the layer at `index`, inserting the copy above it.
    DuplicateLayer { index: usize },
    /// Rename the layer at `index`.
    RenameLayer { index: usize, name: String },
    /// Set the opacity (0.0–1.0) of the layer at `index`.
    SetLayerOpacity { index: usize, opacity: f32 },
    /// Set the blend mode of the layer at `index` (snake_case string).
    SetLayerBlendMode { index: usize, mode: String },
    /// Show or hide the layer at `index`.
    SetLayerVisible { index: usize, visible: bool },
    /// Lock or unlock pixel edits on the layer at `index`.
    SetLayerLocked { index: usize, locked: bool },
    /// Merge the layer at `index` onto the layer below it.
    MergeDown { index: usize },
    /// Flatten all visible layers into one.
    MergeVisible,
    /// Flatten every layer onto an opaque white background.
    FlattenImage,
    /// Rectangular selection over `(x, y, w, h)` combined per `mode`.
    SelectRectangle {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        #[serde(default = "default_selection_mode")]
        mode: String,
        #[serde(default)]
        feather: u32,
    },
    /// Elliptical selection inscribed in `(x, y, w, h)` combined per `mode`.
    SelectEllipse {
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        #[serde(default = "default_selection_mode")]
        mode: String,
        #[serde(default)]
        feather: u32,
    },
    /// Polygonal selection over `points` ([x, y] each) combined per `mode`.
    SelectPolygon {
        points: Vec<[f32; 2]>,
        #[serde(default = "default_selection_mode")]
        mode: String,
        #[serde(default)]
        feather: u32,
    },
    /// Magic-wand selection seeded at `(x, y)` on `layer`, combined per `mode`.
    SelectWand {
        layer: usize,
        x: f32,
        y: f32,
        tolerance: u8,
        contiguous: bool,
        #[serde(default = "default_sample")]
        sample: String,
        #[serde(default = "default_selection_mode")]
        mode: String,
    },
    /// Select the whole canvas (spec §8.4 Select All).
    SelectAll,
    /// Clear the selection (spec §8.4 Deselect).
    Deselect,
    /// Invert the selection (spec §8.4 Invert).
    InvertSelection,
    /// Grow the selection by `radius` pixels (spec §8.4 Expand).
    ExpandSelection { radius: u32 },
    /// Shrink the selection by `radius` pixels (spec §8.4 Contract).
    ContractSelection { radius: u32 },
    /// Soften the selection edges by `radius` pixels (spec §8.4 Feather).
    FeatherSelection { radius: u32 },
    /// Flip or 180°-rotate the active layer (`op`: flip_h/flip_v/rotate_180).
    TransformLayer { layer: usize, op: String },
    /// Rotate the active layer 90° (counter-clockwise when `ccw`).
    // Explicit rename: rename_all would yield "rotate_layer90" (serde inserts
    // no separator before digits), but the UI-facing tag is "rotate_layer_90".
    #[serde(rename = "rotate_layer_90")]
    RotateLayer90 { layer: usize, ccw: bool },
    /// Flip the whole canvas (all layers) horizontally or vertically.
    FlipCanvas { horizontal: bool },
    /// Rotate the whole canvas (`rotation`: cw90/ccw90/rotate_180).
    RotateCanvas { rotation: String },
    /// Scale the whole image to `width` × `height` with `interpolation`.
    ScaleImage {
        width: u32,
        height: u32,
        #[serde(default = "default_interpolation")]
        interpolation: String,
    },
    /// Resize the canvas to `width` × `height`, placing content per `anchor`.
    ResizeCanvas {
        width: u32,
        height: u32,
        #[serde(default = "default_anchor")]
        anchor: String,
    },
    /// Crop the canvas to the current selection's bounding box (spec §10.6).
    CropToSelection,
    /// Rasterize a shape onto `layer` (spec §9.2 Shapes).
    ///
    /// `shape` selects the geometry: `line`/`rectangle`/`rounded_rectangle`/
    /// `ellipse` read `points` as `[a, b]` (endpoints or opposite corners);
    /// `polygon` reads `center`, `radius`, `sides` and `rotation`. The
    /// `corner_radius` applies to rounded rectangles only.
    DrawShape {
        layer: usize,
        shape: String,
        #[serde(default)]
        points: Vec<[f32; 2]>,
        #[serde(default)]
        corner_radius: f32,
        #[serde(default)]
        center: [f32; 2],
        #[serde(default)]
        radius: f32,
        #[serde(default)]
        sides: u32,
        #[serde(default)]
        rotation: f32,
        #[serde(default = "default_shape_mode")]
        mode: String,
        #[serde(default = "default_stroke_width")]
        stroke_width: f32,
        #[serde(default)]
        stroke_color: [u8; 4],
        #[serde(default)]
        fill_color: [u8; 4],
        #[serde(default)]
        anti_alias: bool,
        #[serde(default = "default_dash")]
        dash: String,
    },
    /// Paint a gradient ramp on `layer` along the drag `(x0, y0)` → `(x1, y1)`
    /// (spec §5.4). A zero-length drag or an empty selection is a no-op.
    DrawGradient {
        layer: usize,
        #[serde(default = "default_gradient_kind")]
        kind: String,
        #[serde(default = "default_gradient_channel")]
        channel: String,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        #[serde(default)]
        from: [u8; 4],
        #[serde(default)]
        to: [u8; 4],
        #[serde(default = "default_opacity")]
        opacity: f32,
    },
    /// Rasterize `text` onto `layer` at `(x, y)` using a `register_font` id
    /// (spec §9.2 Text; ADR-012). A no-op if the font id or geometry is invalid.
    DrawText {
        layer: usize,
        font_id: u32,
        text: String,
        x: f32,
        y: f32,
        #[serde(default = "default_text_size")]
        size: f32,
        #[serde(default)]
        color: [u8; 4],
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        anti_alias: bool,
        #[serde(default = "default_text_align")]
        align: String,
    },
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
        run_command(handle, bus, spec)
    })
}

/// Error message for an edit aimed at a locked layer (spec §3.2).
const LOCKED: &str = "layer is locked";

/// `true` when the layer at `index` exists and is locked against edits.
fn layer_locked(doc: &Document, index: usize) -> bool {
    doc.layers().get(index).is_some_and(|l| l.locked)
}

impl CommandSpec {
    /// The layer whose pixels this command writes, if it is a pixel edit.
    /// Property, structure and selection commands return `None`, so a
    /// locked layer can still be renamed, hidden, moved or unlocked.
    fn edited_layer(&self) -> Option<usize> {
        match self {
            Self::PencilStroke { layer, .. }
            | Self::EraserStroke { layer, .. }
            | Self::FillBucket { layer, .. }
            | Self::TranslateLayer { layer, .. }
            | Self::TranslateSelection { layer, .. }
            | Self::DrawGradient { layer, .. }
            | Self::RecolorStroke { layer, .. }
            | Self::SmudgeStroke { layer, .. }
            | Self::DeleteSelection { layer }
            | Self::TransformLayer { layer, .. }
            | Self::RotateLayer90 { layer, .. }
            | Self::DrawShape { layer, .. }
            | Self::DrawText { layer, .. } => Some(*layer),
            _ => None,
        }
    }
}

fn run_command(handle: u32, bus: &mut CommandBus, spec: CommandSpec) -> Result<(), JsError> {
    match spec {
        CommandSpec::PencilStroke {
            layer,
            size,
            color,
            opacity,
            shape,
            hardness,
            points,
            pressure,
            pressure_size,
            pressure_opacity,
            azimuth,
            angle,
            tilt_angle,
            stroke_id,
        } => {
            let brush = Brush::new(
                size,
                Color::rgba(color[0], color[1], color[2], color[3]),
                opacity,
            )
            .with_shape(parse_shape(&shape))
            .with_hardness(hardness)
            .with_dynamics(PressureDynamics {
                size: pressure_size,
                opacity: pressure_opacity,
            })
            .with_angle(angle)
            .with_follow_tilt(tilt_angle);
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            let azimuth = azimuth_samples(&azimuth);
            let pen = PenSamples {
                pressure: &pressure,
                azimuth: &azimuth,
            };
            match Pencil::new(brush).stroke_with_pen(layer, &pts, pen, &bus.document) {
                Some(cmd) => bus
                    .apply(Box::new(cmd.with_stroke(stroke_id)))
                    .map_err(to_js),
                None => Ok(()), // stroke missed the canvas — no-op
            }
        }
        CommandSpec::RecolorStroke {
            layer,
            size,
            opacity,
            shape,
            hardness,
            from,
            to,
            tolerance,
            points,
            pressure,
            pressure_size,
            pressure_opacity,
            azimuth,
            angle,
            tilt_angle,
            stroke_id,
        } => {
            let replacement = Color::rgba(to[0], to[1], to[2], to[3]);
            let brush = Brush::new(size, replacement, opacity)
                .with_shape(parse_shape(&shape))
                .with_hardness(hardness)
                .with_dynamics(PressureDynamics {
                    size: pressure_size,
                    opacity: pressure_opacity,
                })
                .with_angle(angle)
                .with_follow_tilt(tilt_angle);
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            let azimuth = azimuth_samples(&azimuth);
            let pen = PenSamples {
                pressure: &pressure,
                azimuth: &azimuth,
            };
            let tool = Recolor::new(
                brush,
                Color::rgba(from[0], from[1], from[2], from[3]),
                replacement,
                tolerance,
            );
            let committed = with_stroke(handle, stroke_id, |scratch| {
                tool.stroke_with_pen(layer, &pts, pen, &bus.document, &mut scratch.recolor)
            });
            match committed {
                Some(cmd) => bus
                    .apply(Box::new(cmd.with_stroke(stroke_id)))
                    .map_err(to_js),
                None => Ok(()), // stroke missed the canvas — no-op
            }
        }
        CommandSpec::SmudgeStroke {
            layer,
            size,
            opacity,
            shape,
            hardness,
            strength,
            points,
            pressure,
            pressure_size,
            pressure_opacity,
            azimuth,
            angle,
            tilt_angle,
            stroke_id,
        } => {
            let brush = Brush::new(size, Color::TRANSPARENT, opacity)
                .with_shape(parse_shape(&shape))
                .with_hardness(hardness)
                .with_dynamics(PressureDynamics {
                    size: pressure_size,
                    opacity: pressure_opacity,
                })
                .with_angle(angle)
                .with_follow_tilt(tilt_angle);
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            let azimuth = azimuth_samples(&azimuth);
            let pen = PenSamples {
                pressure: &pressure,
                azimuth: &azimuth,
            };
            let committed = with_stroke(handle, stroke_id, |scratch| {
                Smudge::new(brush, strength).stroke_with_pen(
                    layer,
                    &pts,
                    pen,
                    &bus.document,
                    &mut scratch.smudge,
                )
            });
            match committed {
                Some(cmd) => bus
                    .apply(Box::new(cmd.with_stroke(stroke_id)))
                    .map_err(to_js),
                None => Ok(()), // stroke missed the canvas — no-op
            }
        }
        CommandSpec::EraserStroke {
            layer,
            size,
            opacity,
            shape,
            hardness,
            mode,
            background,
            points,
            pressure,
            pressure_size,
            pressure_opacity,
            azimuth,
            angle,
            tilt_angle,
            stroke_id,
        } => {
            let brush = Brush::new(size, Color::TRANSPARENT, opacity)
                .with_shape(parse_shape(&shape))
                .with_hardness(hardness)
                .with_dynamics(PressureDynamics {
                    size: pressure_size,
                    opacity: pressure_opacity,
                })
                .with_angle(angle)
                .with_follow_tilt(tilt_angle);
            let eraser = Eraser::new(brush, parse_eraser_mode(&mode)).with_background(Color::rgba(
                background[0],
                background[1],
                background[2],
                background[3],
            ));
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            let azimuth = azimuth_samples(&azimuth);
            let pen = PenSamples {
                pressure: &pressure,
                azimuth: &azimuth,
            };
            match eraser.stroke_with_pen(layer, &pts, pen, &bus.document) {
                Some(cmd) => bus
                    .apply(Box::new(cmd.with_stroke(stroke_id)))
                    .map_err(to_js),
                None => Ok(()),
            }
        }
        CommandSpec::FillBucket {
            layer,
            color,
            opacity,
            tolerance,
            contiguous,
            sample,
            x,
            y,
        } => {
            let options = FillOptions {
                tolerance,
                contiguous,
                sample: parse_sample(&sample),
            };
            let fill = Fill::new(Color::rgba(color[0], color[1], color[2], color[3]), options)
                .with_opacity(opacity);
            match fill.fill(layer, Point::new(x, y), &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)).map_err(to_js),
                None => Ok(()),
            }
        }
        CommandSpec::TranslateLayer { layer, dx, dy } => {
            match Move.translate(layer, dx, dy, &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)).map_err(to_js),
                None => Ok(()),
            }
        }
        CommandSpec::TranslateSelection { layer, dx, dy } => {
            match Move.translate_selection(layer, dx, dy, &bus.document) {
                // The outline travels with its content, so a second drag
                // carries the patch on rather than the hole it left.
                Some(pixels) => bus
                    .apply(Box::new(MoveSelection::new(pixels, dx, dy)))
                    .map_err(to_js),
                None => Ok(()), // zero offset, empty selection or bad layer
            }
        }
        CommandSpec::TranslateSelectionMask { dx, dy } => {
            let Some(mask) = bus.document.selection.as_ref() else {
                return Ok(()); // nothing selected: nothing to move
            };
            if dx == 0 && dy == 0 {
                return Ok(());
            }
            let moved = mask.translated(dx, dy);
            bus.apply(Box::new(SetSelection::replace(moved)))
                .map_err(to_js)
        }
        CommandSpec::DeleteSelection { layer } => {
            match delete_selection(layer, &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)).map_err(to_js),
                None => Ok(()), // empty selection or invalid layer — no-op
            }
        }
        CommandSpec::AddLayer { active } => {
            bus.apply(Box::new(AddLayer::above(active))).map_err(to_js)
        }
        CommandSpec::RemoveLayer { index } => {
            bus.apply(Box::new(RemoveLayer::at(index))).map_err(to_js)
        }
        CommandSpec::MoveLayer { from, to } => {
            bus.apply(Box::new(MoveLayer::new(from, to))).map_err(to_js)
        }
        CommandSpec::DuplicateLayer { index } => bus
            .apply(Box::new(DuplicateLayer::at(index)))
            .map_err(to_js),
        CommandSpec::RenameLayer { index, name } => bus
            .apply(Box::new(RenameLayer::new(index, name)))
            .map_err(to_js),
        CommandSpec::SetLayerOpacity { index, opacity } => bus
            .apply(Box::new(SetLayerOpacity::new(index, opacity)))
            .map_err(to_js),
        CommandSpec::SetLayerBlendMode { index, mode } => bus
            .apply(Box::new(SetLayerBlendMode::new(
                index,
                parse_blend_mode(&mode),
            )))
            .map_err(to_js),
        CommandSpec::SetLayerVisible { index, visible } => bus
            .apply(Box::new(SetLayerVisible::new(index, visible)))
            .map_err(to_js),
        CommandSpec::SetLayerLocked { index, locked } => bus
            .apply(Box::new(SetLayerLocked::new(index, locked)))
            .map_err(to_js),
        CommandSpec::MergeDown { index } => {
            bus.apply(Box::new(MergeDown::at(index))).map_err(to_js)
        }
        CommandSpec::MergeVisible => bus.apply(Box::new(MergeVisible::new())).map_err(to_js),
        CommandSpec::FlattenImage => bus.apply(Box::new(FlattenImage::new())).map_err(to_js),
        CommandSpec::SelectRectangle {
            x,
            y,
            w,
            h,
            mode,
            feather,
        } => {
            let (cw, ch) = (bus.document.canvas.width(), bus.document.canvas.height());
            let mut shape = SelectionMask::rectangle(cw, ch, Rect::new(x, y, w, h));
            shape.feather(feather);
            apply_selection(bus, shape, parse_selection_mode(&mode), "Rectangle Select")
        }
        CommandSpec::SelectEllipse {
            x,
            y,
            w,
            h,
            mode,
            feather,
        } => {
            let (cw, ch) = (bus.document.canvas.width(), bus.document.canvas.height());
            let mut shape = SelectionMask::ellipse(cw, ch, Rect::new(x, y, w, h));
            shape.feather(feather);
            apply_selection(bus, shape, parse_selection_mode(&mode), "Ellipse Select")
        }
        CommandSpec::SelectPolygon {
            points,
            mode,
            feather,
        } => {
            let (cw, ch) = (bus.document.canvas.width(), bus.document.canvas.height());
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            let mut shape = SelectionMask::polygon(cw, ch, &pts);
            shape.feather(feather);
            apply_selection(bus, shape, parse_selection_mode(&mode), "Lasso Select")
        }
        CommandSpec::SelectWand {
            layer,
            x,
            y,
            tolerance,
            contiguous,
            sample,
            mode,
        } => {
            match magic_wand(
                &bus.document,
                layer,
                Point::new(x, y),
                tolerance,
                contiguous,
                parse_sample(&sample),
            ) {
                Some(shape) => {
                    apply_selection(bus, shape, parse_selection_mode(&mode), "Magic Wand")
                }
                None => Ok(()), // off-canvas seed — no-op
            }
        }
        CommandSpec::SelectAll => {
            let (cw, ch) = (bus.document.canvas.width(), bus.document.canvas.height());
            bus.apply(Box::new(
                SetSelection::replace(SelectionMask::new_full(cw, ch)).with_label("Select All"),
            ))
            .map_err(to_js)
        }
        CommandSpec::Deselect => bus.apply(Box::new(SetSelection::clear())).map_err(to_js),
        CommandSpec::InvertSelection => {
            let (cw, ch) = (bus.document.canvas.width(), bus.document.canvas.height());
            // No selection means everything is selected, so its inverse is empty.
            let mut mask = bus
                .document
                .selection
                .clone()
                .unwrap_or_else(|| SelectionMask::new_full(cw, ch));
            mask.invert();
            bus.apply(Box::new(
                SetSelection::replace(mask).with_label("Invert Selection"),
            ))
            .map_err(to_js)
        }
        CommandSpec::ExpandSelection { radius } => {
            modify_selection(bus, "Expand Selection", |m| m.expand(radius))
        }
        CommandSpec::ContractSelection { radius } => {
            modify_selection(bus, "Contract Selection", |m| m.contract(radius))
        }
        CommandSpec::FeatherSelection { radius } => {
            modify_selection(bus, "Feather Selection", |m| m.feather(radius))
        }
        CommandSpec::TransformLayer { layer, op } => {
            let op = match op.as_str() {
                "flip_v" => LayerTransform::FlipVertical,
                "rotate_180" => LayerTransform::Rotate180,
                _ => LayerTransform::FlipHorizontal,
            };
            bus.apply(Box::new(TransformLayer::new(layer, op)))
                .map_err(to_js)
        }
        CommandSpec::RotateLayer90 { layer, ccw } => bus
            .apply(Box::new(RotateLayer90::new(layer, ccw)))
            .map_err(to_js),
        CommandSpec::FlipCanvas { horizontal } => bus
            .apply(Box::new(FlipCanvas::new(horizontal)))
            .map_err(to_js),
        CommandSpec::RotateCanvas { rotation } => {
            let rot = match rotation.as_str() {
                "ccw90" => CanvasRotation::Ccw90,
                "rotate_180" => CanvasRotation::Rotate180,
                _ => CanvasRotation::Cw90,
            };
            bus.apply(Box::new(RotateCanvas::new(rot))).map_err(to_js)
        }
        CommandSpec::ScaleImage {
            width,
            height,
            interpolation,
        } => bus
            .apply(Box::new(ScaleImage::new(
                width,
                height,
                parse_interpolation(&interpolation),
            )))
            .map_err(to_js),
        CommandSpec::ResizeCanvas {
            width,
            height,
            anchor,
        } => bus
            .apply(Box::new(
                ResizeCanvas::new(width, height).with_anchor(parse_anchor(&anchor)),
            ))
            .map_err(to_js),
        CommandSpec::CropToSelection => bus.apply(Box::new(CropToSelection::new())).map_err(to_js),
        CommandSpec::DrawShape {
            layer,
            shape,
            points,
            corner_radius,
            center,
            radius,
            sides,
            rotation,
            mode,
            stroke_width,
            stroke_color,
            fill_color,
            anti_alias,
            dash,
        } => {
            let corner = |i: usize| points.get(i).map(|q| Point::new(q[0], q[1]));
            // line / rectangle / rounded_rectangle / ellipse take `points[0..2]`.
            let built = match shape.as_str() {
                "line" => corner(0).zip(corner(1)).map(|(a, b)| Shape::Line { a, b }),
                "rectangle" => corner(0)
                    .zip(corner(1))
                    .map(|(a, b)| Shape::Rectangle { a, b }),
                "rounded_rectangle" => {
                    corner(0)
                        .zip(corner(1))
                        .map(|(a, b)| Shape::RoundedRectangle {
                            a,
                            b,
                            radius: corner_radius,
                        })
                }
                "ellipse" => corner(0)
                    .zip(corner(1))
                    .map(|(a, b)| Shape::Ellipse { a, b }),
                "polygon" => Some(Shape::Polygon {
                    center: Point::new(center[0], center[1]),
                    radius,
                    sides,
                    rotation,
                }),
                _ => None,
            };
            let style = ShapeStyle {
                mode: parse_shape_mode(&mode),
                stroke_width,
                stroke_color: Color::rgba(
                    stroke_color[0],
                    stroke_color[1],
                    stroke_color[2],
                    stroke_color[3],
                ),
                fill_color: Color::rgba(fill_color[0], fill_color[1], fill_color[2], fill_color[3]),
                anti_alias,
                dash: parse_dash(&dash),
            };
            match built.and_then(|s| Shapes::new(s, style).draw(layer, &bus.document)) {
                Some(cmd) => bus.apply(Box::new(cmd)).map_err(to_js),
                None => Ok(()), // invalid geometry or off-canvas — no-op
            }
        }
        CommandSpec::DrawGradient {
            layer,
            kind,
            channel,
            x0,
            y0,
            x1,
            y1,
            from,
            to,
            opacity,
        } => {
            let tool = Gradient {
                kind: parse_gradient_kind(&kind),
                channel: parse_gradient_channel(&channel),
                from: Color::rgba(from[0], from[1], from[2], from[3]),
                to: Color::rgba(to[0], to[1], to[2], to[3]),
                opacity,
            };
            let a = Point::new(x0, y0);
            let b = Point::new(x1, y1);
            match tool.draw(layer, a, b, &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)).map_err(to_js),
                None => Ok(()), // zero-length drag, empty selection, bad layer
            }
        }
        CommandSpec::DrawText {
            layer,
            font_id,
            text,
            x,
            y,
            size,
            color,
            bold,
            italic,
            anti_alias,
            align,
        } => {
            let style = TextStyle {
                size,
                color: Color::rgba(color[0], color[1], color[2], color[3]),
                bold,
                italic,
                anti_alias,
                align: parse_text_align(&align),
            };
            let cmd = FONTS.with(|fonts| {
                let fonts = fonts.borrow();
                let bytes = fonts.get(font_id as usize)?;
                Text::new(text, Point::new(x, y), style).render(layer, &bus.document, bytes)
            });
            match cmd {
                Some(cmd) => bus.apply(Box::new(cmd)).map_err(to_js),
                None => Ok(()), // unknown font, empty text, or off-canvas — no-op
            }
        }
    }
}

/// Combines `shape` with the current selection per `mode` and applies it as an
/// undoable [`SetSelection`] labeled `label`.
fn apply_selection(
    bus: &mut CommandBus,
    shape: SelectionMask,
    mode: SelectionMode,
    label: &'static str,
) -> Result<(), JsError> {
    let after = apply_mode(bus.document.selection.as_ref(), shape, mode);
    bus.apply(Box::new(SetSelection::replace(after).with_label(label)))
        .map_err(to_js)
}

/// Applies an in-place modifier (`expand`/`contract`/`feather`) to the current
/// selection. A no-op when nothing is selected (the whole canvas is implied).
fn modify_selection(
    bus: &mut CommandBus,
    label: &'static str,
    f: impl FnOnce(&mut SelectionMask),
) -> Result<(), JsError> {
    let Some(mut mask) = bus.document.selection.clone() else {
        return Ok(());
    };
    f(&mut mask);
    bus.apply(Box::new(SetSelection::replace(mask).with_label(label)))
        .map_err(to_js)
}

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
mod command_spec_tests {
    use super::CommandSpec;

    /// Regression: serde's snake_case puts no separator before digits, so the
    /// variant carries an explicit rename to keep the documented wire tag.
    #[test]
    fn rotate_layer_90_wire_tag_deserializes() {
        let spec: CommandSpec =
            serde_json::from_str(r#"{"type":"rotate_layer_90","layer":0,"ccw":true}"#)
                .expect("tag must parse");
        assert!(matches!(
            spec,
            CommandSpec::RotateLayer90 {
                layer: 0,
                ccw: true
            }
        ));
    }

    #[test]
    fn edited_layer_covers_pixel_edits_only() {
        let parse = |json: &str| -> CommandSpec { serde_json::from_str(json).expect("parses") };
        // Every command that writes a layer's pixels names that layer, so the
        // lock gate in `apply_command` refuses it on a locked layer.
        for json in [
            r#"{"type":"pencil_stroke","layer":2,"size":4,"color":[0,0,0,255],"opacity":1,"points":[[1,1]],"stroke_id":1}"#,
            r#"{"type":"eraser_stroke","layer":2,"size":4,"opacity":1,"points":[[1,1]],"stroke_id":1}"#,
            r#"{"type":"recolor_stroke","layer":2,"size":4,"opacity":1,"points":[[1,1]],"stroke_id":1}"#,
            r#"{"type":"smudge_stroke","layer":2,"size":4,"opacity":1,"points":[[1,1]],"stroke_id":1}"#,
            r#"{"type":"fill_bucket","layer":2,"color":[0,0,0,255],"opacity":1,"tolerance":0,"contiguous":true,"x":1,"y":1}"#,
            r#"{"type":"translate_layer","layer":2,"dx":1,"dy":0}"#,
            r#"{"type":"translate_selection","layer":2,"dx":1,"dy":0}"#,
            r#"{"type":"delete_selection","layer":2}"#,
            r#"{"type":"transform_layer","layer":2,"op":"flip_h"}"#,
            r#"{"type":"rotate_layer_90","layer":2,"ccw":false}"#,
            r#"{"type":"draw_shape","layer":2,"shape":"line","points":[[0,0],[3,3]]}"#,
            r#"{"type":"draw_gradient","layer":2,"x0":0,"y0":0,"x1":3,"y1":0}"#,
            r#"{"type":"draw_text","layer":2,"font_id":0,"text":"A","x":0,"y":0}"#,
        ] {
            assert_eq!(parse(json).edited_layer(), Some(2), "{json}");
        }
        assert_eq!(
            parse(r#"{"type":"set_layer_locked","index":2,"locked":false}"#).edited_layer(),
            None
        );
        assert_eq!(
            parse(r#"{"type":"rename_layer","index":2,"name":"x"}"#).edited_layer(),
            None
        );
        assert_eq!(
            parse(r#"{"type":"set_layer_visible","index":2,"visible":false}"#).edited_layer(),
            None
        );
        assert_eq!(
            parse(r#"{"type":"move_layer","from":2,"to":0}"#).edited_layer(),
            None
        );
    }

    #[test]
    fn locked_layer_can_still_be_hidden_and_reordered() {
        let parse = |json: &str| -> CommandSpec { serde_json::from_str(json).expect("parses") };
        let mut doc = super::Document::new(2, 2).expect("doc");
        doc.add_layer("Top").expect("layer");
        let locked_id = doc.layers()[1].id;
        let mut bus = super::CommandBus::new(doc);
        bus.apply(Box::new(super::SetLayerLocked::new(1, true)))
            .expect("lock");

        for json in [
            r#"{"type":"set_layer_visible","index":1,"visible":false}"#,
            r#"{"type":"move_layer","from":1,"to":0}"#,
        ] {
            let spec = parse(json);
            // The gate in `apply_command` only refuses `edited_layer` targets.
            assert!(
                spec.edited_layer()
                    .is_none_or(|i| !super::layer_locked(&bus.document, i))
            );
            assert!(super::run_command(0, &mut bus, spec).is_ok());
        }
        let moved = &bus.document.layers()[0];
        assert_eq!(moved.id, locked_id, "the locked layer moved to the bottom");
        assert!(!moved.visible, "the locked layer is hidden");
        assert!(moved.locked, "and it is still locked");
    }

    #[test]
    fn select_all_selects_everything_in_one_undo_step() {
        let parse = |json: &str| -> CommandSpec { serde_json::from_str(json).expect("parses") };
        let mut bus = super::CommandBus::new(super::Document::new(6, 4).expect("doc"));
        let run = |bus: &mut super::CommandBus, json: &str| {
            assert!(super::run_command(0, bus, parse(json)).is_ok());
        };
        run(
            &mut bus,
            r#"{"type":"select_rectangle","x":1,"y":1,"w":2,"h":2,"mode":"replace"}"#,
        );
        let before = bus.document.selection.clone();
        let depth = bus.history.undo_depth();

        run(&mut bus, r#"{"type":"select_all"}"#);
        let all = bus.document.selection.as_ref().expect("a selection");
        assert_eq!(all.selected_count(), 6 * 4);
        assert!(all.data().iter().all(|&c| c == 255), "fully selected");
        assert_eq!(bus.history.undo_depth(), depth + 1, "one undo step");

        assert!(bus.undo().expect("undo"));
        assert_eq!(bus.document.selection, before);
    }

    #[test]
    fn invert_selection_is_one_undo_step() {
        let parse = |json: &str| -> CommandSpec { serde_json::from_str(json).expect("parses") };
        let mut bus = super::CommandBus::new(super::Document::new(6, 4).expect("doc"));
        let rect = r#"{"type":"select_rectangle","x":1,"y":1,"w":2,"h":2,"mode":"replace"}"#;
        assert!(super::run_command(0, &mut bus, parse(rect)).is_ok());
        let before = bus.document.selection.clone();
        let depth = bus.history.undo_depth();

        let invert = parse(r#"{"type":"invert_selection"}"#);
        assert!(super::run_command(0, &mut bus, invert).is_ok());
        let inverted = bus.document.selection.as_ref().expect("a selection");
        assert_eq!(inverted.selected_count(), 6 * 4 - 2 * 2);
        assert_eq!(inverted.coverage(1, 1), 0);
        assert_eq!(inverted.coverage(0, 0), 255);
        assert_eq!(bus.history.undo_depth(), depth + 1, "one undo step");

        assert!(bus.undo().expect("undo"));
        assert_eq!(bus.document.selection, before);
    }

    #[test]
    fn move_selection_moves_only_the_outline_in_one_undo_step() {
        let parse = |json: &str| -> CommandSpec { serde_json::from_str(json).expect("parses") };
        let mut bus = super::CommandBus::new(super::Document::new(8, 8).expect("doc"));
        // A dab inside the soon-selected square, so moved pixels would show.
        let dab = r#"{"type":"pencil_stroke","layer":0,"size":2,"color":[255,0,0,255],"opacity":1,"points":[[2.5,2.5]],"stroke_id":1}"#;
        assert!(super::run_command(0, &mut bus, parse(dab)).is_ok());
        assert!(
            bus.document.layers()[0]
                .pixels
                .get_pixel(2, 2)
                .is_some_and(|c| c.a > 0)
        );
        let rect = r#"{"type":"select_rectangle","x":1,"y":1,"w":3,"h":3,"mode":"replace"}"#;
        assert!(super::run_command(0, &mut bus, parse(rect)).is_ok());
        let pixels = bus.document.layers()[0].pixels.clone();
        let mask = bus.document.selection.clone().expect("a selection");
        let depth = bus.history.undo_depth();

        let shift = parse(r#"{"type":"translate_selection_mask","dx":3,"dy":2}"#);
        assert_eq!(shift.edited_layer(), None, "writes no pixels");
        assert!(super::run_command(0, &mut bus, shift).is_ok());
        assert_eq!(
            bus.document.selection.as_ref(),
            Some(&mask.translated(3, 2)),
            "the outline moved by (3, 2)"
        );
        assert_eq!(bus.document.layers()[0].pixels, pixels, "pixels unchanged");
        assert_eq!(bus.history.undo_depth(), depth + 1, "one undo step");

        assert!(bus.undo().expect("undo"));
        assert_eq!(bus.document.selection, Some(mask));
    }

    #[test]
    fn stroke_pen_fields_default_and_accept_null_azimuth() {
        let old: CommandSpec = serde_json::from_str(
            r#"{"type":"pencil_stroke","layer":0,"size":4,"color":[0,0,0,255],"opacity":1,"points":[[1,1]],"stroke_id":1}"#,
        )
        .expect("pre-tilt JSON still parses");
        assert!(matches!(
            old,
            CommandSpec::PencilStroke { angle, tilt_angle: false, ref azimuth, .. }
                if angle == pincel_image_core::DEFAULT_BRUSH_ANGLE && azimuth.is_empty()
        ));
        let tilted: CommandSpec = serde_json::from_str(
            r#"{"type":"eraser_stroke","layer":0,"size":4,"opacity":1,"points":[[1,1],[2,2]],"azimuth":[0.5,null],"angle":90,"tilt_angle":true,"stroke_id":2}"#,
        )
        .expect("tilt JSON parses");
        let CommandSpec::EraserStroke { azimuth, angle, .. } = tilted else {
            panic!("eraser stroke");
        };
        assert_eq!(angle, 90.0);
        let samples = super::azimuth_samples(&azimuth);
        assert_eq!(samples[0], 0.5);
        assert!(samples[1].is_nan());
    }

    #[test]
    fn layer_locked_reads_the_flag() {
        let doc = super::Document::new(2, 2).expect("doc");
        let mut bus = super::CommandBus::new(doc);
        assert!(!super::layer_locked(&bus.document, 0));
        bus.apply(Box::new(super::SetLayerLocked::new(0, true)))
            .expect("lock");
        assert!(super::layer_locked(&bus.document, 0));
        assert!(!super::layer_locked(&bus.document, 9));
    }
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

    #[test]
    fn paste_layer_expands_canvas_and_undo_restores_it() {
        let handle = insert(CommandBus::new(Document::new(4, 4).expect("doc")));
        let png = to_png_bytes(&ImageBuffer::new_transparent(6, 5), 1).expect("png");
        paste_layer(handle, &png, true).expect("paste");
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
