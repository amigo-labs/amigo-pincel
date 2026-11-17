//! Animated image import: GIF, APNG and animated WebP into full-canvas
//! RGBA frames with durations (docs/specs/animated-image-import.md).
//!
//! [`probe_animation`] tells whether a file is animated (more than one
//! frame) and how large it is, decoding only the first frame;
//! [`import_animation`] decodes every frame onto the animation canvas
//! (offsets, blending and disposal applied) and converts the delays to
//! Pixel-mode frame durations.

mod apng;
mod canvas;
mod gif;
mod webp;

use thiserror::Error;

use canvas::Compositor;

/// Largest canvas width or height an animation may have to open as one.
pub const ANIMATION_MAX_SIDE: u32 = 4096;

/// Largest `frames × width × height` an animation may have to open as one
/// (256 MiB of RGBA pixels).
pub const ANIMATION_MAX_PIXELS: u64 = 67_108_864;

/// The animated formats [`probe_animation`] recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatedFormat {
    /// GIF87a / GIF89a.
    Gif,
    /// PNG with an `acTL` chunk.
    Apng,
    /// WebP with `ANMF` chunks.
    WebP,
}

/// What [`probe_animation`] learned about an animated file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationProbe {
    /// The file's format.
    pub format: AnimatedFormat,
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// Frames the file announces: GIF image descriptors, the APNG `acTL`
    /// frame count, WebP `ANMF` chunks. Always at least 2.
    pub frame_count: u32,
}

/// Which limit keeps an animation from opening as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationLimit {
    /// The canvas is wider or taller than [`ANIMATION_MAX_SIDE`].
    Side,
    /// `frames × width × height` exceeds [`ANIMATION_MAX_PIXELS`].
    Pixels,
}

impl AnimationProbe {
    /// `frames × width × height`.
    pub fn total_pixels(&self) -> u64 {
        u64::from(self.frame_count) * u64::from(self.width) * u64::from(self.height)
    }

    /// The first limit the animation exceeds, if any.
    pub fn exceeded_limit(&self) -> Option<AnimationLimit> {
        if self.width > ANIMATION_MAX_SIDE || self.height > ANIMATION_MAX_SIDE {
            Some(AnimationLimit::Side)
        } else if self.total_pixels() > ANIMATION_MAX_PIXELS {
            Some(AnimationLimit::Pixels)
        } else {
            None
        }
    }
}

/// One frame of an imported animation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedFrame {
    /// Straight RGBA8 pixels of the whole canvas, row-major.
    pub rgba: Vec<u8>,
    /// Display duration in milliseconds (see [`frame_duration_ms`]).
    pub duration_ms: u16,
}

/// A decoded animation: every frame as the viewer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedAnimation {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
    /// The frames in file order.
    pub frames: Vec<ImportedFrame>,
}

/// Errors of the animated import.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnimationImportError {
    /// The file is not a GIF, PNG or WebP with more than one frame.
    #[error("not an animated GIF, PNG or WebP")]
    NotAnimated,
    /// The header or the first frame cannot be read, or the canvas is
    /// empty.
    #[error("cannot read the image: {0}")]
    Unreadable(String),
    /// A later frame cannot be decoded, or the file ends before every
    /// announced frame.
    #[error("the animation cannot be read completely: {0}")]
    Incomplete(String),
    /// The animation exceeds a limit of [`AnimationProbe::exceeded_limit`].
    #[error("the animation is too large to open as one")]
    TooLarge(AnimationLimit),
}

/// The animated format the bytes' signature names, if any.
fn sniff(bytes: &[u8]) -> Option<AnimatedFormat> {
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(AnimatedFormat::Gif)
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some(AnimatedFormat::Apng)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(AnimatedFormat::WebP)
    } else {
        None
    }
}

/// Canvas size and frame count of a file, before the "more than one
/// frame" check.
struct Header {
    width: u32,
    height: u32,
    frame_count: u32,
}

/// Whether `bytes` hold an animation: `Ok(Some(_))` for a GIF, PNG or
/// WebP with more than one frame whose first frame decodes, `Ok(None)` for
/// any other file (static images, other formats).
///
/// Only the first frame is decoded; the rest is counted from the frame
/// headers where the format allows (docs/specs/animated-image-import.md
/// AC-01, AC-02).
///
/// # Errors
///
/// [`AnimationImportError::Unreadable`] when the file announces more than
/// one frame but its header or first frame cannot be decoded, or its
/// canvas is 0 pixels wide or high (AC-13).
pub fn probe_animation(bytes: &[u8]) -> Result<Option<AnimationProbe>, AnimationImportError> {
    let Some(format) = sniff(bytes) else {
        return Ok(None);
    };
    let header = match format {
        AnimatedFormat::Gif => gif::probe(bytes)?,
        AnimatedFormat::Apng => apng::probe(bytes)?,
        AnimatedFormat::WebP => webp::probe(bytes)?,
    };
    let Some(header) = header else {
        return Ok(None);
    };
    if header.width == 0 || header.height == 0 {
        return Err(AnimationImportError::Unreadable(
            "the canvas is 0 pixels wide or high".to_owned(),
        ));
    }
    Ok(Some(AnimationProbe {
        format,
        width: header.width,
        height: header.height,
        frame_count: header.frame_count,
    }))
}

