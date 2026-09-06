//! wasm-bindgen bindings exposing `fineliner-core` to JavaScript (spec §17).
//!
//! Document state is owned in Rust: handles are opaque indices into a
//! thread-local arena, and JS only ever receives composited pixel buffers and
//! exported bytes (ADR-001). Commands are passed as JSON strings.

use fineliner_core::codec::{to_jpeg_bytes, to_png_bytes, to_webp_bytes};
use fineliner_core::command::{
    AddLayer, Anchor, CanvasRotation, CommandBus, CropToSelection, DuplicateLayer, FlattenImage,
    FlipCanvas, LayerTransform, MergeDown, MergeVisible, MoveLayer, RemoveLayer, RenameLayer,
    ResizeCanvas, RotateCanvas, RotateLayer90, ScaleImage, SetLayerBlendMode, SetLayerLocked,
    SetLayerOpacity, SetLayerVisible, SetPixels, SetSelection, TransformLayer,
};
use fineliner_core::{
    apply_mode, compose, delete_selection, magic_wand, BlendMode, Brush, BrushShape, Color,
    DashPattern, Document, Eraser, EraserMode, Eyedropper, Fill, FillOptions, ImageBuffer,
    Interpolation, Move, Pencil, Point, Rect, SampleSize, SampleSource, SelectionMask,
    SelectionMode, Shape, ShapeMode, ShapeStyle, Shapes, Text, TextAlign, TextStyle,
};
use pincel_effects::adjust::{
    BrightnessContrast, ColorBalance, CurveChannel, Curves, Grayscale, GrayscaleMethod,
    HueSaturation, Invert, Levels, LevelsChannel, Posterize, Threshold,
};
use pincel_effects::blur::{BoxBlur, GaussianBlur, MotionBlur, RadialBlur, RadialKind};
use pincel_effects::distort::{EdgeAlgorithm, EdgeDetect, Emboss, Relief};
use pincel_effects::noise::{AddNoise, NoiseChannels, NoiseType, ReduceNoise};
use pincel_effects::sharpen::{Sharpen, UnsharpMask};
use pincel_effects::{Effect, EffectImage};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;

thread_local! {
    /// Open documents by handle. Handles are never reused, so a stale handle
    /// held by JS errors instead of silently aliasing a newer document.
    static DOCUMENTS: RefCell<HashMap<u32, CommandBus>> = RefCell::new(HashMap::new());
    /// Next handle to hand out; monotonically increasing.
    static NEXT_HANDLE: Cell<u32> = const { Cell::new(0) };
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
    let buffer = fineliner_core::codec::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
    let doc = Document::from_pixels(buffer).map_err(to_js)?;
    Ok(insert(CommandBus::new(doc)))
}

