//! The serialized command format of Image mode and its dispatch.
//!
//! [`CommandSpec`] is the JSON command the editor sends across the wasm
//! boundary (spec §17 `SerializedCommand`); [`run_command`] turns one into
//! the matching tool or command and applies it to a [`CommandBus`]. Both
//! live here so every host — the wasm bindings and the agent tools
//! (docs/specs/mcp-headless-image.md) — runs the same code.

use serde::Deserialize;

use super::{
    AddGroup, AddLayer, Anchor, AutoCrop, Batch, CanvasRotation, CommandBus, CropToSelection,
    DuplicateLayer, FlattenImage, FlipCanvas, HistoryKind, LayerTransform, MergeDown, MergeGroup,
    MergeVisible, MoveLayer, MoveLayerTo, MoveSelection, RemoveLayer, RenameLayer, ResizeCanvas,
    RotateCanvas, RotateLayer90, ScaleImage, SetLayerBlendMode, SetLayerLocked, SetLayerOpacity,
    SetLayerVisible, SetSelection, TransformLayer, auto_crop_rect,
};
use crate::document::{Document, LayerPlacement};
use crate::error::DocumentError;
use crate::{
    Blend, BlendMode, Brush, BrushShape, CloneSnapshot, Color, Curve, CurveLine, DashPattern,
    Eraser, EraserMode, Fill, FillOptions, FontFace, Gradient, GradientChannel, GradientKind,
    Interpolation, LineCap, MAX_LINE_POINTS, Move, OverwriteMemory, PenSamples, Pencil, Point,
    PressureDynamics, Recolor, RecolorOrigins, Rect, SampleSource, SelectionMask, SelectionMode,
    Shape, ShapeMode, ShapeStyle, Shapes, Smudge, SmudgeCarry, Text, TextAlign, TextStyle,
    apply_mode, delete_selection, fill_selection, magic_wand,
};

/// One drag's worth of tool memory, reset when a new `stroke_id` arrives.
/// Hosts keep one per document and pass it to every [`run_command`].
#[derive(Default)]
pub struct StrokeScratch {
    id: u64,
    smudge: SmudgeCarry,
    recolor: RecolorOrigins,
    pub(super) clone: CloneSnapshot,
    /// An Overwrite Brush drag's pixels before it and highest coverage
    /// (docs/specs/image-edit-extras-tool-options.md AC-11, AC-12).
    overwrite: OverwriteMemory,
}

impl StrokeScratch {
    /// Start over when this is a different drag than the one we remember.
    pub(super) fn rewind(&mut self, id: u64) {
        if self.id != id {
            self.id = id;
            self.smudge.clear();
            self.recolor.clear();
            self.clone.clear();
            self.overwrite.clear();
        }
    }
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
    crate::DEFAULT_BRUSH_ANGLE
}