/// Decode every frame of an animated GIF, APNG or WebP onto its canvas
/// (AC-04) with its duration (AC-05).
///
/// # Errors
///
/// [`AnimationImportError::NotAnimated`] for anything
/// [`probe_animation`] does not report as animated,
/// [`AnimationImportError::Unreadable`] as for [`probe_animation`],
/// [`AnimationImportError::TooLarge`] above the limits (AC-08) and
/// [`AnimationImportError::Incomplete`] when a later frame cannot be
/// decoded or the file ends early (AC-09).
pub fn import_animation(bytes: &[u8]) -> Result<ImportedAnimation, AnimationImportError> {
    let probe = probe_animation(bytes)?.ok_or(AnimationImportError::NotAnimated)?;
    if let Some(limit) = probe.exceeded_limit() {
        return Err(AnimationImportError::TooLarge(limit));
    }
    let mut compositor = Compositor::new(probe.width, probe.height);
    match probe.format {
        AnimatedFormat::Gif => gif::decode(bytes, &mut compositor)?,
        AnimatedFormat::Apng => apng::decode(bytes, &mut compositor)?,
        AnimatedFormat::WebP => webp::decode(bytes, &mut compositor)?,
    }
    let frames = compositor.into_frames();
    if frames.len() != probe.frame_count as usize {
        return Err(AnimationImportError::Incomplete(format!(
            "{} of {} frames",
            frames.len(),
            probe.frame_count
        )));
    }
    Ok(ImportedAnimation {
        width: probe.width,
        height: probe.height,
        frames,
    })
}

/// How a format states a frame's delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Delay {
    /// GIF: hundredths of a second.
    GifCentis(u16),
    /// APNG: `num / den` seconds; a `den` of 0 means 100.
    Apng { num: u16, den: u16 },
    /// WebP: milliseconds.
    WebPMs(u32),
}

/// A frame delay as a Pixel-mode duration (AC-05): the delay in ms,
/// rounded half up; a GIF delay of 10 ms or less and any 0 ms delay become
/// 100 ms; above 65535 ms it is 65535 ms. Integer arithmetic only, so wasm
/// and native agree.
pub(super) fn frame_duration_ms(delay: Delay) -> u16 {
    let (num, den, gif) = match delay {
        Delay::GifCentis(c) => (u64::from(c), 100, true),
        Delay::Apng { num, den } => (
            u64::from(num),
            if den == 0 { 100 } else { u64::from(den) },
            false,
        ),
        Delay::WebPMs(ms) => (u64::from(ms), 1000, false),
    };
    // ms = num · 1000 / den, rounded half up.
    let ms = (num * 2000 + den) / (2 * den);
    if ms == 0 || (gif && ms <= 10) {
        100
    } else {
        u16::try_from(ms).unwrap_or(u16::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_duration_ms_gif_hundredths_times_ten() {
        assert_eq!(frame_duration_ms(Delay::GifCentis(7)), 70);
        assert_eq!(frame_duration_ms(Delay::GifCentis(2)), 20);
    }

    #[test]
    fn frame_duration_ms_gif_ten_or_less_becomes_100() {
        assert_eq!(frame_duration_ms(Delay::GifCentis(0)), 100);
        assert_eq!(frame_duration_ms(Delay::GifCentis(1)), 100);
    }

    #[test]
    fn frame_duration_ms_apng_rounds_half_up_and_keeps_short_delays() {
        assert_eq!(frame_duration_ms(Delay::Apng { num: 1, den: 3 }), 333);
        assert_eq!(frame_duration_ms(Delay::Apng { num: 1, den: 2000 }), 1);
        assert_eq!(frame_duration_ms(Delay::Apng { num: 3, den: 2000 }), 2);
        assert_eq!(frame_duration_ms(Delay::Apng { num: 5, den: 1000 }), 5);
    }

    #[test]
    fn frame_duration_ms_apng_zero_denominator_means_100() {
        assert_eq!(frame_duration_ms(Delay::Apng { num: 25, den: 0 }), 250);
    }

    #[test]
    fn frame_duration_ms_zero_becomes_100_in_every_format() {
        assert_eq!(frame_duration_ms(Delay::Apng { num: 0, den: 10 }), 100);
        assert_eq!(
            frame_duration_ms(Delay::Apng {
                num: 1,
                den: 10_000
            }),
            100
        );
        assert_eq!(frame_duration_ms(Delay::WebPMs(0)), 100);
    }

    #[test]
    fn frame_duration_ms_clamps_to_65535() {
        assert_eq!(frame_duration_ms(Delay::GifCentis(u16::MAX)), 65535);
        assert_eq!(frame_duration_ms(Delay::WebPMs(70_000)), 65535);
        assert_eq!(frame_duration_ms(Delay::Apng { num: 66, den: 1 }), 65535);
        assert_eq!(frame_duration_ms(Delay::WebPMs(5)), 5);
    }

    #[test]
    fn exceeded_limit_side_before_pixels() {
        let probe = |w, h, n| AnimationProbe {
            format: AnimatedFormat::Gif,
            width: w,
            height: h,
            frame_count: n,
        };
        assert_eq!(
            probe(5120, 300, 2).exceeded_limit(),
            Some(AnimationLimit::Side)
        );
        assert_eq!(
            probe(64, 4097, 2).exceeded_limit(),
            Some(AnimationLimit::Side)
        );
        assert_eq!(probe(4096, 4096, 4).exceeded_limit(), None);
        assert_eq!(
            probe(4096, 4096, 5).exceeded_limit(),
            Some(AnimationLimit::Pixels)
        );
        assert_eq!(probe(64, 64, 5000).exceeded_limit(), None);
    }

    #[test]
    fn probe_animation_other_formats_are_not_animated() {
        assert_eq!(probe_animation(b"not an image"), Ok(None));
        assert_eq!(probe_animation(&[]), Ok(None));
    }
}