/// Releases the document for `handle`. The handle is never reused.
#[wasm_bindgen]
pub fn close_document(handle: u32) {
    DOCUMENTS.with(|docs| {
        docs.borrow_mut().remove(&handle);
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
/// (ADR-014): `cargo test -p fineliner-wasm export_command_spec` writes
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
        /// Identifies the pointer drag; segments sharing it merge into one undo
        /// step. The UI assigns a fresh id per pointer-down.
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
    with_bus(handle, |bus| match spec {
        CommandSpec::PencilStroke {
            layer,
            size,
            color,
            opacity,
            shape,
            hardness,
            points,
            stroke_id,
        } => {
            let brush = Brush::new(
                size,
                Color::rgba(color[0], color[1], color[2], color[3]),
                opacity,
            )
            .with_shape(parse_shape(&shape))
            .with_hardness(hardness);
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            match Pencil::new(brush).stroke(layer, &pts, &bus.document) {
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
            stroke_id,
        } => {
            let brush = Brush::new(size, Color::TRANSPARENT, opacity)
                .with_shape(parse_shape(&shape))
                .with_hardness(hardness);
            let eraser = Eraser::new(brush, parse_eraser_mode(&mode)).with_background(Color::rgba(
                background[0],
                background[1],
                background[2],
                background[3],
            ));
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            match eraser.stroke(layer, &pts, &bus.document) {
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
    })
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

/// A JSON-serializable effect from JS (spec §17 `SerializedEffect`).
///
/// Like [`CommandSpec`], the TypeScript mirror is generated via ts-rs (ADR-014);
/// the sub-choices (radial kind, edge algorithm, noise type/channels) cross the
/// boundary as snake_case strings, matching the tool-option convention.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
enum EffectSpec {
    /// Gaussian blur, σ = radius / 3.
    GaussianBlur { radius: f32 },
    /// Box blur over `width` × `height` (odd) pixels.
    BoxBlur { width: u32, height: u32 },
    /// Motion blur along a `distance`-long line at `angle` degrees.
    MotionBlur { distance: f32, angle: f32 },
    /// Radial blur about `(center_x, center_y)`; `kind` is `spin` or `zoom`.
    RadialBlur {
        amount: f32,
        center_x: f32,
        center_y: f32,
        kind: String,
    },
    /// Fixed one-step sharpen (no parameters).
    Sharpen,
    /// Unsharp mask: amount (percent), radius (px), threshold (0–255).
    UnsharpMask {
        amount: f32,
        radius: f32,
        threshold: f32,
    },
    /// Emboss lit from `angle`/`elevation` with `relief` steepness.
    Emboss {
        angle: f32,
        elevation: f32,
        relief: f32,
    },
    /// Edge detect; `algorithm` is `sobel`, `prewitt` or `laplacian`.
    EdgeDetect { algorithm: String, amount: f32 },
    /// Colour-preserving relief along `angle` with strength `amount`.
    Relief { angle: f32, amount: f32 },
    /// Add noise; `noise_type` is `uniform`/`gaussian`, `channels` is
    /// `rgb`/`monochromatic`.
    AddNoise {
        amount: u32,
        noise_type: String,
        channels: String,
        seed: u64,
    },
    /// Reduce noise via a median filter of the given radius.
    ReduceNoise { radius: u32 },
    /// Brightness/contrast (−150..150 each); `enhanced` uses an S-curve.
    BrightnessContrast {
        brightness: f32,
        contrast: f32,
        enhanced: bool,
    },
    /// Hue/saturation/lightness in HSL; `colorize` sets a single hue.
    HueSaturation {
        hue: f32,
        saturation: f32,
        lightness: f32,
        colorize: bool,
    },
    /// Per-channel tone curve; `channel` is composite/red/green/blue/alpha.
    Curves {
        channel: String,
        points: Vec<[f32; 2]>,
    },
    /// Levels remap; `channel` is composite/red/green/blue.
    Levels {
        channel: String,
        in_black: f32,
        in_white: f32,
        gamma: f32,
        out_black: f32,
        out_white: f32,
    },
    /// Color balance shifts per tone range (each `[cr, mg, yb]`, −100..100).
    ColorBalance {
        shadows: [f32; 3],
        midtones: [f32; 3],
        highlights: [f32; 3],
        preserve_luminosity: bool,
    },
    /// Invert RGB (no parameters).
    Invert,
    /// Grayscale; `method` is luminosity/average/bt709/channel_mixer.
    Grayscale { method: String, mixer: [f32; 3] },
    /// Posterize to N levels (2–255).
    Posterize { levels: u32 },
    /// Threshold by luminance (0–255).
    Threshold { threshold: u8 },
}

/// Parses a radial-blur kind string; defaults to Spin.
fn parse_radial_kind(s: &str) -> RadialKind {
    match s {
        "zoom" => RadialKind::Zoom,
        _ => RadialKind::Spin,
    }
}

/// Parses an edge-detect algorithm string; defaults to Sobel.
fn parse_edge_algorithm(s: &str) -> EdgeAlgorithm {
    match s {
        "prewitt" => EdgeAlgorithm::Prewitt,
        "laplacian" => EdgeAlgorithm::Laplacian,
        _ => EdgeAlgorithm::Sobel,
    }
}

/// Parses a noise distribution string; defaults to Uniform.
fn parse_noise_type(s: &str) -> NoiseType {
    match s {
        "gaussian" => NoiseType::Gaussian,
        _ => NoiseType::Uniform,
    }
}

/// Parses a noise channels string; defaults to Rgb.
fn parse_noise_channels(s: &str) -> NoiseChannels {
    match s {
        "monochromatic" => NoiseChannels::Monochromatic,
        _ => NoiseChannels::Rgb,
    }
}

/// Parses a curve channel string; defaults to Composite.
fn parse_curve_channel(s: &str) -> CurveChannel {
    match s {
        "red" => CurveChannel::Red,
        "green" => CurveChannel::Green,
        "blue" => CurveChannel::Blue,
        "alpha" => CurveChannel::Alpha,
        _ => CurveChannel::Composite,
    }
}

/// Parses a levels channel string; defaults to Composite.
fn parse_levels_channel(s: &str) -> LevelsChannel {
    match s {
        "red" => LevelsChannel::Red,
        "green" => LevelsChannel::Green,
        "blue" => LevelsChannel::Blue,
        _ => LevelsChannel::Composite,
    }
}

/// Parses a grayscale method string; defaults to Luminosity.
fn parse_grayscale_method(s: &str) -> GrayscaleMethod {
    match s {
        "average" => GrayscaleMethod::Average,
        "bt709" => GrayscaleMethod::Bt709,
        "channel_mixer" => GrayscaleMethod::ChannelMixer,
        _ => GrayscaleMethod::Luminosity,
    }
}

/// Runs the effect described by `spec` over `src`, returning the result buffer.
fn run_effect(spec: &EffectSpec, src: &EffectImage) -> EffectImage {
    match spec {
        EffectSpec::GaussianBlur { radius } => GaussianBlur::new(*radius).apply(src),
        EffectSpec::BoxBlur { width, height } => BoxBlur::new(*width, *height).apply(src),
        EffectSpec::MotionBlur { distance, angle } => MotionBlur::new(*distance, *angle).apply(src),
        EffectSpec::RadialBlur {
            amount,
            center_x,
            center_y,
            kind,
        } => RadialBlur::new(*amount, *center_x, *center_y, parse_radial_kind(kind)).apply(src),
        EffectSpec::Sharpen => Sharpen.apply(src),
        EffectSpec::UnsharpMask {
            amount,
            radius,
            threshold,
        } => UnsharpMask::new(*amount, *radius, *threshold).apply(src),
        EffectSpec::Emboss {
            angle,
            elevation,
            relief,
        } => Emboss::new(*angle, *elevation, *relief).apply(src),
        EffectSpec::EdgeDetect { algorithm, amount } => {
            EdgeDetect::new(parse_edge_algorithm(algorithm), *amount).apply(src)
        }
        EffectSpec::Relief { angle, amount } => Relief::new(*angle, *amount).apply(src),
        EffectSpec::AddNoise {
            amount,
            noise_type,
            channels,
            seed,
        } => AddNoise::new(
            *amount,
            parse_noise_type(noise_type),
            parse_noise_channels(channels),
            *seed,
        )
        .apply(src),
        EffectSpec::ReduceNoise { radius } => ReduceNoise::new(*radius).apply(src),
        EffectSpec::BrightnessContrast {
            brightness,
            contrast,
            enhanced,
        } => {
            let bc = BrightnessContrast::new(*brightness, *contrast);
            let bc = if *enhanced { bc.enhanced() } else { bc };
            bc.apply(src)
        }
        EffectSpec::HueSaturation {
            hue,
            saturation,
            lightness,
            colorize,
        } => {
            let hs = HueSaturation::new(*hue, *saturation, *lightness);
            let hs = if *colorize { hs.colorize() } else { hs };
            hs.apply(src)
        }
        EffectSpec::Curves { channel, points } => {
            Curves::new(parse_curve_channel(channel), points.clone()).apply(src)
        }
        EffectSpec::Levels {
            channel,
            in_black,
            in_white,
            gamma,
            out_black,
            out_white,
        } => Levels::new(
            parse_levels_channel(channel),
            *in_black,
            *in_white,
            *gamma,
            *out_black,
            *out_white,
        )
        .apply(src),
        EffectSpec::ColorBalance {
            shadows,
            midtones,
            highlights,
            preserve_luminosity,
        } => {
            let cb = ColorBalance::new(*shadows, *midtones, *highlights);
            let cb = if *preserve_luminosity {
                cb.preserve_luminosity()
            } else {
                cb
            };
            cb.apply(src)
        }
        EffectSpec::Invert => Invert.apply(src),
        EffectSpec::Grayscale { method, mixer } => {
            let g = match parse_grayscale_method(method) {
                GrayscaleMethod::ChannelMixer => Grayscale::mixer(*mixer),
                m => Grayscale::new(m),
            };
            g.apply(src)
        }
        EffectSpec::Posterize { levels } => Posterize::new(*levels).apply(src),
        EffectSpec::Threshold { threshold } => Threshold::new(*threshold).apply(src),
    }
}

/// Human-readable undo label for an effect.
fn effect_label(spec: &EffectSpec) -> &'static str {
    match spec {
        EffectSpec::GaussianBlur { .. } => "Gaussian Blur",
        EffectSpec::BoxBlur { .. } => "Box Blur",
        EffectSpec::MotionBlur { .. } => "Motion Blur",
        EffectSpec::RadialBlur { .. } => "Radial Blur",
        EffectSpec::Sharpen => "Sharpen",
        EffectSpec::UnsharpMask { .. } => "Unsharp Mask",
        EffectSpec::Emboss { .. } => "Emboss",
        EffectSpec::EdgeDetect { .. } => "Edge Detect",
        EffectSpec::Relief { .. } => "Relief",
        EffectSpec::AddNoise { .. } => "Add Noise",
        EffectSpec::ReduceNoise { .. } => "Reduce Noise",
        EffectSpec::BrightnessContrast { .. } => "Brightness/Contrast",
        EffectSpec::HueSaturation { .. } => "Hue/Saturation",
        EffectSpec::Curves { .. } => "Curves",
        EffectSpec::Levels { .. } => "Levels",
        EffectSpec::ColorBalance { .. } => "Color Balance",
        EffectSpec::Invert => "Invert",
        EffectSpec::Grayscale { .. } => "Grayscale",
        EffectSpec::Posterize { .. } => "Posterize",
        EffectSpec::Threshold { .. } => "Threshold",
    }
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

/// Applies `effect` to the pixels of the layer at `layer` as one undoable step
/// (spec §11, §17.5). Effects target a specific layer, never the composite
/// (§9 invariant). `effect` is a JSON [`EffectSpec`].
#[wasm_bindgen]
pub fn apply_effect(handle: u32, layer: usize, effect: &str) -> Result<(), JsError> {
    let spec: EffectSpec =
        serde_json::from_str(effect).map_err(|e| JsError::new(&e.to_string()))?;
    with_bus(handle, |bus| {
        let (src, w, h) = layer_effect_image(&bus.document, layer)?;
        let out = run_effect(&spec, &src);
        let after = ImageBuffer::from_raw(w, h, out.into_raw()).map_err(to_js)?;
        let cmd =
            SetPixels::new(layer, Rect::new(0, 0, w, h), after).with_label(effect_label(&spec));
        bus.apply(Box::new(cmd)).map_err(to_js)
    })
}

/// Renders a live preview of `effect` on the layer at `layer`: composites the
/// document with that layer's pixels replaced, without mutating document state
/// or the undo stack. Returns canvas-sized RGBA8 (spec §11 live preview).
#[wasm_bindgen]
pub fn preview_effect(
    handle: u32,
    layer: usize,
    effect: &str,
) -> Result<Clamped<Vec<u8>>, JsError> {
    let spec: EffectSpec =
        serde_json::from_str(effect).map_err(|e| JsError::new(&e.to_string()))?;
    with_bus(handle, |bus| {
        let mut layers = bus.document.layers().to_vec();
        let target = layers
            .get_mut(layer)
            .ok_or_else(|| JsError::new("layer index out of bounds"))?;
        let (w, h) = (target.pixels.width(), target.pixels.height());
        let src = EffectImage::from_rgba8(w, h, target.pixels.data().to_vec())
            .map_err(|e| JsError::new(&e.to_string()))?;
        let out = run_effect(&spec, &src);
        target.pixels = ImageBuffer::from_raw(w, h, out.into_raw()).map_err(to_js)?;
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
fn to_js(e: fineliner_core::DocumentError) -> JsError {
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
}

#[cfg(test)]
mod ts_bindings {
    use super::{CommandSpec, EffectSpec};
    use ts_rs::{Config, TS};

    /// Regenerates the TypeScript mirrors of `CommandSpec` and `EffectSpec`
    /// (Fineliner ADR-014). CI checks that the committed files under
    /// `ui/src/modes/image/core/generated/` are in sync.
    #[test]
    fn export_command_spec_bindings() {
        // u64 fields (stroke_id, noise seed) export as `number`: the UI
        // generates small values, far below Number.MAX_SAFE_INTEGER.
        let cfg = Config::new()
            .with_large_int("number")
            .with_out_dir("../../ui/src/modes/image/core/generated");
        CommandSpec::export_all(&cfg).expect("export CommandSpec bindings");
        EffectSpec::export_all(&cfg).expect("export EffectSpec bindings");
    }
}