/// Pen azimuth per point for the core: a missing reading becomes NaN.
pub(super) fn azimuth_samples(azimuth: &[Option<f32>]) -> Vec<f32> {
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
pub(super) fn parse_shape(s: &str) -> BrushShape {
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
pub(super) fn parse_shape_mode(s: &str) -> ShapeMode {
    match s {
        "fill" => ShapeMode::Fill,
        "fill_and_outline" => ShapeMode::FillAndOutline,
        _ => ShapeMode::Outline,
    }
}

pub(super) fn parse_gradient_kind(s: &str) -> GradientKind {
    match s {
        "reflected" => GradientKind::Reflected,
        "radial" => GradientKind::Radial,
        "conical" => GradientKind::Conical,
        "diamond" => GradientKind::Diamond,
        "spiral_cw" => GradientKind::SpiralCw,
        "spiral_ccw" => GradientKind::SpiralCcw,
        _ => GradientKind::Linear,
    }
}

pub(super) fn parse_gradient_channel(s: &str) -> GradientChannel {
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

fn default_star_points() -> u32 {
    crate::STAR_POINTS
}

fn default_inner_ratio() -> f32 {
    crate::STAR_INNER_RATIO
}

fn default_cap() -> String {
    "round".to_string()
}

/// Maps a cap string to a [`LineCap`] (docs/specs/image-shapes-curves.md),
/// defaulting to Round.
fn parse_cap(s: &str) -> LineCap {
    LineCap::parse(s).unwrap_or_default()
}

fn default_blend() -> String {
    "normal".to_string()
}

/// Maps a blend string to a [`Blend`]; anything but `overwrite` is Normal.
pub(super) fn parse_blend(s: &str) -> Blend {
    if s == "overwrite" {
        Blend::Overwrite
    } else {
        Blend::Normal
    }
}

/// Maps a dash-pattern string to a [`DashPattern`], defaulting to solid.
pub(super) fn parse_dash(s: &str) -> DashPattern {
    match s {
        "dashed" => DashPattern::Dashed,
        "dotted" => DashPattern::Dotted,
        "dash_dot" => DashPattern::DashDot,
        "dash_dot_dot" => DashPattern::DashDotDot,
        _ => DashPattern::Solid,
    }
}

/// Maps an interpolation string to an [`Interpolation`], defaulting to bilinear.
fn parse_interpolation(s: &str) -> Interpolation {
    match s {
        "nearest" => Interpolation::Nearest,
        "bicubic" => Interpolation::Bicubic,
        "supersampling" => Interpolation::Supersampling,
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
pub fn parse_sample(s: &str) -> SampleSource {
    match s {
        "all_layers" => SampleSource::AllLayers,
        _ => SampleSource::CurrentLayer,
    }
}

/// A JSON-serializable command from JS (spec §17 `SerializedCommand`).
///
/// The TypeScript mirror of this enum is generated from it via ts-rs
/// (ADR-014): `cargo test -p pincel-image-wasm export_command_spec` writes
/// `ui/src/modes/image/core/generated/CommandSpec.ts`, and the UI's command types are
/// checked against it at `pnpm check` time.
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandSpec {
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
        /// Anti-aliased edge for the hard round tip
        /// (docs/specs/image-edit-extras-tool-options.md AC-02, AC-03).
        #[serde(default)]
        anti_alias: bool,
        /// `normal` or `overwrite` (docs/specs/image-edit-extras-tool-options.md).
        #[serde(default = "default_blend")]
        blend: String,
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
        /// Anti-aliased edge for the hard round tip
        /// (docs/specs/image-edit-extras-tool-options.md AC-02, AC-03).
        #[serde(default)]
        anti_alias: bool,
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
        /// Anti-aliased edge for the hard round tip
        /// (docs/specs/image-edit-extras-tool-options.md AC-02, AC-03).
        #[serde(default)]
        anti_alias: bool,
        stroke_id: u64,
    },
    /// A Clone Stamp stroke: copies the pixels `offset` away through the
    /// brush tip (docs/specs/clone-stamp.md).
    CloneStroke {
        layer: usize,
        size: u32,
        opacity: f32,
        #[serde(default = "default_shape")]
        shape: String,
        #[serde(default = "default_hardness")]
        hardness: f32,
        /// Source position minus destination position, in whole pixels.
        offset: [i32; 2],
        /// Where the copied pixels come from: `current_layer` (default) or
        /// `all_layers`.
        #[serde(default = "default_sample")]
        sample: String,
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
        /// Anti-aliased edge for the hard round tip
        /// (docs/specs/image-edit-extras-tool-options.md AC-02, AC-03).
        #[serde(default)]
        anti_alias: bool,
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
        /// Anti-aliased edge for the hard round tip
        /// (docs/specs/image-edit-extras-tool-options.md AC-02, AC-03).
        #[serde(default)]
        anti_alias: bool,
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
        /// `normal` or `overwrite` (docs/specs/image-edit-extras-tool-options.md).
        #[serde(default = "default_blend")]
        blend: String,
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
    /// Fill the selected pixels of `layer` with `color` (RGBA), partial
    /// coverage moving a pixel toward it; the whole layer when no selection
    /// is active (Edit ▸ Fill Selection). No undo step when nothing changes.
    FillSelection { layer: usize, color: [u8; 4] },
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
    /// Add an empty group named `Group N` on top of the root stack.
    AddGroup,
    /// Move the layer at `index` (with its subtree) next to or into the
    /// layer at `target`; `place` is `above`, `below` or `into`.
    MoveLayerTo {
        index: usize,
        target: usize,
        place: String,
    },
    /// Replace the group at `index` and its subtree with one raster layer.
    MergeGroup { index: usize },
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
        /// Anti-aliased outline (image-edit-extras-tool-options AC-06).
        #[serde(default)]
        anti_alias: bool,
    },
    /// Polygonal selection over `points` ([x, y] each) combined per `mode`.
    SelectPolygon {
        points: Vec<[f32; 2]>,
        #[serde(default = "default_selection_mode")]
        mode: String,
        #[serde(default)]
        feather: u32,
        /// Anti-aliased outline (image-edit-extras-tool-options AC-06).
        #[serde(default)]
        anti_alias: bool,
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
    /// Crop the canvas to the content inside a uniform border; adds no undo
    /// step when there is nothing to crop (docs/specs/image-resize-autocrop.md).
    AutoCrop,
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
        /// `normal` or `overwrite` (docs/specs/image-edit-extras-tool-options.md).
        #[serde(default = "default_blend")]
        blend: String,
        /// A line's start cap: `round` (default), `flat` or `arrow`
        /// (docs/specs/image-shapes-curves.md). A line takes 2–64 `points`:
        /// the start, the control points in order, the end.
        #[serde(default = "default_cap")]
        start_cap: String,
        /// A line's end cap.
        #[serde(default = "default_cap")]
        end_cap: String,
        /// A star's points, 3–50 (docs/specs/image-shapes-curves-shapes.md);
        /// a star takes `center`, `radius` and `rotation` like `polygon`.
        #[serde(default = "default_star_points")]
        star_points: u32,
        /// A star's inner radius as a share of `radius`, 0.1–0.9.
        #[serde(default = "default_inner_ratio")]
        inner_ratio: f32,
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
        /// `normal` or `overwrite` (docs/specs/image-edit-extras-tool-options.md).
        #[serde(default = "default_blend")]
        blend: String,
    },
    /// Begin a live Line or box shape (docs/specs/image-live-objects.md):
    /// drawn as `draw_shape` draws it, but editable until `commit_live`.
    /// `shape` is `line`, `rectangle`, `rounded_rectangle`, `ellipse` or
    /// `polygon`; `points` are the endpoints or two opposite box corners (a
    /// polygon is inscribed in the box). Degenerate geometry begins nothing.
    BeginLiveShape {
        layer: usize,
        shape: String,
        points: [[f32; 2]; 2],
        #[serde(default)]
        corner_radius: f32,
        #[serde(default)]
        sides: u32,
        /// A star's points, 3–50 (docs/specs/image-shapes-curves-shapes.md).
        #[serde(default = "default_star_points")]
        star_points: u32,
        /// A star's inner radius as a share of the outer one, 0.1–0.9.
        #[serde(default = "default_inner_ratio")]
        inner_ratio: f32,
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
        /// `normal` or `overwrite` (docs/specs/image-edit-extras-tool-options.md).
        #[serde(default = "default_blend")]
        blend: String,
        /// A line's control points between the endpoints, in order
        /// (docs/specs/image-shapes-curves.md).
        #[serde(default)]
        control: Vec<[f32; 2]>,
        /// A line's start cap: `round` (default), `flat` or `arrow`.
        #[serde(default = "default_cap")]
        start_cap: String,
        /// A line's end cap.
        #[serde(default = "default_cap")]
        end_cap: String,
    },
    /// Begin a live gradient along `(x0, y0)` -> `(x1, y1)`, drawn as
    /// `draw_gradient` draws it (docs/specs/image-live-objects.md).
    BeginLiveGradient {
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
        /// `normal` or `overwrite` (docs/specs/image-edit-extras-tool-options.md).
        #[serde(default = "default_blend")]
        blend: String,
    },
    /// Edit the live object: absent fields keep their value. Edits sharing
    /// a `gesture` (one handle drag), or consecutive edits of one parameter
    /// group, form one undo step. `points` are endpoints, box corners or the
    /// gradient's drag points; `kind` / `channel` / `from` / `to` /
    /// `opacity` apply to a gradient, the other fields to a shape.
    EditLive {
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        gesture: Option<u64>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        points: Option<[[f32; 2]; 2]>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        shape: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        mode: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        stroke_width: Option<f32>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        dash: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        anti_alias: Option<bool>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        corner_radius: Option<f32>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        sides: Option<u32>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        star_points: Option<u32>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        inner_ratio: Option<f32>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        stroke_color: Option<[u8; 4]>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        fill_color: Option<[u8; 4]>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        kind: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        channel: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        from: Option<[u8; 4]>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        to: Option<[u8; 4]>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        opacity: Option<f32>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        text: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        size: Option<f32>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        color: Option<[u8; 4]>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        bold: Option<bool>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        italic: Option<bool>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        underline: Option<bool>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        strikethrough: Option<bool>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        align: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        blend: Option<String>,
        /// A line's control points, all of them, in order.
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        control: Option<Vec<[f32; 2]>>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        start_cap: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        end_cap: Option<String>,
        /// A live text's new font, a `register_font` id
        /// (docs/specs/system-fonts.md AC-07, AC-09).
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        font_id: Option<u32>,
    },
    /// Begin a live text (docs/specs/image-live-text.md): `(x, y)` is the
    /// top-left of its box for every alignment, `font_id` a `register_font`
    /// id. An empty text, or an unknown font, begins nothing.
    BeginLiveText {
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
        #[serde(default)]
        underline: bool,
        #[serde(default)]
        strikethrough: bool,
        /// `normal` or `overwrite` (docs/specs/image-edit-extras-tool-options.md).
        #[serde(default = "default_blend")]
        blend: String,
    },
    /// Commit the live object: its steps become one (or none, if it changes
    /// no pixel).
    CommitLive,
    /// Cancel the live object: the layer shows the pixels from before it.
    CancelLive,
    /// Drop the step of the live object's handle drag `gesture` without a
    /// redo step (Escape during the drag); nothing when the top step is not
    /// that drag's.
    CancelLiveGesture { gesture: u64 },
    /// Begin the Move tools' live transform of the selection on `layer`
    /// (docs/specs/image-transform-handles.md): its pixels (`pixels`, Move
    /// Selected Pixels) or its outline only (Move Selection), shown with
    /// the box below. Commits any other live object first; nothing without
    /// a selection that selects a pixel.
    BeginTransform {
        layer: usize,
        /// `true`: Move Selected Pixels; `false`: Move Selection.
        pixels: bool,
        /// Centre of the box on the canvas (pixel-edge coordinates).
        center_x: f64,
        center_y: f64,
        /// Width and height of the box (clamped to at least 1 px).
        width: f64,
        height: f64,
        /// Turn in degrees, positive clockwise on screen.
        angle: f64,
        /// The pivot relative to the box (`0.5, 0.5` = its centre).
        pivot_x: f64,
        pivot_y: f64,
        /// `0` Nearest, `1` Bilinear, `2` Bicubic.
        quality: u32,
        /// The drag this change belongs to: changes of one gesture form one
        /// undo step.
        #[serde(default)]
        gesture: Option<u64>,
    },
    /// Change the live transform's box, angle, pivot or quality.
    EditTransform {
        /// Centre of the box on the canvas (pixel-edge coordinates).
        center_x: f64,
        center_y: f64,
        /// Width and height of the box (clamped to at least 1 px).
        width: f64,
        height: f64,
        /// Turn in degrees, positive clockwise on screen.
        angle: f64,
        /// The pivot relative to the box (`0.5, 0.5` = its centre).
        pivot_x: f64,
        pivot_y: f64,
        /// `0` Nearest, `1` Bilinear, `2` Bicubic.
        quality: u32,
        /// The drag this change belongs to: changes of one gesture form one
        /// undo step.
        #[serde(default)]
        gesture: Option<u64>,
    },
    /// Commit the live transform: its steps become one (or none, if nothing
    /// changed).
    CommitTransform,
    /// Cancel the live transform: pixels and outline as before it.
    CancelTransform,
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
        /// A stroke below each line (docs/specs/image-live-text.md AC-10).
        #[serde(default)]
        underline: bool,
        /// A stroke through each line's lowercase letters.
        #[serde(default)]
        strikethrough: bool,
        /// `normal` or `overwrite` (docs/specs/image-edit-extras-tool-options.md).
        #[serde(default = "default_blend")]
        blend: String,
    },
}

impl CommandSpec {
    /// The layer whose pixels this command writes, if it is a pixel edit.
    /// Property, structure and selection commands return `None`, so a
    /// locked layer can still be renamed, hidden, moved or unlocked.
    pub fn edited_layer(&self) -> Option<usize> {
        match self {
            Self::PencilStroke { layer, .. }
            | Self::EraserStroke { layer, .. }
            | Self::FillBucket { layer, .. }
            | Self::TranslateLayer { layer, .. }
            | Self::TranslateSelection { layer, .. }
            | Self::DrawGradient { layer, .. }
            | Self::RecolorStroke { layer, .. }
            | Self::SmudgeStroke { layer, .. }
            | Self::CloneStroke { layer, .. }
            | Self::DeleteSelection { layer }
            | Self::FillSelection { layer, .. }
            | Self::TransformLayer { layer, .. }
            | Self::RotateLayer90 { layer, .. }
            | Self::DrawShape { layer, .. }
            | Self::BeginLiveShape { layer, .. }
            | Self::BeginLiveGradient { layer, .. }
            | Self::BeginLiveText { layer, .. }
            | Self::BeginTransform {
                layer,
                pixels: true,
                ..
            }
            | Self::DrawText { layer, .. } => Some(*layer),
            _ => None,
        }
    }

    /// Whether the command moves or turns a layer's whole content (Move
    /// layer, layer flips and rotations) — the pixel edits that act on every
    /// raster descendant when they target a group (docs/specs/image-layer-groups.md AC-18).
    fn transforms_layer(&self) -> bool {
        matches!(
            self,
            Self::TranslateLayer { .. } | Self::TransformLayer { .. } | Self::RotateLayer90 { .. }
        )
    }

    /// Whether the editor refuses this command on `doc` with the
    /// locked-layer notice: a pixel edit on a layer that is locked itself
    /// or through a parent group, or on a group (which has no pixels).
    /// Move (layer) and layer flips / rotations on a group are allowed
    /// unless the group or a layer inside it is locked. Commands that edit
    /// no pixels, and out-of-range layers, are never refused here.
    pub fn refused(&self, doc: &Document) -> bool {
        let Some(index) = self.edited_layer() else {
            return false;
        };
        match doc.layers().get(index) {
            Some(layer) if layer.is_group() && self.transforms_layer() => {
                (index..doc.subtree_end(index)).any(|i| doc.effectively_locked(i))
            }
            Some(_) => layer_locked(doc, index),
            None => false,
        }
    }
}

/// The commands that move or turn the content of every raster layer in the
/// group at `group` (docs/specs/image-layer-groups.md AC-18), built by `one`
/// per layer. `None` when `group` is not a group.
fn group_edits(
    doc: &Document,
    group: usize,
    one: impl Fn(usize) -> Option<Box<dyn super::Command>>,
) -> Option<Vec<Box<dyn super::Command>>> {
    if !doc.layers().get(group)?.is_group() {
        return None;
    }
    Some(
        (group + 1..doc.subtree_end(group))
            .filter(|&i| !doc.layers()[i].is_group())
            .filter_map(one)
            .collect(),
    )
}

/// Applies the per-layer `cmds` of a group edit as one undo step; nothing
/// when there are none (an empty group or an edit that changes nothing).
fn apply_group_edits(
    bus: &mut CommandBus,
    cmds: Vec<Box<dyn super::Command>>,
    kind: HistoryKind,
) -> Result<(), DocumentError> {
    if cmds.is_empty() {
        return Ok(());
    }
    bus.apply(Box::new(Batch::new(cmds, kind)))
}

/// Applies `spec` to `bus` exactly as the editor does: one tool or command,
/// recorded in history (strokes sharing a `stroke_id` merge into one step).
/// `scratch` carries Recolor / Smudge memory across the commands of one
/// drag; `fonts` are the faces `DrawText` names by `font_id`. A command that
/// misses the canvas, an unknown font or an invalid geometry is a no-op.
pub fn run_command(
    bus: &mut CommandBus,
    spec: CommandSpec,
    scratch: &mut StrokeScratch,
    fonts: &[FontFace<'_>],
) -> Result<(), DocumentError> {
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
            anti_alias,
            blend,
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
            .with_follow_tilt(tilt_angle)
            .with_anti_alias(anti_alias);
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            let azimuth = azimuth_samples(&azimuth);
            let pen = PenSamples {
                pressure: &pressure,
                azimuth: &azimuth,
            };
            scratch.rewind(stroke_id);
            let brush = brush.with_blend(parse_blend(&blend));
            let painted = Pencil::new(brush).stroke_with_memory(
                layer,
                &pts,
                pen,
                &bus.document,
                &mut scratch.overwrite,
            );
            match painted {
                Some(cmd) => bus.apply(Box::new(cmd.with_stroke(stroke_id))),
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
            anti_alias,
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
                .with_follow_tilt(tilt_angle)
                .with_anti_alias(anti_alias);
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
            scratch.rewind(stroke_id);
            let committed =
                tool.stroke_with_pen(layer, &pts, pen, &bus.document, &mut scratch.recolor);
            match committed {
                Some(cmd) => bus.apply(Box::new(cmd.with_stroke(stroke_id))),
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
            anti_alias,
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
                .with_follow_tilt(tilt_angle)
                .with_anti_alias(anti_alias);
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            let azimuth = azimuth_samples(&azimuth);
            let pen = PenSamples {
                pressure: &pressure,
                azimuth: &azimuth,
            };
            scratch.rewind(stroke_id);
            let committed = Smudge::new(brush, strength).stroke_with_pen(
                layer,
                &pts,
                pen,
                &bus.document,
                &mut scratch.smudge,
            );
            match committed {
                Some(cmd) => bus.apply(Box::new(cmd.with_stroke(stroke_id))),
                None => Ok(()), // stroke missed the canvas — no-op
            }
        }
        clone @ CommandSpec::CloneStroke { .. } => super::clone_spec::run(bus, clone, scratch),
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
            anti_alias,
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
                .with_follow_tilt(tilt_angle)
                .with_anti_alias(anti_alias);
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
                Some(cmd) => bus.apply(Box::new(cmd.with_stroke(stroke_id))),
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
            blend,
        } => {
            let options = FillOptions {
                tolerance,
                contiguous,
                sample: parse_sample(&sample),
            };
            let fill = Fill::new(Color::rgba(color[0], color[1], color[2], color[3]), options)
                .with_opacity(opacity)
                .with_blend(parse_blend(&blend));
            match fill.fill(layer, Point::new(x, y), &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)),
                None => Ok(()),
            }
        }
        CommandSpec::TranslateLayer { layer, dx, dy } => {
            let doc = &bus.document;
            if let Some(cmds) = group_edits(doc, layer, |i| {
                Move.translate(i, dx, dy, doc)
                    .map(|c| Box::new(c) as Box<dyn super::Command>)
            }) {
                return apply_group_edits(bus, cmds, HistoryKind::TranslateLayer);
            }
            match Move.translate(layer, dx, dy, &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)),
                None => Ok(()),
            }
        }
        CommandSpec::TranslateSelection { layer, dx, dy } => {
            match Move.translate_selection(layer, dx, dy, &bus.document) {
                // The outline travels with its content, so a second drag
                // carries the patch on rather than the hole it left.
                Some(pixels) => bus.apply(Box::new(MoveSelection::new(pixels, dx, dy))),
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
        }
        CommandSpec::DeleteSelection { layer } => {
            match delete_selection(layer, &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)),
                None => Ok(()), // empty selection or invalid layer — no-op
            }
        }
        CommandSpec::FillSelection { layer, color } => {
            let [r, g, b, a] = color;
            match fill_selection(layer, Color::rgba(r, g, b, a), &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)),
                None => Ok(()), // nothing would change, or an invalid layer
            }
        }
        CommandSpec::AddLayer { active } => bus.apply(Box::new(AddLayer::above(active))),
        CommandSpec::RemoveLayer { index } => bus.apply(Box::new(RemoveLayer::at(index))),
        CommandSpec::MoveLayer { from, to } => bus.apply(Box::new(MoveLayer::new(from, to))),
        CommandSpec::DuplicateLayer { index } => bus.apply(Box::new(DuplicateLayer::at(index))),
        CommandSpec::RenameLayer { index, name } => {
            bus.apply(Box::new(RenameLayer::new(index, name)))
        }
        CommandSpec::SetLayerOpacity { index, opacity } => {
            bus.apply(Box::new(SetLayerOpacity::new(index, opacity)))
        }
        CommandSpec::SetLayerBlendMode { index, mode } => bus.apply(Box::new(
            SetLayerBlendMode::new(index, parse_blend_mode(&mode)),
        )),
        CommandSpec::SetLayerVisible { index, visible } => {
            bus.apply(Box::new(SetLayerVisible::new(index, visible)))
        }
        CommandSpec::SetLayerLocked { index, locked } => {
            bus.apply(Box::new(SetLayerLocked::new(index, locked)))
        }
        CommandSpec::MergeDown { index } => bus.apply(Box::new(MergeDown::at(index))),
        CommandSpec::MergeVisible => bus.apply(Box::new(MergeVisible::new())),
        CommandSpec::FlattenImage => bus.apply(Box::new(FlattenImage::new())),
        CommandSpec::AddGroup => bus.apply(Box::new(AddGroup::on_top())),
        CommandSpec::MoveLayerTo {
            index,
            target,
            place,
        } => {
            let place = match place.as_str() {
                "above" => LayerPlacement::Above,
                "below" => LayerPlacement::Below,
                _ => LayerPlacement::Into,
            };
            bus.apply(Box::new(MoveLayerTo::new(index, target, place)))
        }
        CommandSpec::MergeGroup { index } => bus.apply(Box::new(MergeGroup::at(index))),
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
            apply_selection(
                bus,
                shape,
                parse_selection_mode(&mode),
                HistoryKind::SelectRectangle,
            )
        }
        CommandSpec::SelectEllipse {
            x,
            y,
            w,
            h,
            mode,
            feather,
            anti_alias,
        } => {
            let (cw, ch) = (bus.document.canvas.width(), bus.document.canvas.height());
            let rect = Rect::new(x, y, w, h);
            let mut shape = if anti_alias {
                SelectionMask::ellipse_aa(cw, ch, rect)
            } else {
                SelectionMask::ellipse(cw, ch, rect)
            };
            shape.feather(feather);
            apply_selection(
                bus,
                shape,
                parse_selection_mode(&mode),
                HistoryKind::SelectEllipse,
            )
        }
        CommandSpec::SelectPolygon {
            points,
            mode,
            feather,
            anti_alias,
        } => {
            let (cw, ch) = (bus.document.canvas.width(), bus.document.canvas.height());
            let pts: Vec<Point> = points.iter().map(|p| Point::new(p[0], p[1])).collect();
            let mut shape = if anti_alias {
                SelectionMask::polygon_aa(cw, ch, &pts)
            } else {
                SelectionMask::polygon(cw, ch, &pts)
            };
            shape.feather(feather);
            apply_selection(
                bus,
                shape,
                parse_selection_mode(&mode),
                HistoryKind::SelectLasso,
            )
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
                Some(shape) => apply_selection(
                    bus,
                    shape,
                    parse_selection_mode(&mode),
                    HistoryKind::SelectMagicWand,
                ),
                None => Ok(()), // off-canvas seed — no-op
            }
        }
        CommandSpec::SelectAll => {
            let (cw, ch) = (bus.document.canvas.width(), bus.document.canvas.height());
            bus.apply(Box::new(
                SetSelection::replace(SelectionMask::new_full(cw, ch))
                    .with_kind(HistoryKind::SelectAll),
            ))
        }
        CommandSpec::Deselect => bus.apply(Box::new(SetSelection::clear())),
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
                SetSelection::replace(mask).with_kind(HistoryKind::InvertSelection),
            ))
        }
        CommandSpec::ExpandSelection { radius } => {
            modify_selection(bus, HistoryKind::ExpandSelection, |m| m.expand(radius))
        }
        CommandSpec::ContractSelection { radius } => {
            modify_selection(bus, HistoryKind::ContractSelection, |m| m.contract(radius))
        }
        CommandSpec::FeatherSelection { radius } => {
            modify_selection(bus, HistoryKind::FeatherSelection, |m| m.feather(radius))
        }
        CommandSpec::TransformLayer { layer, op } => {
            let op = match op.as_str() {
                "flip_v" => LayerTransform::FlipVertical,
                "rotate_180" => LayerTransform::Rotate180,
                _ => LayerTransform::FlipHorizontal,
            };
            if let Some(cmds) = group_edits(&bus.document, layer, |i| {
                Some(Box::new(TransformLayer::new(i, op)))
            }) {
                let kind = super::Command::kind(&TransformLayer::new(layer, op));
                return apply_group_edits(bus, cmds, kind);
            }
            bus.apply(Box::new(TransformLayer::new(layer, op)))
        }
        CommandSpec::RotateLayer90 { layer, ccw } => {
            if let Some(cmds) = group_edits(&bus.document, layer, |i| {
                Some(Box::new(RotateLayer90::new(i, ccw)))
            }) {
                let kind = super::Command::kind(&RotateLayer90::new(layer, ccw));
                return apply_group_edits(bus, cmds, kind);
            }
            bus.apply(Box::new(RotateLayer90::new(layer, ccw)))
        }
        CommandSpec::FlipCanvas { horizontal } => bus.apply(Box::new(FlipCanvas::new(horizontal))),
        CommandSpec::RotateCanvas { rotation } => {
            let rot = match rotation.as_str() {
                "ccw90" => CanvasRotation::Ccw90,
                "rotate_180" => CanvasRotation::Rotate180,
                _ => CanvasRotation::Cw90,
            };
            bus.apply(Box::new(RotateCanvas::new(rot)))
        }
        CommandSpec::ScaleImage {
            width,
            height,
            interpolation,
        } => bus.apply(Box::new(ScaleImage::new(
            width,
            height,
            parse_interpolation(&interpolation),
        ))),
        CommandSpec::ResizeCanvas {
            width,
            height,
            anchor,
        } => bus.apply(Box::new(
            ResizeCanvas::new(width, height).with_anchor(parse_anchor(&anchor)),
        )),
        CommandSpec::CropToSelection => bus.apply(Box::new(CropToSelection::new())),
        CommandSpec::AutoCrop => match auto_crop_rect(&bus.document) {
            Some(_) => bus.apply(Box::new(AutoCrop::new())),
            None => Ok(()),
        },
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
            blend,
            start_cap,
            end_cap,
            star_points,
            inner_ratio,
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
                // The box shapes of docs/specs/image-shapes-curves-shapes.md;
                // the first point is the drag start.
                "triangle" => corner(0)
                    .zip(corner(1))
                    .map(|(a, b)| Shape::Triangle { a, b }),
                "diamond" => corner(0)
                    .zip(corner(1))
                    .map(|(a, b)| Shape::Diamond { a, b }),
                "heart" => corner(0).zip(corner(1)).map(|(a, b)| Shape::Heart { a, b }),
                "block_arrow" => corner(0)
                    .zip(corner(1))
                    .map(|(start, end)| Shape::BlockArrow { start, end }),
                "callout" => corner(0)
                    .zip(corner(1))
                    .map(|(start, end)| Shape::Callout { start, end }),
                "star" => Some(Shape::Star {
                    center: Point::new(center[0], center[1]),
                    radius,
                    points: star_points,
                    inner_ratio,
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
                blend: parse_blend(&blend),
            };
            if shape == "line" {
                // 2–64 points: the start, the control points, the end
                // (docs/specs/image-shapes-curves.md); more draw nothing.
                if points.len() < 2 || points.len() > MAX_LINE_POINTS {
                    return Ok(());
                }
                let curve = Curve {
                    points: points.iter().map(|q| Point::new(q[0], q[1])).collect(),
                    start_cap: parse_cap(&start_cap),
                    end_cap: parse_cap(&end_cap),
                };
                return match CurveLine::new(curve, style).draw(layer, &bus.document) {
                    Some(cmd) => bus.apply(Box::new(cmd)),
                    None => Ok(()),
                };
            }
            match built.and_then(|s| Shapes::new(s, style).draw(layer, &bus.document)) {
                Some(cmd) => bus.apply(Box::new(cmd)),
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
            blend,
        } => {
            let tool = Gradient {
                kind: parse_gradient_kind(&kind),
                channel: parse_gradient_channel(&channel),
                from: Color::rgba(from[0], from[1], from[2], from[3]),
                to: Color::rgba(to[0], to[1], to[2], to[3]),
                opacity,
                blend: parse_blend(&blend),
            };
            let a = Point::new(x0, y0);
            let b = Point::new(x1, y1);
            match tool.draw(layer, a, b, &bus.document) {
                Some(cmd) => bus.apply(Box::new(cmd)),
                None => Ok(()), // zero-length drag, empty selection, bad layer
            }
        }
        CommandSpec::BeginLiveShape {
            layer,
            shape,
            points,
            corner_radius,
            sides,
            star_points,
            inner_ratio,
            mode,
            stroke_width,
            stroke_color,
            fill_color,
            anti_alias,
            dash,
            blend,
            control,
            start_cap,
            end_cap,
        } => {
            let Some(kind) = super::live_spec::parse_live_shape(&shape) else {
                return Ok(()); // unknown shape: a no-op, as for draw_shape
            };
            let line = kind == super::LiveShapeKind::Line;
            let caps_set = start_cap != "round" || end_cap != "round";
            if !line && (!control.is_empty() || caps_set) {
                return Err(DocumentError::LiveEditMismatch);
            }
            if control.len() + 2 > MAX_LINE_POINTS {
                return Err(DocumentError::LiveEditMismatch);
            }
            let params = super::LiveParams::Shape(super::LiveShape {
                kind,
                a: Point::new(points[0][0], points[0][1]),
                b: Point::new(points[1][0], points[1][1]),
                style: ShapeStyle {
                    mode: parse_shape_mode(&mode),
                    stroke_width,
                    stroke_color: rgba(stroke_color),
                    fill_color: rgba(fill_color),
                    anti_alias,
                    dash: parse_dash(&dash),
                    blend: parse_blend(&blend),
                },
                corner_radius,
                sides,
                star_points,
                inner_ratio,
                control: control.iter().map(|q| Point::new(q[0], q[1])).collect(),
                start_cap: parse_cap(&start_cap),
                end_cap: parse_cap(&end_cap),
            });
            bus.begin_live(layer, params).map(|_| ())
        }
        CommandSpec::BeginLiveGradient {
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
            blend,
        } => {
            let params = super::LiveParams::Gradient(super::LiveGradient {
                gradient: Gradient {
                    kind: parse_gradient_kind(&kind),
                    channel: parse_gradient_channel(&channel),
                    from: rgba(from),
                    to: rgba(to),
                    opacity,
                    blend: parse_blend(&blend),
                },
                a: Point::new(x0, y0),
                b: Point::new(x1, y1),
            });
            bus.begin_live(layer, params).map(|_| ())
        }
        CommandSpec::EditLive {
            gesture,
            points,
            shape,
            mode,
            stroke_width,
            dash,
            anti_alias,
            corner_radius,
            sides,
            star_points,
            inner_ratio,
            stroke_color,
            fill_color,
            kind,
            channel,
            from,
            to,
            opacity,
            text,
            size,
            color,
            bold,
            italic,
            underline,
            strikethrough,
            align,
            blend,
            control,
            start_cap,
            end_cap,
            font_id,
        } => {
            // An unknown font id keeps the font, as `draw_text` ignores one.
            let font = font_id.and_then(|id| {
                fonts.get(id as usize).map(|f| super::LiveFont {
                    id,
                    bytes: std::sync::Arc::from(f.bytes),
                    index: f.index,
                })
            });
            let cap = |c: Option<String>| -> Result<Option<LineCap>, DocumentError> {
                c.map(|c| LineCap::parse(&c).ok_or(DocumentError::LiveEditMismatch))
                    .transpose()
            };
            let (start_cap, end_cap) = (cap(start_cap)?, cap(end_cap)?);
            let shape = match shape {
                Some(s) => Some(
                    super::live_spec::parse_live_shape(&s)
                        .ok_or(DocumentError::LiveEditMismatch)?,
                ),
                None => None,
            };
            let edit = super::LiveEdit {
                points: points.map(|[a, b]| [Point::new(a[0], a[1]), Point::new(b[0], b[1])]),
                shape,
                mode: mode.as_deref().map(parse_shape_mode),
                stroke_width,
                dash: dash.as_deref().map(parse_dash),
                anti_alias,
                corner_radius,
                sides,
                star_points,
                inner_ratio,
                stroke_color: stroke_color.map(rgba),
                fill_color: fill_color.map(rgba),
                gradient_kind: kind.as_deref().map(parse_gradient_kind),
                gradient_channel: channel.as_deref().map(parse_gradient_channel),
                gradient_from: from.map(rgba),
                gradient_to: to.map(rgba),
                gradient_opacity: opacity,
                text,
                text_size: size,
                text_color: color.map(rgba),
                bold,
                italic,
                underline,
                strikethrough,
                align: align.as_deref().map(parse_text_align),
                blend: blend.as_deref().map(parse_blend),
                control: control.map(|c| c.iter().map(|q| Point::new(q[0], q[1])).collect()),
                start_cap,
                end_cap,
                font,
            };
            bus.edit_live(&edit, gesture)
        }
        CommandSpec::BeginLiveText {
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
            underline,
            strikethrough,
            blend,
        } => {
            let Some(face) = fonts.get(font_id as usize) else {
                return Ok(()); // unknown font: a no-op, as for draw_text
            };
            let params = super::LiveParams::Text(super::LiveText {
                text,
                left: x,
                top: y,
                style: TextStyle {
                    size,
                    color: rgba(color),
                    bold,
                    italic,
                    anti_alias,
                    align: parse_text_align(&align),
                    underline,
                    strikethrough,
                    blend: parse_blend(&blend),
                },
                font: super::LiveFont {
                    id: font_id,
                    bytes: std::sync::Arc::from(face.bytes),
                    index: face.index,
                },
            });
            bus.begin_live(layer, params).map(|_| ())
        }
        CommandSpec::CommitLive => bus.commit_live(),
        CommandSpec::CancelLive => bus.cancel_live(),
        CommandSpec::CancelLiveGesture { gesture } => bus.cancel_live_gesture(gesture).map(|_| ()),
        CommandSpec::BeginTransform {
            layer,
            pixels,
            center_x,
            center_y,
            width,
            height,
            angle,
            pivot_x,
            pivot_y,
            quality,
            gesture,
        } => {
            let s = transform_settings(
                (center_x, center_y),
                (width, height),
                angle,
                (pivot_x, pivot_y),
                quality,
            );
            bus.begin_transform(layer, pixels, s, gesture).map(|_| ())
        }
        CommandSpec::EditTransform {
            center_x,
            center_y,
            width,
            height,
            angle,
            pivot_x,
            pivot_y,
            quality,
            gesture,
        } => {
            let s = transform_settings(
                (center_x, center_y),
                (width, height),
                angle,
                (pivot_x, pivot_y),
                quality,
            );
            bus.edit_transform(s, gesture)
        }
        CommandSpec::CommitTransform => bus.commit_transform(),
        CommandSpec::CancelTransform => bus.cancel_transform(),
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
            underline,
            strikethrough,
            blend,
        } => {
            let style = TextStyle {
                size,
                color: Color::rgba(color[0], color[1], color[2], color[3]),
                bold,
                italic,
                anti_alias,
                align: parse_text_align(&align),
                underline,
                strikethrough,
                blend: parse_blend(&blend),
            };
            let cmd = fonts.get(font_id as usize).and_then(|face| {
                Text::new(text, Point::new(x, y), style).render(layer, &bus.document, *face)
            });
            match cmd {
                Some(cmd) => bus.apply(Box::new(cmd)),
                None => Ok(()), // unknown font, empty text, or off-canvas — no-op
            }
        }
    }
}

