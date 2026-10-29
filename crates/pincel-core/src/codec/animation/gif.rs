//! GIF writer on top of `gif` 0.14 (spec §7.5).
//!
//! One local 8-bit palette per frame: exact when the frame's opaque
//! colours plus one transparent entry fit 256 entries, median-cut
//! otherwise. Alpha below 128 is transparent. Every frame covers the full
//! canvas and is disposed to background, so a pixel that turns
//! transparent in frame k is transparent when frame k shows.

use std::collections::HashMap;

use super::FrameSource;
use super::quantize::{median_cut, nearest};
use crate::codec::png::ExportError;
use crate::document::FrameIndex;

/// Alpha at or above which a pixel is opaque in a GIF.
const ALPHA_THRESHOLD: u8 = 128;

/// Entries of a GIF colour table.
const TABLE_SIZE: usize = 256;

/// One frame as palette indices.
struct IndexedFrame {
    /// `[r, g, b, …]`, at most 256 entries.
    palette: Vec<u8>,
    indices: Vec<u8>,
    transparent: Option<u8>,
}

/// Distinct opaque colours of an RGBA8 buffer with their pixel counts,
/// and whether any pixel is transparent under the GIF threshold.
fn opaque_colors(pixels: &[u8]) -> (HashMap<[u8; 3], u32>, bool) {
    let mut colors = HashMap::new();
    let mut transparent = false;
    for &[r, g, b, a] in pixels.as_chunks::<4>().0 {
        if a < ALPHA_THRESHOLD {
            transparent = true;
        } else {
            *colors.entry([r, g, b]).or_insert(0) += 1;
        }
    }
    (colors, transparent)
}

/// True when the frame's opaque colours, plus one entry when it has
/// transparent pixels, fit a 256-entry colour table.
pub(super) fn fits_table(pixels: &[u8]) -> bool {
    let (colors, transparent) = opaque_colors(pixels);
    colors.len() + usize::from(transparent) <= TABLE_SIZE
}

/// True when any pixel is partially transparent (alpha 1–254).
pub(super) fn has_partial_alpha(pixels: &[u8]) -> bool {
    pixels
        .as_chunks::<4>()
        .0
        .iter()
        .any(|px| px[3] != 0 && px[3] != u8::MAX)
}

/// Map an RGBA8 frame to palette indices: exact colours when they fit the
/// table, median-cut otherwise; the transparent entry, when needed, is the
/// last one.
fn index_frame(pixels: &[u8]) -> IndexedFrame {
    let (colors, has_transparent) = opaque_colors(pixels);
    let reserve = usize::from(has_transparent);
    let (entries, lookup): (Vec<[u8; 3]>, HashMap<[u8; 3], u8>) =
        if colors.len() + reserve <= TABLE_SIZE {
            let mut exact: Vec<[u8; 3]> = colors.keys().copied().collect();
            exact.sort_unstable();
            let lookup = exact
                .iter()
                .enumerate()
                .map(|(i, &c)| (c, u8::try_from(i).unwrap_or(u8::MAX)))
                .collect();
            (exact, lookup)
        } else {
            let palette = median_cut(&colors, TABLE_SIZE - reserve);
            let lookup = colors.keys().map(|&c| (c, nearest(&palette, c))).collect();
            (palette, lookup)
        };
    // With a transparent pixel the table holds at most 255 colours, so the
    // next index is free and fits `u8`.
    let transparent = has_transparent.then(|| u8::try_from(entries.len()).unwrap_or(u8::MAX));
    let indices = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&[r, g, b, a]| match transparent {
            Some(t) if a < ALPHA_THRESHOLD => t,
            _ => lookup.get(&[r, g, b]).copied().unwrap_or(0),
        })
        .collect();
    let mut palette: Vec<u8> = entries.into_iter().flatten().collect();
    if transparent.is_some() {
        palette.extend_from_slice(&[0, 0, 0]);
    }
    IndexedFrame {
        palette,
        indices,
        transparent,
    }
}

