//! Composition pipeline. See `docs/specs/pincel.md` §4.
//!
//! Covers every color mode (RGBA, indexed through the palette, grayscale —
//! Decision Log 2026-10-29), image cels and tilemap cels, and every
//! blend mode — separable and the four non-separable HSL modes (spec §15
//! Decision Log, 2026-10-27), linked cels (followed to their target) — plus
//! the onion-skin pass (spec §4.2). Decoration overlays raise
//! [`RenderError`] and arrive in a later milestone.

mod blend;
mod compose;
mod error;
mod image_layer;
mod onion;
mod request;
mod source;
mod tilemap_layer;

#[cfg(test)]
mod test_support;

pub use compose::compose;
pub use error::RenderError;
pub use request::{ComposeRequest, ComposeResult, LayerFilter, OnionSkin, Overlays};

pub(crate) use blend::{blend_pixel_into, mul_u8};
pub(crate) use image_layer::composite_image_cel;
pub(crate) use source::Paint;