/// A [`Color`] from wire RGBA bytes.
fn rgba(c: [u8; 4]) -> Color {
    Color::rgba(c[0], c[1], c[2], c[3])
}

/// Combines `shape` with the current selection per `mode` and applies it as an
/// undoable [`SetSelection`] of history kind `kind`.
fn apply_selection(
    bus: &mut CommandBus,
    shape: SelectionMask,
    mode: SelectionMode,
    kind: HistoryKind,
) -> Result<(), DocumentError> {
    let after = apply_mode(bus.document.selection.as_ref(), shape, mode);
    bus.apply(Box::new(SetSelection::replace(after).with_kind(kind)))
}

/// Applies an in-place modifier (`expand`/`contract`/`feather`) to the current
/// selection. A no-op when nothing is selected (the whole canvas is implied).
fn modify_selection(
    bus: &mut CommandBus,
    kind: HistoryKind,
    f: impl FnOnce(&mut SelectionMask),
) -> Result<(), DocumentError> {
    let Some(mut mask) = bus.document.selection.clone() else {
        return Ok(());
    };
    f(&mut mask);
    bus.apply(Box::new(SetSelection::replace(mask).with_kind(kind)))
}

/// `true` when pixel edits on the layer at `index` are refused: it exists
/// and is locked itself or through a parent group, or it is a group, which
/// has no pixels to edit (docs/specs/image-layer-groups.md AC-11, AC-12).
pub fn layer_locked(doc: &Document, index: usize) -> bool {
    doc.layers()
        .get(index)
        .is_some_and(|l| l.is_group() || doc.effectively_locked(index))
}

