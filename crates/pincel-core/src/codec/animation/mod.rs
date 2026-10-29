//! Animated export (APNG, GIF). See `docs/specs/pincel.md` §7.5 and
//! `docs/specs/animated-export.md`.
//!
//! Every frame of the sequence is composed at zoom 1 over the full canvas
//! (exactly what [`export_frame_png`](super::export_frame_png) writes),
//! upscaled by an integer nearest-neighbour factor and handed to the
//! format writer one at a time, so memory is bounded by one scaled frame
//! plus the output bytes.

mod apng;
mod sequence;
#[cfg(test)]
mod test_support;

pub use sequence::animation_frames;

use super::png::ExportError;
use super::scale::{scaled_size, upscale_nearest};
use crate::document::{CelMap, FrameIndex, Sprite};
use crate::render::{ComposeRequest, compose};

/// File format of an animated export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationFormat {
    /// Animated PNG, written with the `.png` extension.
    Apng,
}

/// Knobs for [`export_animation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationOptions {
    /// Output format.
    pub format: AnimationFormat,
    /// `None` exports every frame forward; `Some(name)` exports the named
    /// tag in its direction (see [`animation_frames`]).
    pub tag: Option<String>,
    /// Integer nearest-neighbour scale, `1..=10`.
    pub scale: u32,
    /// How often the animation plays in total; `0` plays forever.
    pub loops: u16,
}

impl AnimationOptions {
    /// Every frame, scale 1, playing forever.
    pub fn new(format: AnimationFormat) -> Self {
        Self {
            format,
            tag: None,
            scale: 1,
            loops: 0,
        }
    }

    /// Restrict the export to the named tag.
    pub fn with_tag(mut self, name: impl Into<String>) -> Self {
        self.tag = Some(name.into());
        self
    }

    /// Set the integer scale factor (`1..=10`).
    pub fn with_scale(mut self, scale: u32) -> Self {
        self.scale = scale;
        self
    }

    /// Set the total play count; `0` plays forever.
    pub fn with_loops(mut self, loops: u16) -> Self {
        self.loops = loops;
        self
    }
}

/// Encode the animation selected by `opts` as APNG or GIF bytes.
///
/// # Limitations
///
/// Every frame goes through `compose()`, so a sprite it refuses (an
/// indexed or grayscale sprite, a linked cel) fails the export with
/// [`ExportError::Render`] before any byte is produced.
///
/// # Errors
///
/// [`ExportError::TagNotFound`] / [`ExportError::TagRangeInvalid`] for a
/// bad tag, [`ExportError::NoFrames`] for an empty sequence,
/// [`ExportError::InvalidScale`] for a scale outside `1..=10`,
/// [`ExportError::ScaledTooLarge`] when an edge would exceed 16384 px,
/// [`ExportError::Render`] / [`ExportError::Encode`] from compose and the
/// encoder.
pub fn export_animation(
    sprite: &Sprite,
    cels: &CelMap,
    opts: &AnimationOptions,
) -> Result<Vec<u8>, ExportError> {
    let frames = animation_frames(sprite, opts.tag.as_deref())?;
    if frames.is_empty() {
        return Err(ExportError::NoFrames);
    }
    let (width, height) = scaled_size(sprite.width, sprite.height, opts.scale)?;
    let mut source = FrameSource::new(sprite, cels, opts.scale);
    match opts.format {
        AnimationFormat::Apng => apng::write_apng(&mut source, &frames, width, height, opts.loops),
    }
}

/// Composes and upscales one frame at a time into reused buffers.
struct FrameSource<'a> {
    sprite: &'a Sprite,
    cels: &'a CelMap,
    scale: u32,
    composed: Vec<u8>,
    scaled: Vec<u8>,
}

impl<'a> FrameSource<'a> {
    fn new(sprite: &'a Sprite, cels: &'a CelMap, scale: u32) -> Self {
        Self {
            sprite,
            cels,
            scale,
            composed: Vec::new(),
            scaled: Vec::new(),
        }
    }

    /// Display duration of `frame` in milliseconds.
    fn duration_ms(&self, frame: FrameIndex) -> u16 {
        self.sprite
            .frames
            .get(frame.0 as usize)
            .map_or(0, |f| f.duration_ms)
    }

    /// The scaled RGBA8 pixels of `frame`, valid until the next call.
    fn render(&mut self, frame: FrameIndex) -> Result<&[u8], ExportError> {
        let request = ComposeRequest::full(frame, self.sprite.width, self.sprite.height);
        compose(self.sprite, self.cels, &request, &mut self.composed)?;
        if self.scale == 1 {
            return Ok(&self.composed);
        }
        upscale_nearest(
            &self.composed,
            self.sprite.width,
            self.sprite.height,
            self.scale,
            &mut self.scaled,
        );
        Ok(&self.scaled)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::three_frames;
    use super::*;
    use crate::document::{ColorMode, Frame, Layer, LayerId};
    use crate::render::RenderError;

    #[test]
    fn export_animation_frameless_sprite_returns_no_frames() {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .build()
            .expect("sprite builds");
        let err = export_animation(
            &sprite,
            &CelMap::new(),
            &AnimationOptions::new(AnimationFormat::Apng),
        )
        .expect_err("no frames");
        assert!(matches!(err, ExportError::NoFrames));
    }

    #[test]
    fn export_animation_indexed_sprite_returns_render_error() {
        let sprite = Sprite::builder(2, 2)
            .color_mode(ColorMode::Indexed {
                transparent_index: 0,
            })
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let err = export_animation(
            &sprite,
            &CelMap::new(),
            &AnimationOptions::new(AnimationFormat::Apng),
        )
        .expect_err("indexed refused");
        assert!(matches!(
            err,
            ExportError::Render(RenderError::UnsupportedColorMode { .. })
        ));
    }

    #[test]
    fn export_animation_scale_out_of_range_returns_err() {
        let (sprite, cels) = three_frames(Vec::new());
        for scale in [0, 11] {
            let opts = AnimationOptions::new(AnimationFormat::Apng).with_scale(scale);
            assert!(matches!(
                export_animation(&sprite, &cels, &opts),
                Err(ExportError::InvalidScale { .. })
            ));
        }
    }

    #[test]
    fn export_animation_unknown_tag_returns_err() {
        let (sprite, cels) = three_frames(Vec::new());
        let opts = AnimationOptions::new(AnimationFormat::Apng).with_tag("nope");
        assert!(matches!(
            export_animation(&sprite, &cels, &opts),
            Err(ExportError::TagNotFound { .. })
        ));
    }

    #[test]
    fn export_animation_over_edge_limit_returns_err_naming_limit() {
        let sprite = Sprite::builder(1639, 1)
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let opts = AnimationOptions::new(AnimationFormat::Apng).with_scale(10);
        let err = export_animation(&sprite, &CelMap::new(), &opts).expect_err("too large");
        assert!(matches!(err, ExportError::ScaledTooLarge { .. }));
        assert!(err.to_string().contains("16384"));
    }
}
