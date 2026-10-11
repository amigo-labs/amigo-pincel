//! Timeline surface (M14.5): frame durations, frame removal, animation
//! tags and onion-skin settings. See `docs/specs/pincel.md` §3.3, §3.6,
//! §4.2.
//!
//! Every mutator routes through the undo bus. Duration and tag edits are
//! playback metadata and emit no dirty event — the UI re-reads the
//! timeline after each call. Frame removal and onion-skin changes alter
//! the composite and emit `dirty-canvas`. Tag directions cross the
//! boundary as snake_case names (`forward`, `reverse`, `pingpong`,
//! `pingpong_reverse`).

use pincel_core::{
    AddTag, FrameIndex, OnionSkin, RemoveFrame, RemoveTag, Rgba, SetFrameDuration, SetTagDirection,
    SetTagName, SetTagRange, Tag, TagDirection,
};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

fn direction_name(d: TagDirection) -> &'static str {
    match d {
        TagDirection::Forward => "forward",
        TagDirection::Reverse => "reverse",
        TagDirection::Pingpong => "pingpong",
        TagDirection::PingpongReverse => "pingpong_reverse",
    }
}

fn direction_from_name(name: &str) -> Result<TagDirection, String> {
    Ok(match name {
        "forward" => TagDirection::Forward,
        "reverse" => TagDirection::Reverse,
        "pingpong" => TagDirection::Pingpong,
        "pingpong_reverse" => TagDirection::PingpongReverse,
        other => return Err(format!("unknown tag direction `{other}`")),
    })
}

#[wasm_bindgen]
impl Document {
    /// Display duration of `frame` in milliseconds. Errors when out of
    /// range.
    #[wasm_bindgen(js_name = frameDuration)]
    pub fn frame_duration(&self, frame: u32) -> Result<u16, String> {
        self.sprite
            .frames
            .get(frame as usize)
            .map(|f| f.duration_ms)
            .ok_or_else(|| format!("frame {frame} out of range"))
    }

    /// Set `frame`'s duration (ms). Consecutive edits to the same frame
    /// merge into one undo entry until [`Document::end_stroke`] seals it.
    #[wasm_bindgen(js_name = setFrameDuration)]
    pub fn set_frame_duration(&mut self, frame: u32, duration_ms: u16) -> Result<(), String> {
        if duration_ms == 0 {
            return Err("frame duration must be at least 1 ms".into());
        }
        let cmd = SetFrameDuration::new(FrameIndex::new(frame), duration_ms);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to set frame duration: {e}"))?;
        Ok(())
    }

    /// Remove `frame`, shifting later cels, tags and slice keys down (see
    /// `pincel_core::RemoveFrame`). The current frame is clamped into the
    /// new range. One undo entry; emits `dirty-canvas`. Errors on an
    /// out-of-range index or when it is the last remaining frame.
    #[wasm_bindgen(js_name = removeFrame)]
    pub fn remove_frame(&mut self, frame: u32) -> Result<(), String> {
        self.bus.seal();
        self.bus
            .execute(
                RemoveFrame::new(FrameIndex::new(frame)).into(),
                &mut self.sprite,
                &mut self.cels,
            )
            .map_err(|e| format!("failed to remove frame: {e}"))?;
        self.bus.seal();
        if self.current_frame.0 > frame || self.current_frame.0 >= self.frame_count() {
            self.current_frame = FrameIndex::new(self.current_frame.0.saturating_sub(1));
        }
        self.clamp_current_frame();
        self.events.push(Event::dirty_canvas());
        Ok(())
    }

    /// Number of animation tags.
    #[wasm_bindgen(getter, js_name = tagCount)]
    pub fn tag_count(&self) -> u32 {
        self.sprite.tags.len() as u32
    }

    /// Name of the tag at `index`.
    #[wasm_bindgen(js_name = tagName)]
    pub fn tag_name(&self, index: u32) -> Result<String, String> {
        self.tag(index).map(|t| t.name.clone())
    }

    /// First frame (inclusive) of the tag at `index`.
    #[wasm_bindgen(js_name = tagFrom)]
    pub fn tag_from(&self, index: u32) -> Result<u32, String> {
        self.tag(index).map(|t| t.from.0)
    }

