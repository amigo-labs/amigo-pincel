//! Aseprite-compatible sprite-sheet data (spec §7.3): the sidecar JSON
//! that `aseprite --sheet … --data` writes, in its "array" layout, so
//! engines and tools that already read Aseprite sheets read Pincel's.
//!
//! Frame, tag and slice indices are *atlas* indices: when the sheet holds
//! only one tag's frames, tag ranges are clipped to the exported frames and
//! renumbered, and slice keys are re-emitted at the first exported frame
//! where their geometry applies.

use crate::document::{FrameIndex, Rgba, SliceKey, Sprite, TagDirection};

use super::png::{AtlasManifest, escape_json};

/// Build the Aseprite `--data` JSON for an atlas exported from `sprite`.
///
/// `image` is the PNG's file name as the JSON's `meta.image` should name
/// it (e.g. `"hero.png"`); frame `filename`s are derived from its stem.
pub fn aseprite_sheet_json(sprite: &Sprite, manifest: &AtlasManifest, image: &str) -> String {
    let stem = image.rsplit_once('.').map_or(image, |(s, _)| s);
    let exported: Vec<FrameIndex> = manifest.frames.iter().map(|f| f.frame).collect();

    let frames: Vec<String> = manifest
        .frames
        .iter()
        .map(|f| {
            let duration = sprite
                .frames
                .get(f.frame.0 as usize)
                .map_or(100, |fr| fr.duration_ms);
            format!(
                "{{\"filename\":\"{} {}\",\"frame\":{},\"rotated\":false,\"trimmed\":false,\
                 \"spriteSourceSize\":{{\"x\":0,\"y\":0,\"w\":{},\"h\":{}}},\
                 \"sourceSize\":{{\"w\":{},\"h\":{}}},\"duration\":{}}}",
                escape_json(stem),
                f.frame.0,
                rect_json(f.x as i64, f.y as i64, f.w, f.h),
                f.w,
                f.h,
                f.w,
                f.h,
                duration
            )
        })
        .collect();

    let tags: Vec<String> = sprite
        .tags
        .iter()
        .filter_map(|t| {
            let first = exported.iter().position(|f| *f >= t.from && *f <= t.to)?;
            let last = exported.iter().rposition(|f| *f >= t.from && *f <= t.to)?;
            Some(format!(
                "{{\"name\":\"{}\",\"from\":{},\"to\":{},\"direction\":\"{}\",\"color\":\"{}\"}}",
                escape_json(&t.name),
                first,
                last,
                direction_name(t.direction),
                hex(t.color)
            ))
        })
        .collect();

    let slices: Vec<String> = sprite
        .slices
        .iter()
        .filter_map(|s| {
            let keys = remap_keys(&s.keys, &exported);
            if keys.is_empty() {
                return None;
            }
            Some(format!(
                "{{\"name\":\"{}\",\"color\":\"{}\",\"keys\":[{}]}}",
                escape_json(&s.name),
                hex(s.color),
                keys.join(",")
            ))
        })
        .collect();

    format!(
        "{{\"frames\":[{}],\"meta\":{{\"app\":\"https://pincel.amigo-labs.dev/\",\
         \"version\":\"{}\",\"image\":\"{}\",\"format\":\"RGBA8888\",\
         \"size\":{{\"w\":{},\"h\":{}}},\"scale\":\"1\",\"frameTags\":[{}],\
         \"layers\":[],\"slices\":[{}]}}}}",
        frames.join(","),
        env!("CARGO_PKG_VERSION"),
        escape_json(image),
        manifest.width,
        manifest.height,
        tags.join(","),
        slices.join(",")
    )
}

fn rect_json(x: i64, y: i64, w: u32, h: u32) -> String {
    format!("{{\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}")
}

fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

fn direction_name(d: TagDirection) -> &'static str {
    match d {
        TagDirection::Forward => "forward",
        TagDirection::Reverse => "reverse",
        TagDirection::Pingpong => "pingpong",
        TagDirection::PingpongReverse => "pingpong_reverse",
    }
}