/// The settings of a transform command; non-finite numbers fall back to
/// the identity's (a centred pivot, no turn, 1 px).
fn transform_settings(
    center: (f64, f64),
    size: (f64, f64),
    angle: f64,
    pivot: (f64, f64),
    quality: u32,
) -> super::TransformSettings {
    let or = |v: f64, d: f64| if v.is_finite() { v } else { d };
    super::TransformSettings {
        center: (or(center.0, 0.0), or(center.1, 0.0)),
        size: (or(size.0, 1.0), or(size.1, 1.0)),
        angle: or(angle, 0.0),
        pivot: (or(pivot.0, 0.5), or(pivot.1, 0.5)),
        quality: pincel_effects::transform::Quality::from_index(quality),
    }
}

#[cfg(test)]
mod tests {
    use super::CommandSpec;

    mod groups {
        use super::super::{CommandBus, CommandSpec, StrokeScratch, layer_locked, run_command};
        use crate::Color;
        use crate::document::tree::tests::tree_doc;

        fn parse(json: &str) -> CommandSpec {
            serde_json::from_str(json).expect("parses")
        }

        /// bg, G { a, H { b } }, each raster layer with one opaque pixel at
        /// its own spot.
        fn bus() -> CommandBus {
            let mut doc = tree_doc(&[
                ("bg", None, false),
                ("G", None, true),
                ("a", Some("G"), false),
                ("H", Some("G"), true),
                ("b", Some("H"), false),
            ]);
            for (i, x) in [(0, 0), (2, 1), (4, 2)] {
                doc.layers[i]
                    .pixels
                    .set_pixel(x, 0, Color::rgba(255, 0, 0, 255));
            }
            CommandBus::new(doc)
        }

