//! `grid.set` and the `doc.info` / `doc.list` asset fields (AC-28 … AC-30).

use super::*;

#[test]
fn grid_set_is_reported_and_saved() {
    let mut s = S::new();
    s.new_doc(8, 8);
    assert_eq!(
        s.ok("doc.info", json!({}))["grid"],
        json!({ "x": 0, "y": 0, "width": 16, "height": 16 })
    );
    s.ok(
        "grid.set",
        json!({ "x": -2, "y": 3, "width": 8, "height": 4 }),
    );
    let grid = json!({ "x": -2, "y": 3, "width": 8, "height": 4 });
    assert_eq!(s.ok("doc.info", json!({}))["grid"], grid);
    s.ok("doc.save", json!({ "path": "g.aseprite" }));
    let out = read_aseprite(&s.host.files["g.aseprite"]).expect("read");
    assert_eq!(
        out.sprite.metadata.grid_or_default(),
        pincel_core::Rect::new(-2, 3, 8, 4)
    );
    s.ok("doc.open", json!({ "path": "g.aseprite" }));
    assert_eq!(s.ok("doc.info", json!({}))["grid"], grid);
}

#[test]
fn an_opened_files_zero_size_header_grid_is_reported_and_replaceable() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    let (mut sprite, cels) = s.state(&id);
    sprite.metadata.grid = Some(pincel_core::Rect::new(1, -2, 0, 5));
    let mut bytes = Vec::new();
    pincel_core::write_aseprite(&sprite, &cels, &mut bytes).expect("write");
    let header = read_aseprite(&bytes).expect("read").sprite.metadata.grid;
    assert_eq!(header, Some(pincel_core::Rect::new(1, -2, 0, 5)));
    s.host.files.insert("zero.aseprite".into(), bytes);
    s.ok("doc.open", json!({ "path": "zero.aseprite" }));
    let stored = json!({ "x": 1, "y": -2, "width": 0, "height": 5 });
    assert_eq!(s.ok("doc.info", json!({}))["grid"], stored);
    let list = s.ok("doc.list", json!({}))["documents"].clone();
    assert_eq!(list[1]["grid"], stored);
    let grid = json!({ "x": 0, "y": 0, "width": 4, "height": 4 });
    assert_eq!(s.ok("grid.set", grid.clone())["changed"], true);
    assert_eq!(s.ok("doc.info", json!({}))["grid"], grid);
}

#[test]
fn grid_set_out_of_range_leaves_the_grid() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    // The argument and its range, from the schema check.
    for (args, message) in [
        (
            json!({ "x": 0, "y": 0, "width": 0, "height": 4 }),
            "width must be 1–65535",
        ),
        (
            json!({ "x": 0, "y": 0, "width": 4, "height": 65536 }),
            "height must be 1–65535",
        ),
        (
            json!({ "x": 40000, "y": 0, "width": 4, "height": 4 }),
            "x must be -32768–32767",
        ),
        (
            json!({ "x": 0, "y": -40000, "width": 4, "height": 4 }),
            "y must be -32768–32767",
        ),
    ] {
        let before = s.state(&id);
        let e = s.call("grid.set", args.clone()).unwrap_err();
        assert_eq!(
            e,
            crate::AgentError::InvalidArgs {
                tool: "grid.set".to_owned(),
                message: message.to_owned(),
            },
            "{args}"
        );
        assert_eq!(s.state(&id), before);
    }
    // A missing argument is a schema mismatch.
    let before = s.state(&id);
    let e = s
        .call("grid.set", json!({ "x": 0, "y": 0, "width": 4 }))
        .unwrap_err();
    assert!(
        matches!(&e, crate::AgentError::InvalidArgs { message, .. } if message.starts_with("missing required argument `height`")),
        "{e:?}"
    );
    assert_eq!(s.state(&id), before);
}

#[test]
fn doc_info_reports_slice_and_tileset_counts_and_grid() {
    let mut s = S::new();
    let pid = tiled(&mut s);
    s.ok("slice.add", json!({ "name": "a" }));
    let info = s.ok("doc.info", json!({}));
    assert_eq!(
        (info["slices"].clone(), info["tilesets"].clone()),
        (json!(1), json!(1))
    );
    s.new_image(4, 4);
    let info = s.ok("doc.info", json!({}));
    for key in ["slices", "tilesets", "grid"] {
        assert!(info.get(key).is_none(), "{key}");
    }
    // doc.list reports the same per document: the Pixel one with its
    // counts and grid, the Image one with none of these fields.
    let list = s.ok("doc.list", json!({}))["documents"].clone();
    assert_eq!(
        (
            list[0]["slices"].clone(),
            list[0]["tilesets"].clone(),
            list[0]["grid"].clone()
        ),
        (
            json!(1),
            json!(1),
            json!({ "x": 0, "y": 0, "width": 16, "height": 16 })
        )
    );
    for key in ["slices", "tilesets", "grid"] {
        assert!(list[1].get(key).is_none(), "Image entry: {key}");
    }
    // The counts follow edits, in doc.info and doc.list alike.
    s.ok("slice.add", json!({ "doc": pid, "name": "b" }));
    for name in ["c", "d"] {
        s.ok(
            "tileset.add",
            json!({ "doc": pid, "name": name, "tile_width": 2, "tile_height": 2 }),
        );
    }
    let info = s.ok("doc.info", json!({ "doc": pid }));
    assert_eq!(
        (info["slices"].clone(), info["tilesets"].clone()),
        (json!(2), json!(3))
    );
    let list = s.ok("doc.list", json!({}))["documents"].clone();
    assert_eq!(
        (list[0]["slices"].clone(), list[0]["tilesets"].clone()),
        (json!(2), json!(3))
    );
}
