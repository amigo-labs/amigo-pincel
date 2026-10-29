//! Onion-skin pass for [`super::compose`]. See `docs/specs/pincel.md` §4.2
//! and the Decision Log entry of 2026-10-11 for the tint math.
//!
//! Each neighbouring frame is composited on its own (same layer filter,
//! same viewport) into a scratch buffer, tinted, and blended source-over
//! into the destination *before* the current frame, so the onion sits
//! under the live pixels:
//!
//! - previous frames are tinted red, next frames blue — every colour
//!   channel moves halfway toward the tint (`(c + tint) / 2`), which keeps
//!   the drawing's shading readable while making the direction obvious;
//! - the neighbour at distance `k` of `n` requested frames is blended at
//!   `alpha · (n − k + 1) / n` (the nearest at full `previous_alpha` /
//!   `next_alpha`, farther ones fading linearly);
//! - farther frames are blended first so nearer ones land on top.
//!
//! Frames outside the sprite are skipped; there is no wrap-around.

use crate::document::{BlendMode, CelMap, FrameIndex, Sprite};
use crate::geometry::Rect;

use super::blend::{blend_pixel_into, mul_u8};
use super::compose::composite_visible_layers;
use super::error::RenderError;
use super::request::{ComposeRequest, OnionSkin};

const PREVIOUS_TINT: [u8; 3] = [255, 0, 0];
const NEXT_TINT: [u8; 3] = [0, 0, 255];