        /// Edits that write a layer's own pixels: refused on a group.
        const PIXEL_ONLY_EDITS: [&str; 5] = [
            r#"{"type":"pencil_stroke","layer":L,"size":1,"color":[0,0,0,255],"opacity":1,"points":[[1,1]],"stroke_id":1}"#,
            r#"{"type":"fill_bucket","layer":L,"x":1,"y":1,"color":[0,0,0,255],"opacity":1,"tolerance":0,"contiguous":true}"#,
            r#"{"type":"translate_selection","layer":L,"dx":1,"dy":0}"#,
            r#"{"type":"delete_selection","layer":L}"#,
            r#"{"type":"fill_selection","layer":L,"color":[0,0,0,255]}"#,
        ];

        /// Move (layer): allowed on a group, where it moves the descendants.
        const TRANSLATE_LAYER: &str = r#"{"type":"translate_layer","layer":L,"dx":1,"dy":0}"#;

        fn edit(json: &str, layer: usize) -> CommandSpec {
            parse(&json.replace(":L", &format!(":{layer}")))
        }

        #[test]
        fn locked_group_refuses_edits_on_its_descendants() {
            let mut bus = bus();
            bus.document.layers[1].locked = true;
            for layer in [2, 4] {
                assert!(layer_locked(&bus.document, layer), "{layer}");
                for json in PIXEL_ONLY_EDITS.into_iter().chain([TRANSLATE_LAYER]) {
                    assert!(
                        edit(json, layer).refused(&bus.document),
                        "{json} on {layer}"
                    );
                }
            }
            assert!(!edit(PIXEL_ONLY_EDITS[0], 0).refused(&bus.document));
            assert!(
                !bus.document.layers[2].locked,
                "the child's own flag is untouched"
            );
        }

