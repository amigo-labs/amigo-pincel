//! `slice.*` (AC-04 … AC-10).

use super::*;

#[test]
fn slice_add_list_and_remove_like_the_editor() {
    let mut s = S::new();
    s.new_doc(16, 8);
    let out = s.ok("slice.add", json!({ "name": "  body ", "bounds": { "x": 1, "y": 2, "width": 3, "height": 4 }, "color": "#00ff0080" }));
    assert_eq!(out["slice"], 0);
    let out = s.ok("slice.add", json!({ "name": "all" }));
    assert_eq!(out["slice"], 1);
    let list = s.ok("slice.list", json!({}))["slices"].clone();
    assert_eq!(
        list,
        json!([
            { "id": 0, "name": "body", "color": "#00FF0080", "keys": [{ "frame": 0, "bounds": { "x": 1, "y": 2, "width": 3, "height": 4 }, "center": null, "pivot": null }] },
            { "id": 1, "name": "all", "color": "#3B82F6FF", "keys": [{ "frame": 0, "bounds": { "x": 0, "y": 0, "width": 16, "height": 8 }, "center": null, "pivot": null }] },
        ])
    );
    s.ok("slice.remove", json!({ "slice": 0 }));
    assert_eq!(
        s.ok("slice.list", json!({}))["slices"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    s.ok("doc.undo", json!({}));
    assert_eq!(
        s.ok("slice.list", json!({}))["slices"],
        list,
        "undo restores the position"
    );
}

#[test]
fn slice_add_with_bad_arguments_adds_nothing() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    for (args, arg) in [
        (json!({ "name": "  " }), "name"),
        (
            json!({ "name": "a", "bounds": { "x": 0, "y": 0, "width": 0, "height": 2 } }),
            "width",
        ),
        (json!({ "name": "a", "color": "blue" }), "color"),
    ] {
        let before = s.state(&id);
        let e = s.err("slice.add", args.clone());
        assert!(e.contains(arg), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn slice_set_key_upserts_by_frame_and_inherits_fields() {
    let mut s = S::new();
    s.new_doc(16, 8);
    for _ in 0..3 {
        s.ok("frame.add", json!({}));
    }
    s.ok(
        "slice.add",
        json!({ "name": "a", "bounds": { "x": 0, "y": 0, "width": 8, "height": 8 } }),
    );
    s.ok("slice.set_key", json!({ "slice": 0, "center": { "x": 1, "y": 1, "width": 6, "height": 6 }, "pivot": { "x": 4, "y": 8 } }));
    // Frame 2 inherits bounds, center and pivot from frame 0's key.
    s.ok(
        "slice.set_key",
        json!({ "slice": 0, "frame": 2, "bounds": { "x": 2, "y": 0, "width": 8, "height": 8 } }),
    );
    s.ok(
        "slice.set_key",
        json!({ "slice": 0, "frame": 1, "pivot": null }),
    );
    let keys = s.ok("slice.list", json!({}))["slices"][0]["keys"].clone();
    assert_eq!(
        keys,
        json!([
            { "frame": 0, "bounds": { "x": 0, "y": 0, "width": 8, "height": 8 }, "center": { "x": 1, "y": 1, "width": 6, "height": 6 }, "pivot": { "x": 4, "y": 8 } },
            { "frame": 1, "bounds": { "x": 0, "y": 0, "width": 8, "height": 8 }, "center": { "x": 1, "y": 1, "width": 6, "height": 6 }, "pivot": null },
            { "frame": 2, "bounds": { "x": 2, "y": 0, "width": 8, "height": 8 }, "center": { "x": 1, "y": 1, "width": 6, "height": 6 }, "pivot": { "x": 4, "y": 8 } },
        ])
    );
}

#[test]
fn slice_set_key_with_bad_arguments_leaves_the_slice() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    s.ok("frame.add", json!({}));
    s.ok("slice.add", json!({ "name": "a" }));
    s.ok(
        "slice.set_key",
        json!({ "slice": 0, "frame": 1, "pivot": { "x": 1, "y": 1 } }),
    );
    s.ok("slice.remove", json!({ "slice": 0 }));
    s.ok("doc.undo", json!({}));
    for (args, arg) in [
        (json!({ "slice": 0 }), "bounds"),
        (
            json!({ "slice": 0, "bounds": { "x": 0, "y": 0, "width": 0, "height": 1 } }),
            "bounds.width",
        ),
        (
            json!({ "slice": 0, "center": { "x": 0, "y": 0, "width": 1, "height": 0 } }),
            "center",
        ),
        (json!({ "slice": 0, "pivot": { "x": 1 } }), "pivot"),
    ] {
        let before = s.state(&id);
        let e = s.err("slice.set_key", args.clone());
        assert!(e.contains(arg), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
    // No key before frame 1 but a first key at 0: a key before the first
    // one needs bounds. Build a slice whose first key is at frame 1.
    {
        let d = s.ws.get_mut(&id).expect("doc");
        d.sprite.slices[0].keys.remove(0);
    }
    let e = s.err(
        "slice.set_key",
        json!({ "slice": 0, "frame": 0, "pivot": { "x": 0, "y": 0 } }),
    );
    assert!(e.contains("`bounds`"), "{e}");
}
