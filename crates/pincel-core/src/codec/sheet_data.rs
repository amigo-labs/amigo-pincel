//! Aseprite-compatible sprite-sheet data (spec §7.3): the sidecar JSON
//! that `aseprite --sheet … --data` writes, in its "array" layout, so
//! engines and tools that already read Aseprite sheets read Pincel's.
//!
//! Frame, tag and slice indices are *atlas* indices: when the sheet holds
//! only one tag's frames, tag ranges are clipped to the exported frames and
//! renumbered, and slice keys are re-emitted at the first exported frame
//! where their geometry applies.

use std::fmt::{self, Write as _};

use crate::document::{FrameIndex, Rgba, SliceKey, Sprite, TagDirection};
use crate::geometry::Rect;

use super::png::{AtlasManifest, escape_json};

/// Build the Aseprite `--data` JSON for an atlas exported from `sprite`.
///
/// `image` is the PNG's file name as the JSON's `meta.image` should name
/// it (e.g. `"hero.png"`); frame `filename`s are derived from its stem.
///
/// A scaled sheet ([`AtlasManifest::scale`] above 1) is described as
/// Aseprite's CLI `--scale` describes one: frame rects, sizes, slice
/// bounds, centers and pivots are multiplied by the scale, `meta.size` is
/// the scaled PNG's size, and `meta.scale` stays `"1"`.
pub fn aseprite_sheet_json(sprite: &Sprite, manifest: &AtlasManifest, image: &str) -> String {
    let mut out = String::new();
    write_sheet(&mut out, sprite, manifest, image)
        .expect("fmt::Write for String never returns an error");
    out
}

/// Append the sheet JSON for [`aseprite_sheet_json`] to `out`.
fn write_sheet(
    out: &mut String,
    sprite: &Sprite,
    manifest: &AtlasManifest,
    image: &str,
) -> fmt::Result {
    let stem = escape_json(image.rsplit_once('.').map_or(image, |(s, _)| s));
    let exported: Vec<FrameIndex> = manifest.frames.iter().map(|f| f.frame).collect();

    out.push_str("{\"frames\":[");
    for (i, f) in manifest.frames.iter().enumerate() {
        let duration = sprite
            .frames
            .get(f.frame.0 as usize)
            .map_or(100, |fr| fr.duration_ms);
        let (x, y, w, h) = (f.x, f.y, f.w, f.h);
        if i > 0 {
            out.push(',');
        }
        write!(
            out,
            "{{\"filename\":\"{stem} {}\",\"frame\":{{\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}},\
             \"rotated\":false,\"trimmed\":false,\
             \"spriteSourceSize\":{{\"x\":0,\"y\":0,\"w\":{w},\"h\":{h}}},\
             \"sourceSize\":{{\"w\":{w},\"h\":{h}}},\"duration\":{duration}}}",
            f.frame.0
        )?;
    }

    write!(
        out,
        "],\"meta\":{{\"app\":\"https://pincel.amigo-labs.dev/\",\
         \"version\":\"{}\",\"image\":\"{}\",\"format\":\"RGBA8888\",\
         \"size\":{{\"w\":{},\"h\":{}}},\"scale\":\"1\",\"frameTags\":[",
        env!("CARGO_PKG_VERSION"),
        escape_json(image),
        manifest.width,
        manifest.height
    )?;
    let tags = sprite.tags.iter().filter_map(|t| {
        let first = exported.iter().position(|f| *f >= t.from && *f <= t.to)?;
        let last = exported.iter().rposition(|f| *f >= t.from && *f <= t.to)?;
        Some((t, first, last))
    });
    for (i, (t, first, last)) in tags.enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(
            out,
            "{{\"name\":\"{}\",\"from\":{first},\"to\":{last},\
             \"direction\":\"{}\",\"color\":\"#{:08x}\"}}",
            escape_json(&t.name),
            direction_name(t.direction),
            packed_rgba(t.color)
        )?;
    }

    out.push_str("],\"layers\":[],\"slices\":[");
    // A slice with no key in effect on any exported frame is left out.
    let slices = sprite.slices.iter().filter_map(|s| {
        let mut keys = atlas_keys(&s.keys, &exported).peekable();
        keys.peek()?;
        Some((s, keys))
    });
    for (i, (s, keys)) in slices.enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(
            out,
            "{{\"name\":\"{}\",\"color\":\"#{:08x}\",\"keys\":[",
            escape_json(&s.name),
            packed_rgba(s.color)
        )?;
        for (k, (atlas_index, key)) in keys.enumerate() {
            if k > 0 {
                out.push(',');
            }
            write_key(out, atlas_index, key, manifest.scale)?;
        }
        out.push_str("]}");
    }
    out.push_str("]}}");
    Ok(())
}

