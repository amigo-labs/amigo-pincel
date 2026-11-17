//! APNG frames through `png` 0.18: the `acTL` frame count, each frame's
//! `fcTL` rectangle, blend and dispose op. A default image outside the
//! animation (`IDAT` without `fcTL`) is no frame.

use std::io::Cursor;

use ::png::{BlendOp, ColorType, Decoder, DisposeOp, Reader, Transformations};

use super::canvas::{Blend, Compositor, Dispose, RawFrame};
use super::{AnimationImportError, Delay, Header, frame_duration_ms};

fn unreadable(e: impl std::fmt::Display) -> AnimationImportError {
    AnimationImportError::Unreadable(e.to_string())
}

fn incomplete(e: impl std::fmt::Display) -> AnimationImportError {
    AnimationImportError::Incomplete(e.to_string())
}

fn reader(bytes: &[u8]) -> Result<Reader<Cursor<&[u8]>>, AnimationImportError> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::normalize_to_color8());
    decoder.read_info().map_err(unreadable)
}

fn frame_buffer(reader: &Reader<Cursor<&[u8]>>) -> Result<Vec<u8>, AnimationImportError> {
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| unreadable("image too large to buffer"))?;
    Ok(vec![0; size])
}

/// Image size and `acTL` frame count; `None` without `acTL` or with one
/// frame. The default image (what a first-frame open shows) is decoded.
pub(super) fn probe(bytes: &[u8]) -> Result<Option<Header>, AnimationImportError> {
    let mut reader = reader(bytes)?;
    let info = reader.info();
    let (width, height) = (info.width, info.height);
    let Some(actl) = info.animation_control else {
        return Ok(None);
    };
    if actl.num_frames < 2 {
        return Ok(None);
    }
    let mut buf = frame_buffer(&reader)?;
    reader.next_frame(&mut buf).map_err(unreadable)?;
    Ok(Some(Header {
        width,
        height,
        frame_count: actl.num_frames,
    }))
}

/// Straight RGBA8 from the 8-bit layouts `normalize_to_color8` leaves.
fn to_rgba(buf: &[u8], color: ColorType) -> Result<Vec<u8>, AnimationImportError> {
    Ok(match color {
        ColorType::Rgba => buf.to_vec(),
        ColorType::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        ColorType::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        ColorType::Indexed => return Err(incomplete("unexpanded palette frame")),
    })
}