/// GIF delay in hundredths of a second: `duration_ms / 10` rounded to the
/// nearest integer, at least 2 (browsers clamp faster delays).
fn delay_cs(duration_ms: u16) -> u16 {
    let cs = (u32::from(duration_ms) + 5) / 10;
    // At most 6554 for `u16::MAX` ms.
    u16::try_from(cs.max(2)).unwrap_or(u16::MAX)
}

/// Write `frames` as a GIF of `width × height`; `loops` is the total play
/// count (`0` = forever, `1` = no NETSCAPE2.0 block, `N` = loop count
/// `N − 1`).
pub(super) fn write_gif(
    source: &mut FrameSource<'_>,
    frames: &[FrameIndex],
    width: u32,
    height: u32,
    loops: u16,
) -> Result<Vec<u8>, ExportError> {
    let enc = |e: ::gif::EncodingError| ExportError::GifEncode(e.to_string());
    let edge = |v: u32| {
        u16::try_from(v)
            .map_err(|_| ExportError::GifEncode(format!("{v} px exceeds the GIF size limit")))
    };
    let (w, h) = (edge(width)?, edge(height)?);
    let mut out = Vec::new();
    let mut encoder = ::gif::Encoder::new(&mut out, w, h, &[]).map_err(enc)?;
    let repeat = match loops {
        0 => ::gif::Repeat::Infinite,
        // `Finite(0)` writes no NETSCAPE2.0 block: one play.
        n => ::gif::Repeat::Finite(n - 1),
    };
    encoder.set_repeat(repeat).map_err(enc)?;
    for &frame in frames {
        let delay = delay_cs(source.duration_ms(frame));
        let indexed = index_frame(source.render(frame)?);
        let mut gif_frame = ::gif::Frame::from_palette_pixels(
            w,
            h,
            indexed.indices,
            indexed.palette,
            indexed.transparent,
        );
        gif_frame.delay = delay;
        gif_frame.dispose = ::gif::DisposalMethod::Background;
        encoder.write_frame(&gif_frame).map_err(enc)?;
    }
    // Writes the trailer.
    encoder.into_inner().map_err(enc)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{composed, sprite_from, three_frames, upscaled};
    use super::super::{AnimationFormat, AnimationOptions, export_animation};
    use super::*;
    use crate::document::{CelMap, Sprite, Tag, TagDirection};

    struct DecodedFrame {
        delay: u16,
        /// The displayed canvas after this frame, disposal of the previous
        /// frame applied.
        shown: Vec<u8>,
    }

    /// Decode a GIF and play it back: each frame is drawn over the canvas
    /// left by the previous frame's disposal (transparent pixels keep the
    /// canvas), as a browser does.
    fn play(bytes: &[u8]) -> (u16, u16, Vec<DecodedFrame>) {
        let mut options = ::gif::DecodeOptions::new();
        options.set_color_output(::gif::ColorOutput::RGBA);
        let mut decoder = options
            .read_info(std::io::Cursor::new(bytes))
            .expect("gif header parses");
        let (w, h) = (decoder.width(), decoder.height());
        let mut canvas = vec![0u8; usize::from(w) * usize::from(h) * 4];
        let mut out = Vec::new();
        while let Some(frame) = decoder.read_next_frame().expect("frame decodes") {
            let mut previous = canvas.clone();
            for y in 0..usize::from(frame.height) {
                for x in 0..usize::from(frame.width) {
                    let src = (y * usize::from(frame.width) + x) * 4;
                    if frame.buffer[src + 3] == 0 {
                        continue;
                    }
                    let cx = usize::from(frame.left) + x;
                    let cy = usize::from(frame.top) + y;
                    let dst = (cy * usize::from(w) + cx) * 4;
                    canvas[dst..dst + 4].copy_from_slice(&frame.buffer[src..src + 4]);
                }
            }
            out.push(DecodedFrame {
                delay: frame.delay,
                shown: canvas.clone(),
            });
            match frame.dispose {
                ::gif::DisposalMethod::Background => {
                    for y in 0..usize::from(frame.height) {
                        for x in 0..usize::from(frame.width) {
                            let cx = usize::from(frame.left) + x;
                            let cy = usize::from(frame.top) + y;
                            let dst = (cy * usize::from(w) + cx) * 4;
                            canvas[dst..dst + 4].fill(0);
                        }
                    }
                }
                ::gif::DisposalMethod::Previous => std::mem::swap(&mut canvas, &mut previous),
                _ => {}
            }
        }
        (w, h, out)
    }

    fn gif() -> AnimationOptions {
        AnimationOptions::new(AnimationFormat::Gif)
    }

    /// The NETSCAPE2.0 loop count in `bytes`, if the block is present.
    fn netscape_loops(bytes: &[u8]) -> Option<u16> {
        let at = bytes.windows(11).position(|w| w == b"NETSCAPE2.0")?;
        let block = &bytes[at + 11..at + 15];
        assert_eq!(&block[..2], &[3, 1], "loop sub-block");
        Some(u16::from_le_bytes([block[2], block[3]]))
    }

    /// GIF view of a composed RGBA8 buffer: alpha thresholded to 0 / 255,
    /// transparent pixels zeroed.
    fn thresholded(pixels: &[u8]) -> Vec<u8> {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[r, g, b, a]| {
                if a < 128 {
                    [0, 0, 0, 0]
                } else {
                    [r, g, b, 255]
                }
            })
            .collect()
    }

    /// One-frame `w × h` sprite whose first `opaque` pixels have distinct
    /// opaque colours and the rest are transparent.
    fn distinct_colors(w: u32, h: u32, opaque: u32) -> (Sprite, CelMap) {
        let mut px = vec![0u8; (w * h * 4) as usize];
        for i in 0..opaque {
            let [r, g, _, _] = (i * 7).to_le_bytes();
            let at = (i * 4) as usize;
            px[at..at + 4].copy_from_slice(&[r, g, 99, 255]);
        }
        sprite_from(w, h, &[(100, px)], Vec::new())
    }

    #[test]
    fn export_animation_gif_starts_with_gif89a() {
        let (sprite, cels) = three_frames(Vec::new());
        let bytes = export_animation(&sprite, &cels, &gif()).expect("export");
        assert_eq!(&bytes[..6], b"GIF89a");
        assert_eq!(play(&bytes).2.len(), 3);
    }

    #[test]
    fn export_animation_gif_loop_extension_counts_extra_plays() {
        let (sprite, cels) = three_frames(Vec::new());
        let forever = export_animation(&sprite, &cels, &gif()).expect("export");
        assert_eq!(netscape_loops(&forever), Some(0));
        let thrice = export_animation(&sprite, &cels, &gif().with_loops(3)).expect("export");
        assert_eq!(netscape_loops(&thrice), Some(2));
        let once = export_animation(&sprite, &cels, &gif().with_loops(1)).expect("export");
        assert_eq!(netscape_loops(&once), None);
    }

    #[test]
    fn export_animation_gif_alpha_threshold_is_128() {
        let px = vec![
            9, 9, 9, 127, //
            10, 20, 30, 128, //
            40, 50, 60, 0, //
            70, 80, 90, 255,
        ];
        let (sprite, cels) = sprite_from(2, 2, &[(100, px.clone())], Vec::new());
        let bytes = export_animation(&sprite, &cels, &gif()).expect("export");
        let (_, _, frames) = play(&bytes);
        assert_eq!(frames[0].shown, thresholded(&px));
        assert_eq!(&frames[0].shown[4..8], &[10, 20, 30, 255]);
        assert_eq!(frames[0].shown[3], 0);
    }

    #[test]
    fn export_animation_gif_fitting_frames_keep_exact_rgb() {
        // 255 distinct colours + transparent, and 256 distinct opaque.
        for (w, h, opaque) in [(16, 16, 255), (16, 16, 256)] {
            let (sprite, cels) = distinct_colors(w, h, opaque);
            let bytes = export_animation(&sprite, &cels, &gif()).expect("export");
            let (_, _, frames) = play(&bytes);
            assert_eq!(
                frames[0].shown,
                thresholded(&composed(&sprite, &cels, 0)),
                "{opaque} colours"
            );
        }
    }

    #[test]
    fn export_animation_gif_overflowing_frame_is_quantized_and_written() {
        // 256 distinct opaque colours plus transparent pixels: 257 entries.
        let (sprite, cels) = distinct_colors(17, 16, 256);
        let bytes = export_animation(&sprite, &cels, &gif()).expect("export");
        let (w, h, frames) = play(&bytes);
        assert_eq!((w, h), (17, 16));
        let shown = &frames[0].shown;
        let source = composed(&sprite, &cels, 0);
        // Transparency survives quantization.
        for (got, want) in shown
            .as_chunks::<4>()
            .0
            .iter()
            .zip(source.as_chunks::<4>().0)
        {
            assert_eq!(got[3] == 0, want[3] < 128);
        }
    }

    #[test]
    fn export_animation_gif_frames_follow_sequence_and_scale() {
        let mut tag = Tag::new(
            "pr",
            crate::document::FrameIndex::new(0),
            crate::document::FrameIndex::new(2),
        );
        tag.direction = TagDirection::PingpongReverse;
        let (sprite, cels) = three_frames(vec![tag]);
        let opts = gif().with_tag("pr").with_scale(2);
        let bytes = export_animation(&sprite, &cels, &opts).expect("export");
        let (w, h, frames) = play(&bytes);
        assert_eq!((w, h), (4, 4));
        assert_eq!(frames.len(), 4);
        for (slot, source) in [2, 1, 0, 1].into_iter().enumerate() {
            let want = thresholded(&upscaled(&composed(&sprite, &cels, source), 2, 2));
            assert_eq!(frames[slot].shown, want, "slot {slot}");
        }
    }

    #[test]
    fn delay_cs_rounds_to_hundredths_with_minimum_two() {
        assert_eq!(delay_cs(100), 10);
        assert_eq!(delay_cs(235), 24);
        assert_eq!(delay_cs(234), 23);
        assert_eq!(delay_cs(25), 3);
        assert_eq!(delay_cs(15), 2);
        assert_eq!(delay_cs(4), 2);
        assert_eq!(delay_cs(u16::MAX), 6554);
    }

    #[test]
    fn export_animation_gif_frame_delays_follow_durations() {
        let (sprite, cels) = three_frames(Vec::new());
        let bytes = export_animation(&sprite, &cels, &gif()).expect("export");
        let delays: Vec<u16> = play(&bytes).2.iter().map(|f| f.delay).collect();
        // 100, 50 and 230 ms.
        assert_eq!(delays, [10, 5, 23]);
    }

    #[test]
    fn export_animation_gif_pixel_turning_transparent_shows_transparent() {
        let opaque = [200u8, 100, 50, 255].repeat(4);
        let mut hole = opaque.clone();
        hole[0..4].fill(0);
        let (sprite, cels) = sprite_from(
            2,
            2,
            &[(100, opaque.clone()), (100, hole.clone())],
            Vec::new(),
        );
        let bytes = export_animation(&sprite, &cels, &gif()).expect("export");
        let (_, _, frames) = play(&bytes);
        assert_eq!(frames[0].shown, opaque);
        assert_eq!(frames[1].shown, hole);
        assert_eq!(frames[1].shown[3], 0);
    }

    #[test]
    fn fits_table_counts_the_transparent_entry() {
        let (sprite, cels) = distinct_colors(16, 16, 256);
        assert!(fits_table(&composed(&sprite, &cels, 0)));
        let (sprite, cels) = distinct_colors(17, 16, 256);
        assert!(!fits_table(&composed(&sprite, &cels, 0)));
    }

    #[test]
    fn has_partial_alpha_detects_alpha_between_1_and_254() {
        assert!(!has_partial_alpha(&[1, 2, 3, 0, 4, 5, 6, 255]));
        assert!(has_partial_alpha(&[1, 2, 3, 1]));
        assert!(has_partial_alpha(&[1, 2, 3, 254]));
    }
}
