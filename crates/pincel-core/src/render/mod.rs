//! Composition pipeline. See `docs/specs/pincel.md` §4.
//!
//! M3 covers the RGBA color mode and image cels with the `Normal` blend
//! mode. Tilemap layers, group layers, indexed color, linked cels, non-Normal
//! blend modes, onion skin, and decoration overlays all raise
//! [`RenderError`] and arrive in later milestones. The `dirty_hint` field
//! on the request is accepted but currently ignored (Phase 1.5, see spec
//! §4.3).

mod blend;
mod compose;
mod error;
mod image_layer;
mod request;
mod tilemap_layer;

#[cfg(test)]
mod test_support;

pub use compose::compose;
pub use error::RenderError;
pub use request::{ComposeRequest, ComposeResult, LayerFilter, OnionSkin, Overlays};