/// Decode every animation frame onto `compositor`.
pub(super) fn decode(
    bytes: &[u8],
    compositor: &mut Compositor,
) -> Result<(), AnimationImportError> {
    let mut reader = reader(bytes)?;
    let num_frames = reader
        .info()
        .animation_control
        .map_or(0, |actl| actl.num_frames);
    let mut buf = frame_buffer(&reader)?;
    if reader.info().frame_control.is_none() {
        // The default image is not part of the animation.
        reader.next_frame(&mut buf).map_err(incomplete)?;
    }
    for _ in 0..num_frames {
        let out = reader.next_frame(&mut buf).map_err(incomplete)?;
        let fctl = reader
            .info()
            .frame_control
            .ok_or_else(|| incomplete("frame without fcTL"))?;
        let rgba = to_rgba(&buf[..out.buffer_size()], out.color_type)?;
        if rgba.len() != fctl.width as usize * fctl.height as usize * 4 {
            return Err(incomplete("frame data does not match its fcTL"));
        }
        compositor.push(RawFrame {
            x: i64::from(fctl.x_offset),
            y: i64::from(fctl.y_offset),
            width: fctl.width,
            height: fctl.height,
            rgba,
            blend: match fctl.blend_op {
                BlendOp::Source => Blend::Source,
                BlendOp::Over => Blend::Over,
            },
            dispose: match fctl.dispose_op {
                DisposeOp::None => Dispose::None,
                DisposeOp::Background => Dispose::Background,
                DisposeOp::Previous => Dispose::Previous,
            },
            duration_ms: frame_duration_ms(Delay::Apng {
                num: fctl.delay_num,
                den: fctl.delay_den,
            }),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{AnimatedFormat, import_animation, probe_animation};
    use super::*;

    /// One APNG frame of a fixture.
    struct ApngFrame {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        delay: (u16, u16),
        dispose: DisposeOp,
        blend: BlendOp,
        rgba: Vec<u8>,
    }

    fn full(w: u32, h: u32, px: &[[u8; 4]]) -> ApngFrame {
        ApngFrame {
            x: 0,
            y: 0,
            w,
            h,
            delay: (1, 10),
            dispose: DisposeOp::None,
            blend: BlendOp::Source,
            rgba: px.concat(),
        }
    }

    /// An RGBA APNG; `default` is a separate default image outside the
    /// animation.
    fn apng(w: u32, h: u32, default: Option<&[[u8; 4]]>, frames: &[ApngFrame]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = ::png::Encoder::new(&mut out, w, h);
        enc.set_color(ColorType::Rgba);
        enc.set_depth(::png::BitDepth::Eight);
        enc.set_animated(u32::try_from(frames.len()).expect("few"), 0)
            .expect("animated");
        if default.is_some() {
            enc.set_sep_def_img(true).expect("sep");
        }
        let mut writer = enc.write_header().expect("header");
        if let Some(px) = default {
            writer
                .write_image_data(&px.concat())
                .expect("default image");
        }
        for f in frames {
            writer.reset_frame_position().expect("pos");
            writer.set_frame_dimension(f.w, f.h).expect("dim");
            writer.set_frame_position(f.x, f.y).expect("pos");
            writer.set_frame_delay(f.delay.0, f.delay.1).expect("delay");
            writer.set_dispose_op(f.dispose).expect("dispose");
            writer.set_blend_op(f.blend).expect("blend");
            writer.write_image_data(&f.rgba).expect("frame");
        }
        writer.finish().expect("finish");
        out
    }

    const R: [u8; 4] = [255, 0, 0, 255];
    const G: [u8; 4] = [0, 255, 0, 255];
    const B: [u8; 4] = [0, 0, 255, 255];
    const T: [u8; 4] = [0, 0, 0, 0];
    const HALF_B: [u8; 4] = [0, 0, 255, 128];

    fn px(rgba: &[u8]) -> Vec<[u8; 4]> {
        rgba.as_chunks::<4>().0.to_vec()
    }

    #[test]
    fn probe_animation_apng_reads_the_actl_frame_count() {
        let bytes = apng(
            2,
            1,
            None,
            &[
                full(2, 1, &[R, R]),
                full(2, 1, &[G, G]),
                full(2, 1, &[B, B]),
            ],
        );
        let probe = probe_animation(&bytes).expect("probe").expect("animated");
        assert_eq!(probe.format, AnimatedFormat::Apng);
        assert_eq!((probe.width, probe.height, probe.frame_count), (2, 1, 3));
    }

    #[test]
    fn probe_animation_one_frame_apng_and_plain_png_are_static() {
        let one = apng(1, 1, None, &[full(1, 1, &[R])]);
        assert_eq!(probe_animation(&one), Ok(None));
        let plain = crate::codec::encode_png(&R, 1, 1).expect("png");
        assert_eq!(probe_animation(&plain), Ok(None));
    }

    #[test]
    fn import_animation_apng_separate_default_image_is_no_frame() {
        let bytes = apng(1, 1, Some(&[B]), &[full(1, 1, &[R]), full(1, 1, &[G])]);
        let anim = import_animation(&bytes).expect("import");
        assert_eq!(anim.frames.len(), 2);
        assert_eq!(px(&anim.frames[0].rgba), [R]);
        assert_eq!(px(&anim.frames[1].rgba), [G]);
    }

    #[test]
    fn import_animation_apng_blend_and_dispose_ops() {
        let mut first = full(2, 1, &[R, R]);
        first.dispose = DisposeOp::None;
        // Over at x = 1: half blue over red.
        let mut second = full(1, 1, &[HALF_B]);
        second.x = 1;
        second.blend = BlendOp::Over;
        second.dispose = DisposeOp::Previous;
        // Source at x = 0 with a transparent pixel, then cleared.
        let mut third = full(1, 1, &[T]);
        third.dispose = DisposeOp::Background;
        let mut fourth = full(1, 1, &[G]);
        fourth.x = 1;
        fourth.blend = BlendOp::Over;
        let bytes = apng(2, 1, None, &[first, second, third, fourth]);
        let anim = import_animation(&bytes).expect("import");
        let frames: Vec<_> = anim.frames.iter().map(|f| px(&f.rgba)).collect();
        assert_eq!(frames[0], [R, R]);
        assert_eq!(frames[1], [R, [127, 0, 128, 255]]);
        // Previous restored the red under frame 1.
        assert_eq!(frames[2], [T, R]);
        // Background cleared frame 2's pixel (already transparent).
        assert_eq!(frames[3], [T, G]);
    }

    #[test]
    fn import_animation_apng_durations_from_num_and_den() {
        let mut a = full(1, 1, &[R]);
        a.delay = (1, 3);
        let mut b = full(1, 1, &[G]);
        b.delay = (25, 0);
        let mut c = full(1, 1, &[B]);
        c.delay = (0, 1);
        let mut d = full(1, 1, &[R]);
        d.delay = (5, 1000);
        let bytes = apng(1, 1, None, &[a, b, c, d]);
        let anim = import_animation(&bytes).expect("import");
        let durations: Vec<u16> = anim.frames.iter().map(|f| f.duration_ms).collect();
        assert_eq!(durations, [333, 250, 100, 5]);
    }

    #[test]
    fn import_animation_apng_ending_before_the_frame_count_is_incomplete() {
        let bytes = apng(
            1,
            1,
            None,
            &[full(1, 1, &[R]), full(1, 1, &[G]), full(1, 1, &[B])],
        );
        // Announce one frame more than the file holds.
        let mut more = bytes.clone();
        let actl = more
            .windows(4)
            .position(|w| w == b"acTL")
            .expect("acTL chunk");
        more[actl + 7] = 4;
        // Recompute the chunk CRC so the header still parses.
        let crc = crc32(&more[actl..actl + 12]);
        more[actl + 12..actl + 16].copy_from_slice(&crc.to_be_bytes());
        assert_eq!(
            probe_animation(&more)
                .expect("probe")
                .map(|p| p.frame_count),
            Some(4)
        );
        assert!(matches!(
            import_animation(&more),
            Err(AnimationImportError::Incomplete(_))
        ));
    }

    #[test]
    fn probe_animation_apng_broken_default_image_is_unreadable() {
        let mut bytes = apng(1, 1, None, &[full(1, 1, &[R]), full(1, 1, &[G])]);
        let idat = bytes
            .windows(4)
            .position(|w| w == b"IDAT")
            .expect("IDAT chunk");
        // Corrupt the zlib stream (and its CRC check fails too).
        bytes[idat + 4] ^= 0xff;
        bytes[idat + 5] ^= 0xff;
        assert!(matches!(
            probe_animation(&bytes),
            Err(AnimationImportError::Unreadable(_))
        ));
    }

    /// CRC-32 (ISO-HDLC) as PNG chunks use it.
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
}
