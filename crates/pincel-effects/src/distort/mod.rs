//! Distort effects: Emboss, Edge Detect, Relief (Fineliner spec §11.3; in
//! the Stylize submenu since `effect-catalog-v2`) and paint.net's Distort
//! menu (docs/specs/effects-distort.md).
//!
//! [`StylizeOutline`] (Stylize ▸ Outline, docs/specs/
//! effects-object-stylize-color-noise.md) darkens pixels near colour edges.
//!
//! Emboss, Edge Detect and Relief read a luminance height field and its
//! gradient; alpha is preserved from the source. The height field is
//! premultiplied luma (luma · alpha), so the invisible colour of a fully
//! transparent pixel never shows up in its opaque neighbours
//! (docs/specs/effects-adjustments.md AC-18).
//!
//! Bulge, Dents, Polar Inversion, Tile Reflection and Twist resample
//! premultiplied pixels bilinearly with an [`EdgeBehavior`]; Crystalize
//! averages premultiplied cells. Frosted Glass, Morphology and Pixelate's
//! Centre colour copy whole pixels.

mod crystalize;
mod dents;
pub(crate) mod edge;
mod edge_detect;
mod emboss;
mod frosted_glass;
mod morphology;
mod pixelate;
mod relief;
mod stylize_outline;
mod warp;

pub use crystalize::Crystalize;
pub use dents::Dents;
pub use edge::EdgeBehavior;
pub use edge_detect::{EdgeAlgorithm, EdgeDetect};
pub use emboss::Emboss;
pub use frosted_glass::FrostedGlass;
pub use morphology::{Morphology, MorphologyOp, MorphologyShape};
pub use pixelate::{Pixelate, PixelateColor};
pub use relief::Relief;
pub use stylize_outline::StylizeOutline;
pub use warp::{Bulge, PolarInversion, TileReflection, Twist};

/// Premultiplied Rec.601 luma (luma · alpha) of pixel `(x, y)` in
/// `[0.0, 1.0]`, clamping at the edges.
pub(crate) fn luma_at(data: &[u8], w: usize, h: usize, x: i32, y: i32) -> f32 {
    let cx = x.clamp(0, w as i32 - 1) as usize;
    let cy = y.clamp(0, h as i32 - 1) as usize;
    let o = (cy * w + cx) * 4;
    let luma =
        (0.299 * data[o] as f32 + 0.587 * data[o + 1] as f32 + 0.114 * data[o + 2] as f32) / 255.0;
    luma * data[o + 3] as f32 / 255.0
}

/// Normalised luminance gradient `(gx, gy)` at `(x, y)`, each roughly in
/// `[-1, 1]`. `mid` is the centre tap weight: 2.0 gives Sobel, 1.0 Prewitt.
pub(crate) fn gradient(data: &[u8], w: usize, h: usize, x: i32, y: i32, mid: f32) -> (f32, f32) {
    let l = |dx, dy| luma_at(data, w, h, x + dx, y + dy);
    let (tl, tc, tr) = (l(-1, -1), l(0, -1), l(1, -1));
    let (ml, mr) = (l(-1, 0), l(1, 0));
    let (bl, bc, br) = (l(-1, 1), l(0, 1), l(1, 1));
    let norm = 2.0 + mid;
    let gx = ((tr + mid * mr + br) - (tl + mid * ml + bl)) / norm;
    let gy = ((bl + mid * bc + br) - (tl + mid * tc + tr)) / norm;
    (gx, gy)
}
