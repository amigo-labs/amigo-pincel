//! Animated WebP: the RIFF container (`VP8X`, `ANMF`) is walked here; each
//! frame's bitstream (`VP8L`, or `VP8` with an optional `ALPH`) is wrapped
//! into a still WebP and decoded by `image-webp`, so offsets, blending and
//! disposal go through the shared canvas like GIF and APNG.

use std::io::Cursor;
use std::ops::Range;

use image_webp::WebPDecoder;

use super::canvas::{Blend, Compositor, Dispose, RawFrame};
use super::{AnimationImportError, Delay, Header, frame_duration_ms};

/// `VP8X` flag: the file is animated.
const FLAG_ANIMATION: u8 = 0b0000_0010;
/// `VP8X` flag: the image has alpha.
const FLAG_ALPHA: u8 = 0b0001_0000;

fn unreadable(e: impl std::fmt::Display) -> AnimationImportError {
    AnimationImportError::Unreadable(e.to_string())
}

fn incomplete(e: impl std::fmt::Display) -> AnimationImportError {
    AnimationImportError::Incomplete(e.to_string())
}

/// One RIFF chunk: its FourCC and where its payload lies. `data` may run
/// past the end of the file when the file is truncated.
struct Chunk {
    fourcc: [u8; 4],
    data: Range<usize>,
}

/// The chunks of `bytes[range]`, in order; a chunk whose header is cut off
/// ends the list.
fn chunks(bytes: &[u8], range: Range<usize>) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut pos = range.start;
    let end = range.end.min(bytes.len());
    while pos + 8 <= end {
        let mut fourcc = [0; 4];
        fourcc.copy_from_slice(&bytes[pos..pos + 4]);
        let mut size = [0; 4];
        size.copy_from_slice(&bytes[pos + 4..pos + 8]);
        let size = u32::from_le_bytes(size) as usize;
        let start = pos + 8;
        out.push(Chunk {
            fourcc,
            data: start..start.saturating_add(size),
        });
        pos = start.saturating_add(size).saturating_add(size & 1);
    }
    out
}

fn u24(b: &[u8]) -> u32 {
    u32::from(b[0]) | (u32::from(b[1]) << 8) | (u32::from(b[2]) << 16)
}

/// The canvas size and the `ANMF` chunks of an animated WebP; `None` when
/// the file has no animated `VP8X` header.
fn layout(bytes: &[u8]) -> Option<(u32, u32, Vec<Range<usize>>)> {
    let riff_end =
        8usize.saturating_add(u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize);
    let top = chunks(bytes, 12..riff_end);
    let vp8x = top.first().filter(|c| &c.fourcc == b"VP8X")?;
    let header = bytes.get(vp8x.data.start..vp8x.data.start + 10)?;
    if header[0] & FLAG_ANIMATION == 0 {
        return None;
    }
    let width = u24(&header[4..7]) + 1;
    let height = u24(&header[7..10]) + 1;
    let frames = top
        .iter()
        .filter(|c| &c.fourcc == b"ANMF")
        .map(|c| c.data.clone())
        .collect();
    Some((width, height, frames))
}

/// One `ANMF` chunk decoded: its rectangle and flags plus straight RGBA8.
fn decode_frame(bytes: &[u8], anmf: &Range<usize>) -> Result<RawFrame, String> {
    let data = bytes
        .get(anmf.clone())
        .ok_or("the file ends inside a frame")?;
    if data.len() < 16 {
        return Err("frame header is too short".to_owned());
    }
    let x = i64::from(u24(&data[0..3])) * 2;
    let y = i64::from(u24(&data[3..6])) * 2;
    let width = u24(&data[6..9]) + 1;
    let height = u24(&data[9..12]) + 1;
    let duration = u24(&data[12..15]);
    let flags = data[15];
    let frame_data = &data[16..];
    let sub = chunks(frame_data, 0..frame_data.len());
    let lossless = sub.iter().any(|c| &c.fourcc == b"VP8L");
    let alpha = lossless || sub.iter().any(|c| &c.fourcc == b"ALPH");
    let rgba = decode_still(frame_data, width, height, alpha)?;
    Ok(RawFrame {
        x,
        y,
        width,
        height,
        rgba,
        blend: if flags & 0b10 != 0 {
            Blend::Source
        } else {
            Blend::Over
        },
        dispose: if flags & 0b01 != 0 {
            Dispose::Background
        } else {
            Dispose::None
        },
        duration_ms: frame_duration_ms(Delay::WebPMs(duration)),
    })
}