/// `r` multiplied by `scale`, each value widened before the product
/// (`i32 × u32` fits `i64`, `u32 × u32` fits `u64`), so it is exact.
fn write_scaled_rect(out: &mut String, r: Rect, scale: u32) -> fmt::Result {
    let (x, y) = (
        i64::from(r.x) * i64::from(scale),
        i64::from(r.y) * i64::from(scale),
    );
    let (w, h) = (
        u64::from(r.width) * u64::from(scale),
        u64::from(r.height) * u64::from(scale),
    );
    write!(out, "{{\"x\":{x},\"y\":{y},\"w\":{w},\"h\":{h}}}")
}

/// `c` as one `0xRRGGBBAA` number: written with `{:08x}` after a `#`, it
/// is Aseprite's `#rrggbbaa`.
fn packed_rgba(c: Rgba) -> u32 {
    u32::from_be_bytes([c.r, c.g, c.b, c.a])
}

fn direction_name(d: TagDirection) -> &'static str {
    match d {
        TagDirection::Forward => "forward",
        TagDirection::Reverse => "reverse",
        TagDirection::Pingpong => "pingpong",
        TagDirection::PingpongReverse => "pingpong_reverse",
    }
}

/// `keys` at atlas indices: for every exported frame, the key in effect
/// there; a key is yielded only at the first exported frame it governs,
/// paired with that atlas index. Keys are told apart by identity, not by
/// geometry, so two equal keys at different frames are both yielded.
///
/// `keys` is **not** guaranteed sorted by frame: editing keeps it sorted
/// (`SetSliceKey` inserts at its `partition_point`, see
/// `command/set_slice_key.rs`), but a loaded file keeps the keys in file
/// order (`build_slice` in `codec/aseprite_read.rs`). The key in effect is
/// therefore found by a reverse linear scan — the key *stored* last among
/// those starting at or before the frame — which is the last key at or
/// before it when `keys` is sorted and still well-defined when it is not.
/// A binary search (`partition_point`) would change the output for
/// unsorted keys; `aseprite_sheet_json_unsorted_keys_use_last_stored_key`
/// pins this.
fn atlas_keys<'a>(
    keys: &'a [SliceKey],
    exported: &'a [FrameIndex],
) -> impl Iterator<Item = (usize, &'a SliceKey)> {
    let mut last: Option<&SliceKey> = None;
    exported
        .iter()
        .enumerate()
        .filter_map(move |(atlas_index, frame)| {
            let key = keys.iter().rev().find(|k| k.frame <= *frame)?;
            if last.is_some_and(|l| std::ptr::eq(l, key)) {
                return None;
            }
            last = Some(key);
            Some((atlas_index, key))
        })
}

