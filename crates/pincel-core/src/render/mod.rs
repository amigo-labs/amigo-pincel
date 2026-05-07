//! Composition pipeline. See `docs/specs/pincel.md` §4.
//!
//! M3 covers the RGBA color mode and image cels with the `Normal` blend
//! mode. Tilemap layers, group layers, indexed color, onion skin, overlays,
//! and non-Normal blend modes raise [`RenderError`] and arrive in later
//! milestones.

mod compose;
mod request;

pub use compose::{RenderError, compose};
pub use request::{ComposeRequest, ComposeResult, LayerFilter, OnionSkin, Overlays};