pub(super) fn composite_onion_skin(
    dst: &mut [u8],
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
    onion: OnionSkin,
    viewport: Rect,
) -> Result<(), RenderError> {
    let current = request.frame.0;
    let frame_count = sprite.frames.len() as u32;
    let back = onion.frames_back.min(current);
    let forward = onion
        .frames_forward
        .min(frame_count.saturating_sub(current + 1));
    if (back == 0 || onion.previous_alpha == 0) && (forward == 0 || onion.next_alpha == 0) {
        return Ok(());
    }

    let mut scratch = vec![0u8; dst.len()];
    let passes = (1..=back)
        .rev()
        .map(|k| (current - k, k, back, onion.previous_alpha, PREVIOUS_TINT))
        .chain(
            (1..=forward)
                .rev()
                .map(|k| (current + k, k, forward, onion.next_alpha, NEXT_TINT)),
        );
    for (frame, k, n, alpha, tint) in passes {
        if alpha == 0 {
            continue;
        }
        scratch.fill(0);
        composite_visible_layers(
            &mut scratch,
            sprite,
            cels,
            &request.include_layers,
            FrameIndex::new(frame),
            viewport,
        )?;
        let fade = (u32::from(alpha) * (n - k + 1) / n) as u8;
        for (d, s) in dst
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(scratch.as_chunks::<4>().0)
        {
            if s[3] == 0 {
                continue;
            }
            let mix = |c: u8, t: u8| ((u16::from(c) + u16::from(t)) / 2) as u8;
            blend_pixel_into(
                BlendMode::Normal,
                d,
                mix(s[0], tint[0]),
                mix(s[1], tint[1]),
                mix(s[2], tint[2]),
                mul_u8(s[3], fade),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::document::{Cel, CelMap, FrameIndex, LayerId};
    use crate::render::OnionSkin;
    use crate::render::request::ComposeRequest;

    use super::super::test_support::{compose_owned, full_req, one_layer_sprite, solid};

    /// A 1×1, three-frame sprite whose frames are opaque red / green /
    /// blue; frame 1 (green) is transparent when `hole` is set.
    fn rgb_frames(hole: bool) -> (crate::document::Sprite, CelMap) {
        let sprite = one_layer_sprite(1, 1, 3);
        let mut cels = CelMap::new();
        let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
        for (i, c) in colors.iter().enumerate() {
            if hole && i == 1 {
                continue;
            }
            cels.insert(Cel::image(
                LayerId::new(0),
                FrameIndex::new(i as u32),
                solid(1, 1, *c),
            ));
        }
        (sprite, cels)
    }

    fn req(frame: u32, onion: OnionSkin) -> ComposeRequest {
        ComposeRequest {
            frame: FrameIndex::new(frame),
            onion_skin: Some(onion),
            ..full_req(1, 1)
        }
    }

    #[test]
    fn opaque_current_frame_hides_the_onion() {
        let (sprite, cels) = rgb_frames(false);
        let r = compose_owned(&sprite, &cels, &req(1, OnionSkin::default())).unwrap();
        assert_eq!(r.pixels, vec![0, 255, 0, 255]);
    }

    #[test]
    fn previous_frame_shows_red_tinted_under_a_hole() {
        let (sprite, cels) = rgb_frames(true);
        let onion = OnionSkin {
            previous_alpha: 255,
            frames_forward: 0,
            ..OnionSkin::default()
        };
        let r = compose_owned(&sprite, &cels, &req(1, onion)).unwrap();
        // Frame 0 is pure red; mixing toward the red tint keeps r = 255
        // and halves the (zero) green / blue.
        assert_eq!(r.pixels, vec![255, 0, 0, 255]);
    }

    #[test]
    fn next_frame_is_blue_tinted_at_next_alpha() {
        let (sprite, cels) = rgb_frames(true);
        let onion = OnionSkin {
            next_alpha: 128,
            frames_back: 0,
            ..OnionSkin::default()
        };
        let r = compose_owned(&sprite, &cels, &req(1, onion)).unwrap();
        assert_eq!(r.pixels, vec![0, 0, 255, 128]);
    }

    #[test]
    fn farther_frames_fade_linearly() {
        // Frame 0 red, frames 1 and 2 empty: rendering frame 2 with two
        // frames back puts frame 0 at distance 2 of 2 → half alpha.
        let sprite = one_layer_sprite(1, 1, 3);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        let onion = OnionSkin {
            previous_alpha: 200,
            frames_back: 2,
            frames_forward: 0,
            ..OnionSkin::default()
        };
        let r = compose_owned(&sprite, &cels, &req(2, onion)).unwrap();
        assert_eq!(r.pixels[3], 100);
    }

    /// A 1×1, three-frame sprite with a single opaque cel of `color` on
    /// frame `at`; the other frames are empty.
    fn single_cel(at: u32, color: [u8; 4]) -> (crate::document::Sprite, CelMap) {
        let sprite = one_layer_sprite(1, 1, 3);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(at),
            solid(1, 1, color),
        ));
        (sprite, cels)
    }

    #[test]
    fn previous_frame_tint_moves_each_channel_halfway_to_red() {
        let (sprite, cels) = single_cel(0, [100, 150, 200, 255]);
        let onion = OnionSkin {
            previous_alpha: 255,
            frames_back: 1,
            frames_forward: 0,
            ..OnionSkin::default()
        };
        let r = compose_owned(&sprite, &cels, &req(1, onion)).unwrap();
        // ((100 + 255) / 2, 150 / 2, 200 / 2)
        assert_eq!(r.pixels, vec![177, 75, 100, 255]);
    }

    #[test]
    fn next_frame_tint_moves_each_channel_halfway_to_blue() {
        let (sprite, cels) = single_cel(2, [100, 150, 200, 255]);
        let onion = OnionSkin {
            next_alpha: 255,
            frames_back: 0,
            frames_forward: 1,
            ..OnionSkin::default()
        };
        let r = compose_owned(&sprite, &cels, &req(1, onion)).unwrap();
        // (100 / 2, 150 / 2, (200 + 255) / 2)
        assert_eq!(r.pixels, vec![50, 75, 227, 255]);
    }

    #[test]
    fn first_frame_does_not_wrap_to_the_last_frame() {
        // Only the last frame has pixels; wrapping back from frame 0 would
        // show it.
        let (sprite, cels) = single_cel(2, [0, 255, 0, 255]);
        let onion = OnionSkin {
            previous_alpha: 255,
            frames_back: 2,
            frames_forward: 0,
            ..OnionSkin::default()
        };
        let r = compose_owned(&sprite, &cels, &req(0, onion)).unwrap();
        assert_eq!(r.pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn last_frame_does_not_wrap_to_the_first_frame() {
        let (sprite, cels) = single_cel(0, [0, 255, 0, 255]);
        let onion = OnionSkin {
            next_alpha: 255,
            frames_back: 0,
            frames_forward: 2,
            ..OnionSkin::default()
        };
        let r = compose_owned(&sprite, &cels, &req(2, onion)).unwrap();
        assert_eq!(r.pixels, vec![0, 0, 0, 0]);
    }

    #[test]
    fn first_frame_without_neighbours_renders_plainly() {
        let (sprite, cels) = rgb_frames(false);
        let onion = OnionSkin {
            frames_forward: 0,
            ..OnionSkin::default()
        };
        let plain = compose_owned(&sprite, &cels, &full_req(1, 1)).unwrap();
        let r = compose_owned(&sprite, &cels, &req(0, onion)).unwrap();
        assert_eq!(r.pixels, plain.pixels);
    }
}
