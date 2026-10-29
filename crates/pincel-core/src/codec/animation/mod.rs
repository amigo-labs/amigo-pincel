//! Animated export (APNG, GIF). See `docs/specs/pincel.md` §7.5 and
//! `docs/specs/animated-export.md`.
//!
//! Every frame of the sequence is composed at zoom 1 over the full canvas
//! (exactly what [`export_frame_png`](super::export_frame_png) writes),
//! upscaled by an integer nearest-neighbour factor and handed to the
//! format writer one at a time, so memory is bounded by one scaled frame
//! plus the output bytes.

mod apng;
mod gif;
mod quantize;
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
    /// GIF89a with one 8-bit palette per frame and 1-bit transparency.
    Gif,
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
        AnimationFormat::Gif => gif::write_gif(&mut source, &frames, width, height, opts.loops),
    }
}

/// What the export dialog shows before exporting: see [`animation_info`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationInfo {
    /// Number of frames the export contains (pingpong counts the way back).
    pub frame_count: u32,
    /// Default play count: the tag's repeat count when it is non-zero,
    /// otherwise `0` (forever).
    pub default_loops: u16,
    /// At least one frame does not fit a 256-entry GIF colour table (its
    /// distinct opaque colours plus one transparent entry), so a GIF
    /// export approximates its colours.
    pub gif_approximated: bool,
    /// At least one frame has partially transparent pixels (alpha 1–254),
    /// which GIF reduces to on / off.
    pub semi_transparent: bool,
}

/// Describe the animation `tag` selects (`None` = every frame) without
/// encoding it. Each distinct frame is composed once at zoom 1; the scale
/// changes neither the colours nor the alpha.
///
/// # Errors
///
/// As [`export_animation`] for the tag, an empty sequence and anything
/// `compose()` refuses.
pub fn animation_info(
    sprite: &Sprite,
    cels: &CelMap,
    tag: Option<&str>,
) -> Result<AnimationInfo, ExportError> {
    let frames = animation_frames(sprite, tag)?;
    if frames.is_empty() {
        return Err(ExportError::NoFrames);
    }
    let default_loops = tag
        .and_then(|name| sprite.tags.iter().find(|t| t.name == name))
        .map_or(0, |t| t.repeats);
    let mut distinct = frames.clone();
    distinct.sort_unstable();
    distinct.dedup();
    let mut source = FrameSource::new(sprite, cels, 1);
    let mut gif_approximated = false;
    let mut semi_transparent = false;
    for frame in distinct {
        let pixels = source.render(frame)?;
        gif_approximated |= !gif::fits_table(pixels);
        semi_transparent |= gif::has_partial_alpha(pixels);
    }
    Ok(AnimationInfo {
        frame_count: u32::try_from(frames.len()).unwrap_or(u32::MAX),
        default_loops,
        gif_approximated,
        semi_transparent,
    })
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
    use super::test_support::{sprite_from, three_frames};
    use super::*;
    use crate::document::{ColorMode, Frame, Layer, LayerId};
    use crate::render::RenderError;

    #[test]
    fn animation_info_counts_the_sequence_frames() {
        let mut pp = crate::document::Tag::new("pp", FrameIndex::new(0), FrameIndex::new(2));
        pp.direction = crate::document::TagDirection::Pingpong;
        let (sprite, cels) = three_frames(vec![pp]);
        let all = animation_info(&sprite, &cels, None).expect("info");
        assert_eq!(all.frame_count, 3);
        let tagged = animation_info(&sprite, &cels, Some("pp")).expect("info");
        assert_eq!(tagged.frame_count, 4);
    }

    #[test]
    fn animation_info_default_loops_follow_the_tag_repeats() {
        let mut thrice =
            crate::document::Tag::new("thrice", FrameIndex::new(0), FrameIndex::new(1));
        thrice.repeats = 3;
        let endless = crate::document::Tag::new("endless", FrameIndex::new(1), FrameIndex::new(2));
        let (sprite, cels) = three_frames(vec![thrice, endless]);
        let loops = |tag| {
            animation_info(&sprite, &cels, tag)
                .expect("info")
                .default_loops
        };
        assert_eq!(loops(None), 0);
        assert_eq!(loops(Some("thrice")), 3);
        assert_eq!(loops(Some("endless")), 0);
    }

    #[test]
    fn animation_info_flags_frames_over_the_gif_table() {
        let (sprite, cels) = three_frames(Vec::new());
        assert!(
            !animation_info(&sprite, &cels, None)
                .expect("info")
                .gif_approximated
        );
        // Frame 1: 256 distinct opaque colours plus transparent pixels.
        let mut many = vec![0u8; 17 * 16 * 4];
        for i in 0..256u32 {
            let at = (i * 4) as usize;
            many[at..at + 4].copy_from_slice(&[i as u8, (i / 2) as u8, 7, 255]);
        }
        let plain = vec![0u8; 17 * 16 * 4];
        let (sprite, cels) = sprite_from(17, 16, &[(100, plain), (100, many)], Vec::new());
        let info = animation_info(&sprite, &cels, None).expect("info");
        assert!(info.gif_approximated);
        assert!(!info.semi_transparent);
    }

    #[test]
    fn animation_info_flags_semi_transparent_pixels() {
        let half = vec![10, 20, 30, 128, 0, 0, 0, 0, 1, 2, 3, 255, 0, 0, 0, 0];
        let (sprite, cels) = sprite_from(2, 2, &[(100, half)], Vec::new());
        assert!(
            animation_info(&sprite, &cels, None)
                .expect("info")
                .semi_transparent
        );
        let (sprite, cels) = three_frames(Vec::new());
        assert!(
            !animation_info(&sprite, &cels, None)
                .expect("info")
                .semi_transparent
        );
    }

    #[test]
    fn animation_info_unknown_tag_returns_err() {
        let (sprite, cels) = three_frames(Vec::new());
        assert!(matches!(
            animation_info(&sprite, &cels, Some("nope")),
            Err(ExportError::TagNotFound { .. })
        ));
    }

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
