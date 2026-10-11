//! Composition pipeline. See `docs/specs/pincel.md` §4.
//!
//! Covers the RGBA color mode, image cels and tilemap cels, and every
//! separable blend mode (the four non-separable HSL modes fall back to
//! `Normal` — spec §15 Decision Log, 2026-07-09), plus the onion-skin
//! pass (spec §4.2). Indexed color, linked cels and decoration overlays
//! raise [`RenderError`] and arrive in later milestones.

mod blend;
mod compose;
mod error;
mod image_layer;
mod onion;
mod request;
mod tilemap_layer;

#[cfg(test)]
mod test_support;

pub use compose::compose;
pub use error::RenderError;
pub use request::{ComposeRequest, ComposeResult, LayerFilter, OnionSkin, Overlays};

pub(crate) use blend::{blend_pixel_into, mul_u8};
pub(crate) use image_layer::composite_image_cel;
