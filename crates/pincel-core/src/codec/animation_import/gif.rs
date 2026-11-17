//! GIF frames through `gif` 0.14: every image descriptor is one frame,
//! decoded with its own colour table and transparent index and drawn
//! "over" the canvas.

use std::num::NonZeroU64;

use ::gif::{ColorOutput, DecodeOptions, DisposalMethod, MemoryLimit};

use super::canvas::{Blend, Compositor, Dispose, RawFrame};
use super::{AnimationImportError, Delay, Header, frame_duration_ms};

/// Largest decoded frame buffer the decoder may allocate (4 × the pixel
/// budget's bytes, so any frame of an animation within the limits fits).
const FRAME_MEMORY_LIMIT: u64 = 1 << 30;

fn options(skip: bool) -> DecodeOptions {
    let mut options = DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    if let Some(limit) = NonZeroU64::new(FRAME_MEMORY_LIMIT) {
        options.set_memory_limit(MemoryLimit::Bytes(limit));
    }
    options.skip_frame_decoding(skip);
    options
}

fn unreadable(e: impl std::fmt::Display) -> AnimationImportError {
    AnimationImportError::Unreadable(e.to_string())
}

fn incomplete(e: impl std::fmt::Display) -> AnimationImportError {
    AnimationImportError::Incomplete(e.to_string())
}

/// Screen size and image descriptor count; `None` with fewer than two
/// descriptors. The first frame is decoded; the others are only counted
/// (a file that breaks off after the second descriptor still counts it).
pub(super) fn probe(bytes: &[u8]) -> Result<Option<Header>, AnimationImportError> {
    let mut counter = options(true).read_info(bytes).map_err(unreadable)?;
    let (width, height) = (u32::from(counter.width()), u32::from(counter.height()));
    let mut frame_count: u32 = 0;
    loop {
        match counter.next_frame_info() {
            Ok(Some(_)) => frame_count = frame_count.saturating_add(1),
            Ok(None) => break,
            Err(e) if frame_count == 0 => return Err(unreadable(e)),
            Err(_) => break,
        }
    }
    if frame_count < 2 {
        return Ok(None);
    }
    let mut first = options(false).read_info(bytes).map_err(unreadable)?;
    match first.read_next_frame() {
        Ok(Some(_)) => {}
        Ok(None) => return Err(unreadable("no image data")),
        Err(e) => return Err(unreadable(e)),
    }
    Ok(Some(Header {
        width,
        height,
        frame_count,
    }))
}

