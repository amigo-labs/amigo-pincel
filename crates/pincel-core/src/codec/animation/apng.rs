//! APNG writer on top of `png` 0.18 (spec §7.5).
//!
//! Every frame covers the full canvas with `dispose_op = None` and
//! `blend_op = Source`, so a decoded frame equals its composed frame
//! independent of the frames before it. The first frame is the default
//! image (`IDAT`) and part of the animation.

use super::FrameSource;
use crate::codec::png::ExportError;
use crate::document::FrameIndex;

/// Write `frames` as an RGBA8 APNG of `width × height`; `loops` is the
/// play count (`0` = forever).
pub(super) fn write_apng(
    source: &mut FrameSource<'_>,
    frames: &[FrameIndex],
    width: u32,
    height: u32,
    loops: u16,
) -> Result<Vec<u8>, ExportError> {
    let enc = |e: ::png::EncodingError| ExportError::Encode(e.to_string());
    let num_frames = u32::try_from(frames.len()).map_err(|_| {
        ExportError::Encode(format!("{} frames exceed the APNG limit", frames.len()))
    })?;
    let mut out = Vec::new();
    let mut encoder = ::png::Encoder::new(&mut out, width, height);
    encoder.set_color(::png::ColorType::Rgba);
    encoder.set_depth(::png::BitDepth::Eight);
    encoder
        .set_animated(num_frames, u32::from(loops))
        .map_err(enc)?;
    encoder
        .set_dispose_op(::png::DisposeOp::None)
        .map_err(enc)?;
    encoder.set_blend_op(::png::BlendOp::Source).map_err(enc)?;
    let mut writer = encoder.write_header().map_err(enc)?;
    for &frame in frames {
        // The fcTL is written with the frame data, so the delay set here
        // applies to this frame.
        writer
            .set_frame_delay(source.duration_ms(frame), 1000)
            .map_err(enc)?;
        writer
            .write_image_data(source.render(frame)?)
            .map_err(enc)?;
    }
    writer.finish().map_err(enc)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{composed, three_frames, upscaled};
    use super::super::{AnimationFormat, AnimationOptions, export_animation};
    use crate::document::{FrameIndex, Tag, TagDirection};

    struct DecodedApng {
        width: u32,
        height: u32,
        num_frames: u32,
        num_plays: u32,
        /// `(delay_num, delay_den, pixels)` per frame.
        frames: Vec<(u16, u16, Vec<u8>)>,
    }

    fn decode(bytes: &[u8]) -> DecodedApng {
        let decoder = ::png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("png header parses");
        let info = reader.info();
        let actl = info.animation_control.expect("acTL present");
        let (width, height) = (info.width, info.height);
        let mut frames = Vec::new();
        for _ in 0..actl.num_frames {
            let size = reader.output_buffer_size().expect("known size");
            let mut buf = vec![0u8; size];
            let out = reader.next_frame(&mut buf).expect("frame decodes");
            buf.truncate(out.buffer_size());
            let fctl = reader.info().frame_control.expect("fcTL present");
            assert_eq!(fctl.dispose_op, ::png::DisposeOp::None);
            assert_eq!(fctl.blend_op, ::png::BlendOp::Source);
            assert_eq!((fctl.x_offset, fctl.y_offset), (0, 0));
            assert_eq!((fctl.width, fctl.height), (width, height));
            frames.push((fctl.delay_num, fctl.delay_den, buf));
        }
        DecodedApng {
            width,
            height,
            num_frames: actl.num_frames,
            num_plays: actl.num_plays,
            frames,
        }
    }

    fn apng() -> AnimationOptions {
        AnimationOptions::new(AnimationFormat::Apng)
    }

    #[test]
    fn export_animation_apng_has_png_signature_and_actl_frame_count() {
        let (sprite, cels) = three_frames(Vec::new());
        let bytes = export_animation(&sprite, &cels, &apng()).expect("export");
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        assert_eq!(decode(&bytes).num_frames, 3);
    }

    #[test]
    fn export_animation_apng_frames_equal_composed_frames_in_sequence_order() {
        let mut tag = Tag::new("pp", FrameIndex::new(0), FrameIndex::new(2));
        tag.direction = TagDirection::Pingpong;
        let (sprite, cels) = three_frames(vec![tag]);
        let bytes = export_animation(&sprite, &cels, &apng().with_tag("pp")).expect("export");
        let decoded = decode(&bytes);
        assert_eq!(decoded.num_frames, 4);
        for (slot, source) in [0, 1, 2, 1].into_iter().enumerate() {
            assert_eq!(
                decoded.frames[slot].2,
                composed(&sprite, &cels, source),
                "slot {slot} shows frame {source}"
            );
        }
    }

    #[test]
    fn export_animation_apng_scale_makes_exact_blocks() {
        let (sprite, cels) = three_frames(Vec::new());
        let bytes = export_animation(&sprite, &cels, &apng().with_scale(3)).expect("export");
        let decoded = decode(&bytes);
        assert_eq!((decoded.width, decoded.height), (6, 6));
        for (i, frame) in decoded.frames.iter().enumerate() {
            let source = composed(&sprite, &cels, i as u32);
            assert_eq!(frame.2, upscaled(&source, 2, 3), "frame {i}");
        }
    }

    #[test]
    fn export_animation_apng_delay_is_duration_in_ms() {
        let (sprite, cels) = three_frames(Vec::new());
        let bytes = export_animation(&sprite, &cels, &apng()).expect("export");
        let delays: Vec<(u16, u16)> = decode(&bytes).frames.iter().map(|f| (f.0, f.1)).collect();
        assert_eq!(delays, [(100, 1000), (50, 1000), (230, 1000)]);
    }

    #[test]
    fn export_animation_apng_play_count_follows_loops() {
        let (sprite, cels) = three_frames(Vec::new());
        let forever = export_animation(&sprite, &cels, &apng()).expect("export");
        assert_eq!(decode(&forever).num_plays, 0);
        let thrice = export_animation(&sprite, &cels, &apng().with_loops(3)).expect("export");
        assert_eq!(decode(&thrice).num_plays, 3);
    }

    #[test]
    fn export_animation_apng_single_frame_is_valid_animation() {
        let (sprite, cels) = three_frames(vec![Tag::new(
            "one",
            FrameIndex::new(1),
            FrameIndex::new(1),
        )]);
        let bytes = export_animation(&sprite, &cels, &apng().with_tag("one")).expect("export");
        let decoded = decode(&bytes);
        assert_eq!(decoded.num_frames, 1);
        assert_eq!(decoded.frames[0].2, composed(&sprite, &cels, 1));
    }
}