    /// Last frame (inclusive) of the tag at `index`.
    #[wasm_bindgen(js_name = tagTo)]
    pub fn tag_to(&self, index: u32) -> Result<u32, String> {
        self.tag(index).map(|t| t.to.0)
    }

    /// Playback direction of the tag at `index` as a snake_case name.
    #[wasm_bindgen(js_name = tagDirection)]
    pub fn tag_direction(&self, index: u32) -> Result<String, String> {
        self.tag(index)
            .map(|t| direction_name(t.direction).to_owned())
    }

    /// Display color of the tag at `index`, packed `0xRRGGBBAA`.
    #[wasm_bindgen(js_name = tagColor)]
    pub fn tag_color(&self, index: u32) -> Result<u32, String> {
        self.tag(index).map(|t| t.color.to_u32())
    }

    /// Append a forward-playing tag over `from..=to` and return its index.
    /// `color` is `0xRRGGBBAA`. One undo entry.
    #[wasm_bindgen(js_name = addTag)]
    pub fn add_tag(&mut self, name: &str, from: u32, to: u32, color: u32) -> Result<u32, String> {
        let mut tag = Tag::new(name, FrameIndex::new(from), FrameIndex::new(to));
        tag.color = Rgba::from_u32(color);
        self.bus.seal();
        self.bus
            .execute(AddTag::new(tag).into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to add tag: {e}"))?;
        self.bus.seal();
        Ok(self.tag_count() - 1)
    }

    /// Remove the tag at `index`. One undo entry.
    #[wasm_bindgen(js_name = removeTag)]
    pub fn remove_tag(&mut self, index: u32) -> Result<(), String> {
        self.bus.seal();
        self.bus
            .execute(
                RemoveTag::new(index as usize).into(),
                &mut self.sprite,
                &mut self.cels,
            )
            .map_err(|e| format!("failed to remove tag: {e}"))?;
        self.bus.seal();
        Ok(())
    }

    /// Set the frame range of the tag at `index`. Consecutive calls for
    /// the same tag merge until [`Document::end_stroke`].
    #[wasm_bindgen(js_name = setTagRange)]
    pub fn set_tag_range(&mut self, index: u32, from: u32, to: u32) -> Result<(), String> {
        let cmd = SetTagRange::new(index as usize, FrameIndex::new(from), FrameIndex::new(to));
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to set tag range: {e}"))?;
        Ok(())
    }

    /// Rename the tag at `index`. Consecutive calls for the same tag merge
    /// until [`Document::end_stroke`].
    #[wasm_bindgen(js_name = setTagName)]
    pub fn set_tag_name(&mut self, index: u32, name: &str) -> Result<(), String> {
        self.bus
            .execute(
                SetTagName::new(index as usize, name).into(),
                &mut self.sprite,
                &mut self.cels,
            )
            .map_err(|e| format!("failed to rename tag: {e}"))?;
        Ok(())
    }

    /// Set the playback direction of the tag at `index` by snake_case
    /// name. One undo entry.
    #[wasm_bindgen(js_name = setTagDirection)]
    pub fn set_tag_direction(&mut self, index: u32, direction: &str) -> Result<(), String> {
        let direction = direction_from_name(direction)?;
        self.bus.seal();
        self.bus
            .execute(
                SetTagDirection::new(index as usize, direction).into(),
                &mut self.sprite,
                &mut self.cels,
            )
            .map_err(|e| format!("failed to set tag direction: {e}"))?;
        self.bus.seal();
        Ok(())
    }

    /// Configure the onion skin used by [`Document::compose`] /
    /// [`Document::compose_dirty`]. `enabled = false` turns it off; the
    /// alphas are `0..=255`. View state, not a document edit: no undo
    /// entry. Emits `dirty-canvas`.
    #[wasm_bindgen(js_name = setOnionSkin)]
    pub fn set_onion_skin(
        &mut self,
        enabled: bool,
        frames_back: u32,
        frames_forward: u32,
        previous_alpha: u8,
        next_alpha: u8,
    ) {
        self.onion_skin = enabled.then_some(OnionSkin {
            previous_alpha,
            next_alpha,
            frames_back,
            frames_forward,
        });
        self.events.push(Event::dirty_canvas());
    }

    /// Whether the onion skin is currently enabled.
    #[wasm_bindgen(getter, js_name = onionSkinEnabled)]
    pub fn onion_skin_enabled(&self) -> bool {
        self.onion_skin.is_some()
    }
}

impl Document {
    fn tag(&self, index: u32) -> Result<&Tag, String> {
        self.sprite
            .tags
            .get(index as usize)
            .ok_or_else(|| format!("tag index {index} out of range"))
    }

