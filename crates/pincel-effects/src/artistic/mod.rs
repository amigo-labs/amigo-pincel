//! paint.net's Artistic menu (docs/specs/effects-artistic-photo.md): Ink
//! Sketch, Oil Painting and Pencil Sketch. Each keeps the source alpha and
//! reads the BT.709 luma of the straight colours (Ink Sketch weights it by
//! alpha for its edges).

mod ink_sketch;
mod oil_painting;
mod pencil_sketch;

pub use ink_sketch::InkSketch;
pub use oil_painting::OilPainting;
pub use pencil_sketch::PencilSketch;

/// BT.709 luma of a pixel's straight RGB, `0.0..=255.0`.
pub(crate) fn luma(p: &[u8]) -> f32 {
    0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
}