        #[test]
        fn pixel_edits_on_a_group_are_refused() {
            let bus = bus();
            assert!(layer_locked(&bus.document, 1));
            for json in PIXEL_ONLY_EDITS {
                assert!(edit(json, 1).refused(&bus.document), "{json}");
            }
            // Move (layer) on a group is allowed: it moves its content.
            assert!(!edit(TRANSLATE_LAYER, 1).refused(&bus.document));
        }

        #[test]
        fn move_and_flip_on_a_group_transform_all_raster_descendants_in_one_undo_step() {
            let mut bus = bus();
            let before = bus.document.layers().to_vec();
            let mut scratch = StrokeScratch::default();
            for json in [
                r#"{"type":"translate_layer","layer":1,"dx":0,"dy":2}"#,
                r#"{"type":"transform_layer","layer":1,"op":"flip_h"}"#,
                r#"{"type":"rotate_layer_90","layer":1,"ccw":false}"#,
            ] {
                let depth = bus.history.undo_depth();
                run_command(&mut bus, parse(json), &mut scratch, &[]).expect(json);
                assert_eq!(bus.history.undo_depth(), depth + 1, "{json}: one step");
                let doc = &bus.document;
                assert_eq!(doc.layers()[0], before[0], "{json}: bg untouched");
                assert_ne!(doc.layers()[2].pixels, before[2].pixels, "{json}: a moved");
                assert_ne!(doc.layers()[4].pixels, before[4].pixels, "{json}: b moved");
                assert_eq!(
                    doc.layers()[1].pixels.width(),
                    0,
                    "{json}: group stays empty"
                );
                assert!(bus.undo().unwrap());
                assert_eq!(bus.document.layers(), before.as_slice(), "{json}: undone");
            }
        }