/// Decode every frame onto `compositor`.
pub(super) fn decode(
    bytes: &[u8],
    compositor: &mut Compositor,
) -> Result<(), AnimationImportError> {
    let mut decoder = options(false).read_info(bytes).map_err(incomplete)?;
    while let Some(frame) = decoder.read_next_frame().map_err(incomplete)? {
        let dispose = match frame.dispose {
            DisposalMethod::Any | DisposalMethod::Keep => Dispose::None,
            DisposalMethod::Background => Dispose::Background,
            DisposalMethod::Previous => Dispose::Previous,
        };
        let expected = usize::from(frame.width) * usize::from(frame.height) * 4;
        if frame.buffer.len() != expected {
            return Err(incomplete("frame data is shorter than its rectangle"));
        }
        compositor.push(RawFrame {
            x: i64::from(frame.left),
            y: i64::from(frame.top),
            width: u32::from(frame.width),
            height: u32::from(frame.height),
            rgba: frame.buffer.to_vec(),
            blend: Blend::Over,
            dispose,
            duration_ms: frame_duration_ms(Delay::GifCentis(frame.delay)),
        });
    }
    Ok(())
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::{AnimatedFormat, import_animation, probe_animation};
    use super::*;
    use std::borrow::Cow;

    /// One GIF frame of a fixture: indices into `palette`.
    pub(crate) struct GifFrame {
        pub left: u16,
        pub top: u16,
        pub width: u16,
        pub height: u16,
        pub delay: u16,
        pub dispose: DisposalMethod,
        pub transparent: Option<u8>,
        /// Local colour table, `[r, g, b, …]`; `None` uses the global one.
        pub palette: Option<Vec<u8>>,
        pub indices: Vec<u8>,
    }

    impl GifFrame {
        pub fn full(w: u16, h: u16, indices: Vec<u8>) -> Self {
            Self {
                left: 0,
                top: 0,
                width: w,
                height: h,
                delay: 10,
                dispose: DisposalMethod::Keep,
                transparent: None,
                palette: None,
                indices,
            }
        }
    }

    /// Global palette: 0 red, 1 green, 2 blue, 3 white.
    pub(crate) const GLOBAL: [u8; 12] = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];

    pub(crate) fn gif_bytes(w: u16, h: u16, frames: &[GifFrame]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = ::gif::Encoder::new(&mut out, w, h, &GLOBAL).expect("encoder");
            for f in frames {
                let frame = ::gif::Frame {
                    left: f.left,
                    top: f.top,
                    width: f.width,
                    height: f.height,
                    delay: f.delay,
                    dispose: f.dispose,
                    transparent: f.transparent,
                    palette: f.palette.clone(),
                    buffer: Cow::Borrowed(&f.indices),
                    ..::gif::Frame::default()
                };
                enc.write_frame(&frame).expect("frame");
            }
        }
        out
    }

    const R: [u8; 4] = [255, 0, 0, 255];
    const G: [u8; 4] = [0, 255, 0, 255];
    const B: [u8; 4] = [0, 0, 255, 255];
    const W: [u8; 4] = [255, 255, 255, 255];
    const T: [u8; 4] = [0, 0, 0, 0];

    fn px(rgba: &[u8]) -> Vec<[u8; 4]> {
        rgba.as_chunks::<4>().0.to_vec()
    }

    #[test]
    fn probe_animation_gif_counts_image_descriptors() {
        let bytes = gif_bytes(
            3,
            2,
            &[
                GifFrame::full(3, 2, vec![0; 6]),
                GifFrame::full(3, 2, vec![1; 6]),
                GifFrame::full(3, 2, vec![2; 6]),
            ],
        );
        let probe = probe_animation(&bytes).expect("probe").expect("animated");
        assert_eq!(probe.format, AnimatedFormat::Gif);
        assert_eq!((probe.width, probe.height, probe.frame_count), (3, 2, 3));
    }

    #[test]
    fn probe_animation_single_frame_gif_is_static() {
        let bytes = gif_bytes(2, 2, &[GifFrame::full(2, 2, vec![0; 4])]);
        assert_eq!(probe_animation(&bytes), Ok(None));
    }

    #[test]
    fn import_animation_gif_offsets_transparency_and_local_tables() {
        let mut second = GifFrame::full(2, 1, vec![0, 1]);
        second.left = 1;
        second.top = 1;
        // Local table: 0 transparent (by index), 1 white.
        second.palette = Some(vec![9, 9, 9, 255, 255, 255]);
        second.transparent = Some(0);
        let bytes = gif_bytes(3, 2, &[GifFrame::full(3, 2, vec![2; 6]), second]);
        let anim = import_animation(&bytes).expect("import");
        assert_eq!((anim.width, anim.height), (3, 2));
        assert_eq!(px(&anim.frames[0].rgba), [B, B, B, B, B, B]);
        assert_eq!(px(&anim.frames[1].rgba), [B, B, B, B, B, W]);
    }

    #[test]
    fn import_animation_gif_frame_beyond_the_screen_is_cut_off() {
        let mut second = GifFrame::full(2, 2, vec![1; 4]);
        second.left = 1;
        second.top = 1;
        let bytes = gif_bytes(2, 2, &[GifFrame::full(2, 2, vec![0; 4]), second]);
        let anim = import_animation(&bytes).expect("import");
        assert_eq!(px(&anim.frames[1].rgba), [R, R, R, G]);
    }

    #[test]
    fn import_animation_gif_disposal_background_and_previous() {
        let mut first = GifFrame::full(2, 1, vec![0, 0]);
        first.dispose = DisposalMethod::Keep;
        let mut second = GifFrame::full(1, 1, vec![1]);
        second.dispose = DisposalMethod::Previous;
        let mut third = GifFrame::full(1, 1, vec![3]);
        third.left = 1;
        third.dispose = DisposalMethod::Background;
        let mut fourth = GifFrame::full(1, 1, vec![0]);
        fourth.transparent = Some(0);
        let bytes = gif_bytes(2, 1, &[first, second, third, fourth]);
        let anim = import_animation(&bytes).expect("import");
        let frames: Vec<_> = anim.frames.iter().map(|f| px(&f.rgba)).collect();
        assert_eq!(frames[0], [R, R]);
        assert_eq!(frames[1], [G, R]);
        // Previous restored frame 0's red before frame 2 drew.
        assert_eq!(frames[2], [R, W]);
        // Background cleared frame 2's rectangle to transparent.
        assert_eq!(frames[3], [R, T]);
    }

    #[test]
    fn import_animation_gif_durations_follow_the_delays() {
        let mut a = GifFrame::full(1, 1, vec![0]);
        a.delay = 7;
        let mut b = GifFrame::full(1, 1, vec![1]);
        b.delay = 1;
        let mut c = GifFrame::full(1, 1, vec![2]);
        c.delay = 0;
        let bytes = gif_bytes(1, 1, &[a, b, c]);
        let anim = import_animation(&bytes).expect("import");
        let durations: Vec<u16> = anim.frames.iter().map(|f| f.duration_ms).collect();
        assert_eq!(durations, [70, 100, 100]);
    }

    #[test]
    fn import_animation_gif_truncated_inside_a_frame_is_incomplete() {
        let bytes = gif_bytes(
            4,
            4,
            &[
                GifFrame::full(4, 4, vec![0; 16]),
                GifFrame::full(4, 4, vec![1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0]),
            ],
        );
        // Cut inside the second frame's image data.
        let cut = &bytes[..bytes.len() - 6];
        assert!(probe_animation(cut).expect("probe").is_some());
        assert!(matches!(
            import_animation(cut),
            Err(AnimationImportError::Incomplete(_))
        ));
    }

    #[test]
    fn probe_animation_gif_broken_first_frame_is_unreadable() {
        let mut bytes = gif_bytes(
            2,
            2,
            &[
                GifFrame::full(2, 2, vec![0; 4]),
                GifFrame::full(2, 2, vec![1; 4]),
            ],
        );
        // Header (13) + global table (12), then the first frame's graphic
        // control extension and its image descriptor (10 bytes), followed
        // by the LZW minimum code size, which may be at most 11.
        let separator = 25
            + bytes[25..]
                .iter()
                .position(|&b| b == 0x2c)
                .expect("image separator");
        bytes[separator + 10] = 12;
        assert!(matches!(
            probe_animation(&bytes),
            Err(AnimationImportError::Unreadable(_))
        ));
    }

    #[test]
    fn probe_animation_gif_zero_width_screen_is_unreadable() {
        let mut bytes = gif_bytes(
            1,
            1,
            &[GifFrame::full(1, 1, vec![0]), GifFrame::full(1, 1, vec![1])],
        );
        bytes[6] = 0;
        assert!(matches!(
            probe_animation(&bytes),
            Err(AnimationImportError::Unreadable(_))
        ));
    }
}