/// Decode a frame's chunks as a still `width × height` extended WebP.
fn decode_still(
    frame_data: &[u8],
    width: u32,
    height: u32,
    alpha: bool,
) -> Result<Vec<u8>, String> {
    let mut vp8x = [0u8; 10];
    vp8x[0] = if alpha { FLAG_ALPHA } else { 0 };
    vp8x[4..7].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x[7..10].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
    let riff_size = u32::try_from(4 + 18 + frame_data.len()).map_err(|e| e.to_string())?;
    let mut still = Vec::with_capacity(8 + riff_size as usize);
    still.extend_from_slice(b"RIFF");
    still.extend_from_slice(&riff_size.to_le_bytes());
    still.extend_from_slice(b"WEBPVP8X");
    still.extend_from_slice(&10u32.to_le_bytes());
    still.extend_from_slice(&vp8x);
    still.extend_from_slice(frame_data);
    let mut decoder = WebPDecoder::new(Cursor::new(still)).map_err(|e| e.to_string())?;
    if decoder.dimensions() != (width, height) {
        return Err("frame size does not match its ANMF header".to_owned());
    }
    let size = decoder
        .output_buffer_size()
        .ok_or("frame too large to buffer")?;
    let mut buf = vec![0; size];
    decoder.read_image(&mut buf).map_err(|e| e.to_string())?;
    if decoder.has_alpha() {
        Ok(buf)
    } else {
        Ok(buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect())
    }
}

/// Canvas size and `ANMF` count; `None` for a still WebP or a single
/// frame. The first frame is decoded.
pub(super) fn probe(bytes: &[u8]) -> Result<Option<Header>, AnimationImportError> {
    let Some((width, height, frames)) = layout(bytes) else {
        return Ok(None);
    };
    if frames.len() < 2 {
        return Ok(None);
    }
    decode_frame(bytes, &frames[0]).map_err(unreadable)?;
    Ok(Some(Header {
        width,
        height,
        frame_count: u32::try_from(frames.len()).unwrap_or(u32::MAX),
    }))
}