/// One slice key as JSON at `atlas_index`, its geometry multiplied by
/// `scale` (the sheet's integer upscale), exactly: see [`write_scaled_rect`].
fn write_key(out: &mut String, atlas_index: usize, key: &SliceKey, scale: u32) -> fmt::Result {
    write!(out, "{{\"frame\":{atlas_index},\"bounds\":")?;
    write_scaled_rect(out, key.bounds, scale)?;
    if let Some(c) = key.center {
        out.push_str(",\"center\":");
        write_scaled_rect(out, c, scale)?;
    }
    if let Some((px, py)) = key.pivot {
        let (px, py) = (
            i64::from(px) * i64::from(scale),
            i64::from(py) * i64::from(scale),
        );
        write!(out, ",\"pivot\":{{\"x\":{px},\"y\":{py}}}")?;
    }
    out.push('}');
    Ok(())
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

    /// Expected values follow Aseprite's CLI `--scale 3`: the sprite is
    /// resized before the export, so every rect, size and pivot is 3×,
    /// `meta.size` is the sheet PNG's size and `meta.scale` stays `"1"`.
    #[test]
    fn scaled_sheet_multiplies_geometry_and_keeps_meta_scale_one() {
        let mut s = sprite();
        s.slices[0].keys[0].center = Some(Rect::new(0, 0, 1, 1));
        let out = export_atlas_png(&s, &CelMap::new(), &AtlasOptions::new(3).with_scale(3))
            .expect("atlas");
        let json = aseprite_sheet_json(&s, &out.manifest, "hero.png");
        assert!(json.contains(
            "{\"filename\":\"hero 1\",\"frame\":{\"x\":6,\"y\":0,\"w\":6,\"h\":6},\
             \"rotated\":false,\"trimmed\":false,\
             \"spriteSourceSize\":{\"x\":0,\"y\":0,\"w\":6,\"h\":6},\
             \"sourceSize\":{\"w\":6,\"h\":6},\"duration\":50}"
        ));
        assert!(json.contains("\"size\":{\"w\":18,\"h\":6},\"scale\":\"1\""));
        assert!(json.contains(
            "\"keys\":[{\"frame\":0,\"bounds\":{\"x\":0,\"y\":0,\"w\":3,\"h\":3},\
             \"center\":{\"x\":0,\"y\":0,\"w\":3,\"h\":3},\"pivot\":{\"x\":3,\"y\":3}},\
             {\"frame\":2,\"bounds\":{\"x\":3,\"y\":3,\"w\":3,\"h\":3}}]"
        ));
        // A key without center or pivot keeps them absent.
        assert!(!json.contains("\"w\":3,\"h\":3},\"center\":{\"x\":3"));
    }

    /// The whole document, byte for byte: key order, number formatting
    /// (negative scaled positions included) and escaping are part of the
    /// contract engines parse.
    #[test]
    fn aseprite_sheet_json_scaled_sheet_matches_golden_bytes() {
        let mut s = sprite();
        s.slices[0].keys[0].center = Some(Rect::new(-1, 2, 1, 1));
        s.slices[0].keys[1].pivot = Some((-2, 5));
        let out = export_atlas_png(&s, &CelMap::new(), &AtlasOptions::new(3).with_scale(3))
            .expect("atlas");
        let json = aseprite_sheet_json(&s, &out.manifest, "he\"ro.png");
        let expected = concat!(
            "{\"frames\":[",
            "{\"filename\":\"he\\\"ro 0\",\"frame\":{\"x\":0,\"y\":0,\"w\":6,\"h\":6},\"rotated\":false,\"trimmed\":false,\"spriteSourceSize\":{\"x\":0,\"y\":0,\"w\":6,\"h\":6},\"sourceSize\":{\"w\":6,\"h\":6},\"duration\":100},",
            "{\"filename\":\"he\\\"ro 1\",\"frame\":{\"x\":6,\"y\":0,\"w\":6,\"h\":6},\"rotated\":false,\"trimmed\":false,\"spriteSourceSize\":{\"x\":0,\"y\":0,\"w\":6,\"h\":6},\"sourceSize\":{\"w\":6,\"h\":6},\"duration\":50},",
            "{\"filename\":\"he\\\"ro 2\",\"frame\":{\"x\":12,\"y\":0,\"w\":6,\"h\":6},\"rotated\":false,\"trimmed\":false,\"spriteSourceSize\":{\"x\":0,\"y\":0,\"w\":6,\"h\":6},\"sourceSize\":{\"w\":6,\"h\":6},\"duration\":70}",
            "],\"meta\":{\"app\":\"https://pincel.amigo-labs.dev/\",\"version\":\"",
            env!("CARGO_PKG_VERSION"),
            "\",\"image\":\"he\\\"ro.png\",\"format\":\"RGBA8888\",\"size\":{\"w\":18,\"h\":6},\"scale\":\"1\",",
            "\"frameTags\":[{\"name\":\"idle\",\"from\":0,\"to\":0,\"direction\":\"forward\",\"color\":\"#ffffffff\"},",
            "{\"name\":\"run\",\"from\":1,\"to\":2,\"direction\":\"pingpong\",\"color\":\"#ffffffff\"}],",
            "\"layers\":[],\"slices\":[{\"name\":\"hit\",\"color\":\"#0000ffff\",\"keys\":[",
            "{\"frame\":0,\"bounds\":{\"x\":0,\"y\":0,\"w\":3,\"h\":3},\"center\":{\"x\":-3,\"y\":6,\"w\":3,\"h\":3},\"pivot\":{\"x\":3,\"y\":3}},",
            "{\"frame\":2,\"bounds\":{\"x\":3,\"y\":3,\"w\":3,\"h\":3},\"pivot\":{\"x\":-6,\"y\":15}}",
            "]}]}}"
        );
        assert_eq!(json, expected);
    }

    /// A slice with no key in effect on any exported frame is left out
    /// whole — no empty entry, no stray separator.
    #[test]
    fn aseprite_sheet_json_slice_without_exported_key_is_dropped() {
        let mut s = sprite();
        let mut late = s.slices[0].clone();
        late.name = "late".into();
        late.keys.remove(0);
        s.slices.insert(0, late);
        let opts = AtlasOptions::new(1).with_tag("idle");
        let out = export_atlas_png(&s, &CelMap::new(), &opts).expect("atlas");
        let json = aseprite_sheet_json(&s, &out.manifest, "idle.png");
        assert!(json.ends_with(concat!(
            r##""layers":[],"slices":[{"name":"hit","color":"#0000ffff","keys":["##,
            r##"{"frame":0,"bounds":{"x":0,"y":0,"w":1,"h":1},"pivot":{"x":1,"y":1}}]}]}}"##,
        )));
    }

    /// Every scaled number is widened before the product, so the largest
    /// slice a `Rect` can hold at the largest scale is written exactly —
    /// never wrapped, never saturated to `u32::MAX`.
    #[test]
    fn write_key_extreme_bounds_at_max_scale_writes_exact_products() {
        let key = SliceKey {
            frame: FrameIndex::new(0),
            bounds: Rect::new(i32::MIN, i32::MAX, u32::MAX, u32::MAX),
            center: Some(Rect::new(-5, 0, u32::MAX, 1)),
            pivot: Some((i32::MIN, i32::MAX)),
        };
        let keys: Vec<String> = atlas_keys(&[key], &[FrameIndex::new(0)])
            .map(|(atlas_index, key)| {
                let mut json = String::new();
                write_key(&mut json, atlas_index, key, u32::MAX).expect("String write");
                json
            })
            .collect();
        assert_eq!(
            keys,
            vec![
                "{\"frame\":0,\
                 \"bounds\":{\"x\":-9223372034707292160,\"y\":9223372030412324865,\
                 \"w\":18446744065119617025,\"h\":18446744065119617025},\
                 \"center\":{\"x\":-21474836475,\"y\":0,\
                 \"w\":18446744065119617025,\"h\":4294967295},\
                 \"pivot\":{\"x\":-9223372034707292160,\"y\":9223372030412324865}}"
                    .to_string()
            ]
        );
    }

    /// The full sheet's manifest, cut down to the frames `keep` selects.
    fn manifest_with(s: &Sprite, keep: impl Fn(FrameIndex) -> bool) -> AtlasManifest {
        let mut manifest = export_atlas_png(s, &CelMap::new(), &AtlasOptions::new(3))
            .expect("atlas")
            .manifest;
        manifest.frames.retain(|f| keep(f.frame));
        manifest
    }

    /// Keys loaded from a file keep file order (`build_slice` in
    /// `codec/aseprite_read.rs` does not sort), so they can arrive out of
    /// frame order. The key in effect is the one *stored* last among those
    /// starting at or before the frame: with keys stored for frames
    /// `[0, 2, 1]`, frame 2 is governed by the frame-1 key (stored after
    /// the frame-2 key), so the frame-2 key is never written. A sorted-only
    /// lookup gives a different result here (a binary search lands the
    /// frame-1 key at atlas frame 2; a sort first writes all three keys),
    /// which this test is meant to catch.
    #[test]
    fn aseprite_sheet_json_unsorted_keys_use_last_stored_key() {
        let mut s = sprite();
        s.slices[0].keys.push(SliceKey {
            frame: FrameIndex::new(1),
            bounds: Rect::new(0, 1, 2, 1),
            center: None,
            pivot: None,
        });
        let frames: Vec<u32> = s.slices[0].keys.iter().map(|k| k.frame.0).collect();
        assert_eq!(frames, [0, 2, 1], "keys are stored out of frame order");
        let json = aseprite_sheet_json(&s, &manifest_with(&s, |_| true), "hero.png");
        assert!(json.ends_with(concat!(
            r##""slices":[{"name":"hit","color":"#0000ffff","keys":["##,
            r##"{"frame":0,"bounds":{"x":0,"y":0,"w":1,"h":1},"pivot":{"x":1,"y":1}},"##,
            r##"{"frame":1,"bounds":{"x":0,"y":1,"w":2,"h":1}}]}]}}"##,
        )));
    }

    #[test]
    fn aseprite_sheet_json_no_exported_frames_writes_empty_lists() {
        let s = sprite();
        let json = aseprite_sheet_json(&s, &manifest_with(&s, |_| false), "hero.png");
        assert!(json.starts_with("{\"frames\":[],\"meta\":{"));
        assert!(json.ends_with("\"frameTags\":[],\"layers\":[],\"slices\":[]}}"));
    }

    #[test]
    fn aseprite_sheet_json_tags_outside_export_are_all_dropped() {
        let mut s = sprite();
        s.tags[1].to = FrameIndex::new(1);
        let json = aseprite_sheet_json(
            &s,
            &manifest_with(&s, |f| f == FrameIndex::new(2)),
            "hero.png",
        );
        assert!(json.contains("\"filename\":\"hero 2\""));
        assert!(json.contains("\"frameTags\":[],\"layers\""));
    }

    #[test]
    fn aseprite_sheet_json_slice_without_keys_is_dropped() {
        let mut s = sprite();
        s.slices.push(Slice {
            id: SliceId::new(1),
            name: "empty".into(),
            color: Rgba::new(255, 0, 0, 255),
            keys: Vec::new(),
            user_data: None,
        });
        let json = aseprite_sheet_json(&s, &manifest_with(&s, |_| true), "hero.png");
        assert!(!json.contains("\"name\":\"empty\""));
        assert!(json.contains("\"slices\":[{\"name\":\"hit\""));
        assert!(json.ends_with("}]}]}}"), "only the hit slice is listed");
    }

    #[test]
    fn aseprite_sheet_json_quote_and_newline_in_names_are_escaped() {
        let mut s = sprite();
        s.tags[0].name = "id\"le\nA".into();
        s.slices[0].name = "h\"it\nB".into();
        let json = aseprite_sheet_json(&s, &manifest_with(&s, |_| true), "he\"ro\nC.png");
        assert!(json.contains("\"filename\":\"he\\\"ro\\nC 0\""));
        assert!(json.contains("\"image\":\"he\\\"ro\\nC.png\""));
        assert!(json.contains("{\"name\":\"id\\\"le\\nA\",\"from\":0"));
        assert!(json.contains("{\"name\":\"h\\\"it\\nB\",\"color\""));
        assert!(!json.contains('\n'), "no raw newline reaches the JSON");
    }

    /// Channel order is r, g, b, a and every channel keeps two digits, so
    /// a leading zero channel and a zero alpha are both written.
    #[test]
    fn packed_rgba_with_08x_writes_aseprite_hex() {
        let color = packed_rgba(Rgba::new(0x01, 0x0a, 0xb0, 0x00));
        assert_eq!(format!("#{color:08x}"), "#010ab000");
    }

    /// Keys are told apart by identity, not geometry: a second key with the
    /// same bounds, center and pivot is still written where it starts.
    #[test]
    fn atlas_keys_two_keys_with_equal_geometry_are_both_written() {
        let key = |frame| SliceKey {
            frame: FrameIndex::new(frame),
            bounds: Rect::new(1, 2, 3, 4),
            center: None,
            pivot: Some((0, 0)),
        };
        let keys = [key(0), key(1)];
        let exported = [FrameIndex::new(0), FrameIndex::new(1), FrameIndex::new(2)];
        let written: Vec<String> = atlas_keys(&keys, &exported)
            .map(|(atlas_index, key)| {
                let mut json = String::new();
                write_key(&mut json, atlas_index, key, 1).expect("String write");
                json
            })
            .collect();
        let expected = |atlas: usize| {
            format!(
                "{{\"frame\":{atlas},\"bounds\":{{\"x\":1,\"y\":2,\"w\":3,\"h\":4}},\
                 \"pivot\":{{\"x\":0,\"y\":0}}}}"
            )
        };
        assert_eq!(written, vec![expected(0), expected(1)]);
    }
}