/// Re-emit `keys` at atlas indices: for every exported frame, the key in
/// effect there (the last key at or before it); a new JSON key is written
/// only where that geometry changes.
fn remap_keys(keys: &[SliceKey], exported: &[FrameIndex]) -> Vec<String> {
    let mut out = Vec::new();
    let mut last: Option<&SliceKey> = None;
    for (atlas_index, frame) in exported.iter().enumerate() {
        let active = keys.iter().rev().find(|k| k.frame <= *frame);
        let Some(key) = active else { continue };
        if last.is_some_and(|l| std::ptr::eq(l, key)) {
            continue;
        }
        last = Some(key);
        let b = key.bounds;
        let mut json = format!(
            "{{\"frame\":{},\"bounds\":{}",
            atlas_index,
            rect_json(i64::from(b.x), i64::from(b.y), b.width, b.height)
        );
        if let Some(c) = key.center {
            json.push_str(&format!(
                ",\"center\":{}",
                rect_json(i64::from(c.x), i64::from(c.y), c.width, c.height)
            ));
        }
        if let Some((px, py)) = key.pivot {
            json.push_str(&format!(",\"pivot\":{{\"x\":{px},\"y\":{py}}}"));
        }
        json.push('}');
        out.push(json);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{AtlasOptions, export_atlas_png};
    use crate::document::{CelMap, Frame, Layer, LayerId, Slice, SliceId, Tag};
    use crate::geometry::Rect;

    fn sprite() -> Sprite {
        let mut s = Sprite::builder(2, 2)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(50))
            .add_frame(Frame::new(70))
            .add_tag(Tag::new("idle", FrameIndex::new(0), FrameIndex::new(0)))
            .add_tag(Tag::new("run", FrameIndex::new(1), FrameIndex::new(2)))
            .build()
            .expect("sprite builds");
        s.tags[1].direction = TagDirection::Pingpong;
        s.slices.push(Slice {
            id: SliceId::new(0),
            name: "hit".into(),
            color: Rgba::new(0, 0, 255, 255),
            keys: vec![
                SliceKey {
                    frame: FrameIndex::new(0),
                    bounds: Rect::new(0, 0, 1, 1),
                    center: None,
                    pivot: Some((1, 1)),
                },
                SliceKey {
                    frame: FrameIndex::new(2),
                    bounds: Rect::new(1, 1, 1, 1),
                    center: None,
                    pivot: None,
                },
            ],
            user_data: None,
        });
        s
    }

    #[test]
    fn full_sheet_lists_frames_durations_tags_and_slices() {
        let s = sprite();
        let out = export_atlas_png(&s, &CelMap::new(), &AtlasOptions::new(3)).expect("atlas");
        let json = aseprite_sheet_json(&s, &out.manifest, "hero.png");
        assert!(json.starts_with("{\"frames\":[{\"filename\":\"hero 0\""));
        assert!(json.contains("\"frame\":{\"x\":2,\"y\":0,\"w\":2,\"h\":2}"));
        assert!(json.contains("\"duration\":50"));
        assert!(json.contains("\"image\":\"hero.png\""));
        assert!(json.contains("\"size\":{\"w\":6,\"h\":2}"));
        assert!(json.contains(
            "{\"name\":\"run\",\"from\":1,\"to\":2,\"direction\":\"pingpong\",\"color\":\"#ffffffff\"}"
        ));
        assert!(json.contains(
            "\"keys\":[{\"frame\":0,\"bounds\":{\"x\":0,\"y\":0,\"w\":1,\"h\":1},\"pivot\":{\"x\":1,\"y\":1}},\
             {\"frame\":2,\"bounds\":{\"x\":1,\"y\":1,\"w\":1,\"h\":1}}]"
        ));
    }

    #[test]
    fn tag_sheet_renumbers_tags_and_slice_keys() {
        let s = sprite();
        let opts = AtlasOptions::new(2).with_tag("run");
        let out = export_atlas_png(&s, &CelMap::new(), &opts).expect("atlas");
        let json = aseprite_sheet_json(&s, &out.manifest, "run.png");
        assert!(json.contains("\"filename\":\"run 1\""));
        assert!(
            !json.contains("\"name\":\"idle\""),
            "idle is outside the sheet"
        );
        assert!(json.contains("\"name\":\"run\",\"from\":0,\"to\":1"));
        // Atlas frame 0 (sprite frame 1) inherits the frame-0 key; the
        // frame-2 key lands at atlas frame 1.
        assert!(json.contains("{\"frame\":0,\"bounds\":{\"x\":0,\"y\":0"));
        assert!(json.contains("{\"frame\":1,\"bounds\":{\"x\":1,\"y\":1"));
    }
}
