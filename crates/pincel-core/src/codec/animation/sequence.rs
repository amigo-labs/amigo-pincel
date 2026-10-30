//! Frame order of an animated export: all frames forward, or one tag
//! played once in its direction. Mirrors the playback stepping in
//! `ui/src/modes/pixel/playback.ts` (`startCursor` / `advance`) for one
//! cycle. See `docs/specs/pincel.md` §3.6.

use crate::codec::png::{ExportError, select_frames};
use crate::document::{FrameIndex, Sprite, TagDirection};

/// The frames of one animation cycle, in display order.
///
/// `None` exports every frame of the sprite once, forward. `Some(name)`
/// exports the named tag's frames in the order of its direction: Forward
/// `from…to`, Reverse `to…from`, Pingpong `from…to` then back down to
/// `from + 1`, PingpongReverse `to…from` then back up to `to − 1`. A tag
/// whose direction byte Pincel doesn't know (`Tag::raw_direction`) plays
/// forward, as in playback.
///
/// # Errors
///
/// [`ExportError::TagNotFound`] for an unknown tag,
/// [`ExportError::TagRangeInvalid`] for a corrupt tag range.
pub fn animation_frames(
    sprite: &Sprite,
    tag: Option<&str>,
) -> Result<Vec<FrameIndex>, ExportError> {
    let mut frames = select_frames(sprite, tag)?;
    // `select_frames` resolved the same first tag of that name, so the
    // lookup cannot miss here; an absent tag means "all frames, forward".
    let direction = tag
        .and_then(|name| sprite.tags.iter().find(|t| t.name == name))
        .map_or(TagDirection::Forward, |t| {
            if t.raw_direction.is_some() {
                TagDirection::Forward
            } else {
                t.direction
            }
        });
    let reverse = matches!(
        direction,
        TagDirection::Reverse | TagDirection::PingpongReverse
    );
    if reverse {
        frames.reverse();
    }
    let pingpong = matches!(
        direction,
        TagDirection::Pingpong | TagDirection::PingpongReverse
    );
    if pingpong && frames.len() > 2 {
        // Travel back without repeating either end frame.
        let back: Vec<FrameIndex> = frames[1..frames.len() - 1].iter().rev().copied().collect();
        frames.extend(back);
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Frame, Layer, LayerId, Tag};

    fn sprite_with(frames: u32, tag: Option<Tag>) -> Sprite {
        let mut builder = Sprite::builder(1, 1).add_layer(Layer::image(LayerId::new(0), "art"));
        for _ in 0..frames {
            builder = builder.add_frame(Frame::new(100));
        }
        if let Some(tag) = tag {
            builder = builder.add_tag(tag);
        }
        builder.build().expect("sprite builds")
    }

    fn tag(from: u32, to: u32, direction: TagDirection) -> Tag {
        let mut tag = Tag::new("t", FrameIndex::new(from), FrameIndex::new(to));
        tag.direction = direction;
        tag
    }

    fn order(sprite: &Sprite, tag: Option<&str>) -> Vec<u32> {
        animation_frames(sprite, tag)
            .expect("sequence")
            .into_iter()
            .map(|f| f.0)
            .collect()
    }

    #[test]
    fn animation_frames_all_frames_are_forward_once() {
        let sprite = sprite_with(4, Some(tag(1, 2, TagDirection::Reverse)));
        assert_eq!(order(&sprite, None), [0, 1, 2, 3]);
    }

    #[test]
    fn animation_frames_forward_tag_runs_from_to() {
        let sprite = sprite_with(5, Some(tag(1, 3, TagDirection::Forward)));
        assert_eq!(order(&sprite, Some("t")), [1, 2, 3]);
    }

    #[test]
    fn animation_frames_reverse_tag_runs_to_from() {
        let sprite = sprite_with(5, Some(tag(1, 3, TagDirection::Reverse)));
        assert_eq!(order(&sprite, Some("t")), [3, 2, 1]);
    }

    #[test]
    fn animation_frames_pingpong_returns_without_repeating_ends() {
        let sprite = sprite_with(3, Some(tag(0, 2, TagDirection::Pingpong)));
        assert_eq!(order(&sprite, Some("t")), [0, 1, 2, 1]);
        let two = sprite_with(2, Some(tag(0, 1, TagDirection::Pingpong)));
        assert_eq!(order(&two, Some("t")), [0, 1]);
    }

    #[test]
    fn animation_frames_pingpong_reverse_starts_at_to() {
        let sprite = sprite_with(3, Some(tag(0, 2, TagDirection::PingpongReverse)));
        assert_eq!(order(&sprite, Some("t")), [2, 1, 0, 1]);
    }

    #[test]
    fn animation_frames_single_frame_tag_is_one_frame_in_every_direction() {
        for direction in [
            TagDirection::Forward,
            TagDirection::Reverse,
            TagDirection::Pingpong,
            TagDirection::PingpongReverse,
        ] {
            let sprite = sprite_with(3, Some(tag(1, 1, direction)));
            assert_eq!(order(&sprite, Some("t")), [1], "{direction:?}");
        }
    }

    #[test]
    fn animation_frames_unknown_direction_byte_plays_forward() {
        let mut unknown = tag(0, 2, TagDirection::Reverse);
        unknown.raw_direction = Some(7);
        let sprite = sprite_with(3, Some(unknown));
        assert_eq!(order(&sprite, Some("t")), [0, 1, 2]);
    }

    #[test]
    fn animation_frames_unknown_tag_returns_err() {
        let sprite = sprite_with(2, None);
        assert!(matches!(
            animation_frames(&sprite, Some("missing")),
            Err(ExportError::TagNotFound { .. })
        ));
    }
}
