//! The [`Effect`] convention: a stateless transform from pixels to pixels.

use crate::image::EffectImage;

/// A stateless image effect (ADR-002): pure `apply`, no document, no state.
///
/// Each effect is a small parameter struct that implements this trait. Effects
/// with spatial parameters (a blur radius, a motion distance) also implement
/// [`Effect::scaled`] so previews can run on a downscaled copy with matching
/// parameters; the default [`Effect::preview`] uses it.
pub trait Effect: Sized {
    /// Applies the effect at full resolution, returning a new buffer the same
    /// size as `src`.
    fn apply(&self, src: &EffectImage) -> EffectImage;

    /// Returns a copy of this effect with spatial parameters multiplied by
    /// `factor` (`0.0 < factor <= 1.0` for previews).
    ///
    /// Effects with spatial extent (a blur radius, a motion distance) scale
    /// their parameters; effects with none return an identical copy of
    /// themselves so [`Effect::preview`] stays correct without a special case.
    fn scaled(&self, factor: f32) -> Self;

    /// Renders a preview no larger than `max_dim` on its longest side.
    ///
    /// If the source already fits, the effect is applied directly. Otherwise the
    /// source is downscaled, [`Effect::scaled`] adjusts the parameters to match,
    /// and the effect runs on the small copy — the caller upscales for display.
    fn preview(&self, src: &EffectImage, max_dim: u32) -> EffectImage {
        let longest = src.width().max(src.height());
        if max_dim == 0 || longest <= max_dim {
            return self.apply(src);
        }
        let factor = max_dim as f32 / longest as f32;
        let w = ((src.width() as f32 * factor).round() as u32).max(1);
        let h = ((src.height() as f32 * factor).round() as u32).max(1);
        let small = src.resized(w, h);
        self.scaled(factor).apply(&small)
    }
}