/// Decode every frame onto `compositor`.
pub(super) fn decode(
    bytes: &[u8],
    compositor: &mut Compositor,
) -> Result<(), AnimationImportError> {
    let (_, _, frames) = layout(bytes).ok_or_else(|| incomplete("not an animated WebP"))?;
    for anmf in &frames {
        compositor.push(decode_frame(bytes, anmf).map_err(incomplete)?);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::{AnimatedFormat, import_animation, probe_animation};
    use super::*;

    /// One frame of a fixture: a lossless bitstream of `rgba`.
    pub(crate) struct WebPFrame {
        pub x: u32,
        pub y: u32,
        pub w: u32,
        pub h: u32,
        pub duration: u32,
        pub blend: bool,
        pub dispose: bool,
        pub rgba: Vec<u8>,
    }

    pub(crate) fn full(w: u32, h: u32, px: &[[u8; 4]]) -> WebPFrame {
        WebPFrame {
            x: 0,
            y: 0,
            w,
            h,
            duration: 100,
            blend: false,
            dispose: false,
            rgba: px.concat(),
        }
    }

    fn chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(fourcc);
        out.extend_from_slice(&u32::try_from(data.len()).expect("small").to_le_bytes());
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(0);
        }
    }

    fn u24le(v: u32) -> [u8; 3] {
        let b = v.to_le_bytes();
        [b[0], b[1], b[2]]
    }

    /// The `VP8L` chunk (header included) of a lossless still WebP.
    fn vp8l(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut still = Vec::new();
        image_webp::WebPEncoder::new(&mut still)
            .encode(rgba, w, h, image_webp::ColorType::Rgba8)
            .expect("encode");
        assert_eq!(&still[12..16], b"VP8L");
        still[12..].to_vec()
    }

    /// An animated WebP of `w × h` with `bg` as the `ANIM` background.
    pub(crate) fn webp(w: u32, h: u32, bg: [u8; 4], frames: &[WebPFrame]) -> Vec<u8> {
        let mut body = b"WEBP".to_vec();
        let mut vp8x = vec![FLAG_ANIMATION | FLAG_ALPHA, 0, 0, 0];
        vp8x.extend_from_slice(&u24le(w - 1));
        vp8x.extend_from_slice(&u24le(h - 1));
        chunk(&mut body, b"VP8X", &vp8x);
        let mut anim = bg.to_vec();
        anim.extend_from_slice(&0u16.to_le_bytes());
        chunk(&mut body, b"ANIM", &anim);
        for f in frames {
            let mut anmf = Vec::new();
            anmf.extend_from_slice(&u24le(f.x / 2));
            anmf.extend_from_slice(&u24le(f.y / 2));
            anmf.extend_from_slice(&u24le(f.w - 1));
            anmf.extend_from_slice(&u24le(f.h - 1));
            anmf.extend_from_slice(&u24le(f.duration));
            anmf.push(u8::from(!f.blend) << 1 | u8::from(f.dispose));
            anmf.extend_from_slice(&vp8l(f.w, f.h, &f.rgba));
            chunk(&mut body, b"ANMF", &anmf);
        }
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&u32::try_from(body.len()).expect("small").to_le_bytes());
        out.extend_from_slice(&body);
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
    fn probe_animation_webp_counts_anmf_chunks() {
        let bytes = webp(
            4,
            2,
            T,
            &[
                full(4, 2, &[R; 8]),
                full(4, 2, &[G; 8]),
                full(4, 2, &[B; 8]),
            ],
        );
        let probe = probe_animation(&bytes).expect("probe").expect("animated");
        assert_eq!(probe.format, AnimatedFormat::WebP);
        assert_eq!((probe.width, probe.height, probe.frame_count), (4, 2, 3));
    }

    #[test]
    fn probe_animation_one_frame_webp_is_static() {
        let bytes = webp(1, 1, T, &[full(1, 1, &[R])]);
        assert_eq!(probe_animation(&bytes), Ok(None));
        let mut still = Vec::new();
        image_webp::WebPEncoder::new(&mut still)
            .encode(&R, 1, 1, image_webp::ColorType::Rgba8)
            .expect("encode");
        assert_eq!(probe_animation(&still), Ok(None));
    }

    #[test]
    fn import_animation_webp_offsets_blend_and_disposal() {
        let first = full(4, 2, &[R; 8]);
        // Over at (2, 0): half blue over red; disposed to background.
        let mut second = full(2, 2, &[HALF_B, HALF_B, HALF_B, HALF_B]);
        second.x = 2;
        second.blend = true;
        second.dispose = true;
        // No blending: the transparent pixel replaces red.
        let mut third = full(2, 1, &[T, G]);
        third.blend = false;
        // A red background colour: disposal still clears to transparent.
        let bytes = webp(4, 2, [255, 0, 0, 255], &[first, second, third]);
        let anim = import_animation(&bytes).expect("import");
        let mixed = [127, 0, 128, 255];
        let frames: Vec<_> = anim.frames.iter().map(|f| px(&f.rgba)).collect();
        assert_eq!(frames[0], [R; 8]);
        assert_eq!(frames[1], [R, R, mixed, mixed, R, R, mixed, mixed]);
        assert_eq!(frames[2], [T, G, T, T, R, R, T, T]);
    }

    #[test]
    fn import_animation_webp_durations_in_ms() {
        let mut a = full(1, 1, &[R]);
        a.duration = 40;
        let mut b = full(1, 1, &[G]);
        b.duration = 0;
        let mut c = full(1, 1, &[B]);
        c.duration = 5;
        let bytes = webp(1, 1, T, &[a, b, c]);
        let anim = import_animation(&bytes).expect("import");
        let durations: Vec<u16> = anim.frames.iter().map(|f| f.duration_ms).collect();
        assert_eq!(durations, [40, 100, 5]);
    }

    #[test]
    fn import_animation_webp_broken_later_frame_is_incomplete() {
        let mut bytes = webp(2, 1, T, &[full(2, 1, &[R, R]), full(2, 1, &[G, B])]);
        // Corrupt the second VP8L bitstream's signature byte.
        let second_vp8l = bytes
            .windows(4)
            .rposition(|w| w == b"VP8L")
            .expect("second frame");
        bytes[second_vp8l + 8] = 0;
        assert!(probe_animation(&bytes).expect("probe").is_some());
        assert!(matches!(
            import_animation(&bytes),
            Err(AnimationImportError::Incomplete(_))
        ));
        // Cut off inside the second frame.
        let good = webp(2, 1, T, &[full(2, 1, &[R, R]), full(2, 1, &[G, B])]);
        let cut = &good[..good.len() - 4];
        assert!(matches!(
            import_animation(cut),
            Err(AnimationImportError::Incomplete(_))
        ));
    }

    #[test]
    fn probe_animation_webp_broken_first_frame_is_unreadable() {
        let mut bytes = webp(2, 1, T, &[full(2, 1, &[R, R]), full(2, 1, &[G, B])]);
        let first_vp8l = bytes
            .windows(4)
            .position(|w| w == b"VP8L")
            .expect("first frame");
        bytes[first_vp8l + 8] = 0;
        assert!(matches!(
            probe_animation(&bytes),
            Err(AnimationImportError::Unreadable(_))
        ));
    }
}
