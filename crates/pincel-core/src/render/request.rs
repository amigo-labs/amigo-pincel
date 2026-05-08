//! Request and result types for [`super::compose`]. See `docs/specs/pincel.md` §4.1.

use crate::document::{FrameIndex, LayerId};
use crate::geometry::Rect;

/// The set of layers to composite.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LayerFilter {
    /// All layers whose `visible` flag is `true` (the default).
    #[default]
    Visible,
    /// All layers, including hidden ones — used for export-all flows.
    All,
    /// Only the listed layers (solo mode). Order is irrelevant; the
    /// renderer still composes in the sprite's z-order.
    Only(Vec<LayerId>),
}

/// Decoration overlays drawn on top of the composited frame. M3 leaves
/// every overlay disabled; they wire up in later milestones.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Overlays {
    pub grid: bool,
    pub slices: bool,
    pub selection_marquee: bool,
}

/// Onion-skin parameters. Both alphas are `0..=255`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnionSkin {
    /// Alpha applied to frames before the current one (rendered tinted red).
    /// Default is `76` ≈ `0.3 * 255` per spec §4.2.
    pub previous_alpha: u8,
    /// Alpha applied to frames after the current one (rendered tinted blue).
    pub next_alpha: u8,
    /// How many frames back to render. `0` disables the previous-frames pass.
    pub frames_back: u32,
    /// How many frames forward to render. `0` disables the next-frames pass.
    pub frames_forward: u32,
}

impl Default for OnionSkin {
    fn default() -> Self {
        Self {
            previous_alpha: 76,
            next_alpha: 76,
            frames_back: 1,
            frames_forward: 1,
        }
    }
}

/// Describes what `compose()` should produce. See `docs/specs/pincel.md` §4.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeRequest {
    /// Which frame to render.
    pub frame: FrameIndex,
    /// The visible region in sprite coordinates.
    pub viewport: Rect,
    /// Integer upscale factor, `1..=64`. `1` means 1:1 pixels.
    pub zoom: u32,
    /// Optional onion-skin overlay. `None` disables.
    pub onion_skin: Option<OnionSkin>,
    /// Which layers to composite.
    pub include_layers: LayerFilter,
    /// Decoration overlays.
    pub overlays: Overlays,
    /// Optional sub-rect hint for incremental repaint. M3 ignores this and
    /// always fills the viewport; M12 will honor it.
    pub dirty_hint: Option<Rect>,
}

impl ComposeRequest {
    /// Render the given frame at 1:1 zoom over the full sprite canvas with
    /// default filters and no overlays. Convenience for tests and the
    /// initial UI bring-up.
    pub fn full(frame: FrameIndex, sprite_w: u32, sprite_h: u32) -> Self {
        Self {
            frame,
            viewport: Rect::new(0, 0, sprite_w, sprite_h),
            zoom: 1,
            onion_skin: None,
            include_layers: LayerFilter::Visible,
            overlays: Overlays::default(),
            dirty_hint: None,
        }
    }
}

/// The output of `compose()`: an RGBA8 pixel buffer in row-major order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposeResult {
    /// `width * height * 4` bytes. Non-premultiplied RGBA8.
    pub pixels: Vec<u8>,
    /// `viewport.width * zoom`.
    pub width: u32,
    /// `viewport.height * zoom`.
    pub height: u32,
    /// Monotonic counter the caller may use to detect staleness. `compose()`
    /// is pure and always returns `0`; the UI layer is expected to maintain
    /// the counter itself across calls.
    pub generation: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_request_uses_default_filters() {
        let req = ComposeRequest::full(FrameIndex::new(0), 32, 16);
        assert_eq!(req.viewport, Rect::new(0, 0, 32, 16));
        assert_eq!(req.zoom, 1);
        assert!(req.onion_skin.is_none());
        assert_eq!(req.include_layers, LayerFilter::Visible);
        assert_eq!(req.overlays, Overlays::default());
        assert!(req.dirty_hint.is_none());
    }

    #[test]
    fn layer_filter_default_is_visible() {
        assert_eq!(LayerFilter::default(), LayerFilter::Visible);
    }

    #[test]
    fn overlays_default_disables_everything() {
        let o = Overlays::default();
        assert!(!o.grid);
        assert!(!o.slices);
        assert!(!o.selection_marquee);
    }

    #[test]
    fn onion_skin_default_matches_spec() {
        let o = OnionSkin::default();
        assert_eq!(o.previous_alpha, 76);
        assert_eq!(o.next_alpha, 76);
        assert_eq!(o.frames_back, 1);
        assert_eq!(o.frames_forward, 1);
    }
}
