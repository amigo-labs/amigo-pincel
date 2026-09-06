//! Distort effects (Fineliner spec §11.3): Emboss, Edge Detect, Relief.
//!
//! All three read a luminance height field and its gradient. They operate on
//! straight RGB (alpha is preserved from the source) rather than premultiplied
//! space: the output is a lit/edge greyscale or a colour-preserving relief, so
//! transparent-pixel colour bleed is not a concern in practice.

mod edge_detect;
mod emboss;
mod relief;

pub use edge_detect::{EdgeAlgorithm, EdgeDetect};
pub use emboss::Emboss;
pub use relief::Relief;

/// Rec.601 luma of pixel `(x, y)` in `[0.0, 1.0]`, clamping at the edges.
pub(crate) fn luma_at(data: &[u8], w: usize, h: usize, x: i32, y: i32) -> f32 {
    let cx = x.clamp(0, w as i32 - 1) as usize;
    let cy = y.clamp(0, h as i32 - 1) as usize;
    let o = (cy * w + cx) * 4;
    (0.299 * data[o] as f32 + 0.587 * data[o + 1] as f32 + 0.114 * data[o + 2] as f32) / 255.0
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