    /// Keep `current_frame` inside `0..frameCount` after a command (or
    /// its undo / redo) changed the frame list.
    pub(crate) fn clamp_current_frame(&mut self) {
        let last = self.frame_count().saturating_sub(1);
        if self.current_frame.0 > last {
            self.current_frame = FrameIndex::new(last);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_frames() -> Document {
        let mut doc = Document::new(2, 2).expect("doc");
        doc.add_frame(100).expect("frame");
        doc.add_frame(100).expect("frame");
        doc
    }

    #[test]
    fn set_frame_duration_round_trips_through_undo() {
        let mut doc = three_frames();
        doc.set_frame_duration(1, 40).expect("set");
        assert_eq!(doc.frame_duration(1), Ok(40));
        assert!(doc.undo());
        assert_eq!(doc.frame_duration(1), Ok(100));
    }

    #[test]
    fn set_frame_duration_zero_is_rejected() {
        let mut doc = three_frames();
        assert!(doc.set_frame_duration(0, 0).is_err());
    }

    #[test]
    fn remove_frame_clamps_the_current_frame() {
        let mut doc = three_frames();
        doc.set_current_frame(2).expect("select");
        doc.remove_frame(2).expect("remove");
        assert_eq!(doc.frame_count(), 2);
        assert_eq!(doc.current_frame(), 1);
    }

    #[test]
    fn remove_frame_before_current_keeps_the_same_content_selected() {
        let mut doc = three_frames();
        doc.set_current_frame(2).expect("select");
        doc.remove_frame(0).expect("remove");
        assert_eq!(doc.current_frame(), 1);
    }

    #[test]
    fn remove_last_remaining_frame_errors() {
        let mut doc = Document::new(2, 2).expect("doc");
        assert!(doc.remove_frame(0).is_err());
    }

    #[test]
    fn undo_of_add_frame_clamps_the_current_frame() {
        let mut doc = Document::new(2, 2).expect("doc");
        doc.add_frame(100).expect("frame");
        doc.set_current_frame(1).expect("select");
        assert!(doc.undo());
        assert_eq!(doc.current_frame(), 0);
    }

    #[test]
    fn tag_lifecycle_through_the_surface() {
        let mut doc = three_frames();
        let i = doc.add_tag("idle", 0, 1, 0xff00_00ff).expect("add");
        assert_eq!(doc.tag_count(), 1);
        assert_eq!(doc.tag_name(i).as_deref(), Ok("idle"));
        assert_eq!((doc.tag_from(i), doc.tag_to(i)), (Ok(0), Ok(1)));
        assert_eq!(doc.tag_color(i), Ok(0xff00_00ff));
        doc.set_tag_range(i, 1, 2).expect("range");
        doc.end_stroke();
        doc.set_tag_name(i, "walk").expect("name");
        doc.end_stroke();
        doc.set_tag_direction(i, "pingpong").expect("dir");
        assert_eq!(doc.tag_direction(i).as_deref(), Ok("pingpong"));
        assert_eq!(doc.tag_name(i).as_deref(), Ok("walk"));
        assert_eq!(doc.tag_to(i), Ok(2));
        doc.remove_tag(i).expect("remove");
        assert_eq!(doc.tag_count(), 0);
        assert!(doc.undo());
        assert_eq!(doc.tag_count(), 1);
    }

    #[test]
    fn unknown_tag_direction_is_rejected() {
        let mut doc = three_frames();
        doc.add_tag("idle", 0, 0, 0).expect("add");
        assert!(doc.set_tag_direction(0, "sideways").is_err());
    }

    #[test]
    fn onion_skin_toggles_and_composes() {
        let mut doc = three_frames();
        doc.set_onion_skin(true, 1, 1, 76, 76);
        assert!(doc.onion_skin_enabled());
        doc.set_current_frame(1).expect("select");
        assert!(doc.compose(1, 1).is_ok());
        doc.set_onion_skin(false, 0, 0, 0, 0);
        assert!(!doc.onion_skin_enabled());
    }
}
