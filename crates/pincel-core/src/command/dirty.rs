//! [`DirtyRegion`] — the per-command description of what a successfully
//! applied (or reverted) command changed on the document. See
//! `docs/specs/pincel.md` §4.3.
//!
//! Today every command in the codebase falls back to the default trait
//! impl that returns [`DirtyRegion::Canvas`], so consumers behave exactly
//! like the pre-M12.3 world. Subsequent slices refine the high-frequency
//! paint commands (SetPixel, DrawLine, DrawRectangle, …) to report a
//! precise [`DirtyRegion::Layer`] rect so the UI render adapter can call
//! `compose()` with a matching `dirty_hint` and blit only the changed
//! sub-rect.

use crate::document::{FrameIndex, LayerId};
use crate::geometry::Rect;

/// What a command changed on the document.
///
/// Returned by [`super::Command::dirty_region`] and propagated through
/// [`super::AnyCommand::dirty_region`]. Consumers translate it into a
/// `compose()` `dirty_hint` (and, on the wasm boundary, into the
/// `dirty-rect` / `dirty-canvas` event the UI listens to).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyRegion {
    /// The command did not change any composite-visible state, or it has
    /// not been applied yet. Consumers should not emit a dirty event.
    None,
    /// The whole composited canvas may have changed — used for structural
    /// commands (add / remove layer, frame, slice, tileset) and as the
    /// default for commands that have not yet been refined.
    Canvas,
    /// A specific axis-aligned sub-rect on a single `(layer, frame)`
    /// pair. Rect coordinates are in sprite space.
    Layer {
        layer: LayerId,
        frame: FrameIndex,
        rect: Rect,
    },
}

impl DirtyRegion {
    /// Convenience constructor for the single-cel rect case.
    pub fn layer_rect(layer: LayerId, frame: FrameIndex, rect: Rect) -> Self {
        Self::Layer { layer, frame, rect }
    }

    /// Returns `true` when the region implies no UI repaint is needed.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_rect_helper_matches_struct_variant() {
        let r = Rect::new(2, 3, 4, 5);
        let region = DirtyRegion::layer_rect(LayerId::new(7), FrameIndex::new(1), r);
        assert_eq!(
            region,
            DirtyRegion::Layer {
                layer: LayerId::new(7),
                frame: FrameIndex::new(1),
                rect: r,
            }
        );
    }

    #[test]
    fn is_none_only_true_for_none_variant() {
        assert!(DirtyRegion::None.is_none());
        assert!(!DirtyRegion::Canvas.is_none());
        assert!(
            !DirtyRegion::layer_rect(LayerId::new(0), FrameIndex::new(0), Rect::new(0, 0, 1, 1))
                .is_none()
        );
    }
}