        #[test]
        fn group_wire_commands_add_move_and_merge_groups() {
            let mut bus = bus();
            let mut scratch = StrokeScratch::default();
            let mut run = |bus: &mut CommandBus, json: &str| {
                run_command(bus, parse(json), &mut scratch, &[]).expect(json);
            };
            run(&mut bus, r#"{"type":"add_group"}"#);
            assert_eq!(bus.document.layers()[5].name, "Group 3");
            assert_eq!(bus.document.active_layer_index(), 5);
            run(
                &mut bus,
                r#"{"type":"move_layer_to","index":0,"target":5,"place":"into"}"#,
            );
            assert_eq!(bus.document.layers()[4].name, "Group 3");
            assert_eq!(bus.document.parent_index(5), Some(4));
            run(
                &mut bus,
                r#"{"type":"move_layer_to","index":5,"target":0,"place":"below"}"#,
            );
            assert_eq!(bus.document.layers()[0].name, "bg");
            assert_eq!(bus.document.parent_index(0), None);
            run(
                &mut bus,
                r#"{"type":"move_layer_to","index":0,"target":1,"place":"above"}"#,
            );
            assert_eq!(bus.document.parent_index(4), None, "above the whole group");
            run(&mut bus, r#"{"type":"merge_group","index":0}"#);
            assert!(!bus.document.layers()[0].is_group());
            assert!(bus.document.is_well_formed());
        }

        #[test]
        fn move_on_a_group_with_a_locked_descendant_is_refused() {
            let mut bus = bus();
            assert!(
                !edit(TRANSLATE_LAYER, 1).refused(&bus.document),
                "allowed while nothing below is locked"
            );
            bus.document.layers[4].locked = true;
            assert!(edit(TRANSLATE_LAYER, 1).refused(&bus.document));
        }
    }

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
            r#"{"type":"fill_selection","layer":2,"color":[0,0,0,255]}"#,
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
            assert!(
                super::run_command(&mut bus, spec, &mut super::StrokeScratch::default(), &[])
                    .is_ok()
            );
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
            assert!(
                super::run_command(bus, parse(json), &mut super::StrokeScratch::default(), &[])
                    .is_ok()
            );
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
        assert!(
            super::run_command(
                &mut bus,
                parse(rect),
                &mut super::StrokeScratch::default(),
                &[]
            )
            .is_ok()
        );
        let before = bus.document.selection.clone();
        let depth = bus.history.undo_depth();

        let invert = parse(r#"{"type":"invert_selection"}"#);
        assert!(
            super::run_command(&mut bus, invert, &mut super::StrokeScratch::default(), &[]).is_ok()
        );
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
        assert!(
            super::run_command(
                &mut bus,
                parse(dab),
                &mut super::StrokeScratch::default(),
                &[]
            )
            .is_ok()
        );
        assert!(
            bus.document.layers()[0]
                .pixels
                .get_pixel(2, 2)
                .is_some_and(|c| c.a > 0)
        );
        let rect = r#"{"type":"select_rectangle","x":1,"y":1,"w":3,"h":3,"mode":"replace"}"#;
        assert!(
            super::run_command(
                &mut bus,
                parse(rect),
                &mut super::StrokeScratch::default(),
                &[]
            )
            .is_ok()
        );
        let pixels = bus.document.layers()[0].pixels.clone();
        let mask = bus.document.selection.clone().expect("a selection");
        let depth = bus.history.undo_depth();

        let shift = parse(r#"{"type":"translate_selection_mask","dx":3,"dy":2}"#);
        assert_eq!(shift.edited_layer(), None, "writes no pixels");
        assert!(
            super::run_command(&mut bus, shift, &mut super::StrokeScratch::default(), &[]).is_ok()
        );
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
                if angle == crate::DEFAULT_BRUSH_ANGLE && azimuth.is_empty()
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
