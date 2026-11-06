//! `export.selection`: the headless Copy and Cut (AC-14 … AC-19).

use super::*;

#[test]
fn export_selection_writes_the_editors_copy() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok(
        "select.ellipse",
        json!({ "x": 0, "y": 0, "width": 4, "height": 4 }),
    );
    let before = s.state(&id);
    let d = s.doc(&id);
    let want = copy_pixels(
        &d.sprite,
        &d.cels,
        FrameIndex::new(0),
        CopySource::Layer(LayerId::new(0)),
    )
    .expect("copy");
    let out = s.ok("export.selection", json!({ "path": "copy.bin" }));
    assert_eq!(
        out["area"],
        json!({ "x": want.rect.x, "y": want.rect.y, "w": want.rect.width, "h": want.rect.height })
    );
    let got = import_png(&s.host.files["copy.bin"]).expect("png");
    assert_eq!(got.rgba, want.rgba);
    assert_eq!(s.state(&id), before);
    // A group copies its rendered pixels.
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok(
        "layer.move",
        json!({ "layer": 0, "target": 1, "place": "into" }),
    );
    let d = s.doc(&id);
    let want = copy_pixels(
        &d.sprite,
        &d.cels,
        FrameIndex::new(0),
        CopySource::Layer(LayerId::new(1)),
    )
    .expect("copy");
    s.ok("export.selection", json!({ "path": "g.png", "layer": 1 }));
    assert_eq!(
        import_png(&s.host.files["g.png"]).expect("png").rgba,
        want.rgba
    );

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.ellipse",
        json!({ "x": 0, "y": 0, "width": 6, "height": 5, "feather": 1 }),
    );
    let (region, pixels) = copy_selection(s.image(&id).document(), false).expect("copy");
    let out = s.ok("export.selection", json!({ "path": "c.png" }));
    assert_eq!(
        out["area"],
        json!({ "x": region.x, "y": region.y, "w": region.w, "h": region.h })
    );
    assert_eq!(
        decode(&s.host.files["c.png"]).expect("png").data(),
        pixels.data()
    );
    // Without a selection: the whole canvas.
    s.ok("select.clear", json!({}));
    let out = s.ok("export.selection", json!({ "path": "all.png" }));
    assert_eq!(out["area"], json!({ "x": 0, "y": 0, "w": 8, "h": 6 }));
}

#[test]
fn export_selection_merged_writes_the_editors_copy_merged() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#00000080" }] }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 3, "height": 3 }),
    );
    let d = s.doc(&id);
    let want =
        copy_pixels(&d.sprite, &d.cels, FrameIndex::new(0), CopySource::Merged).expect("copy");
    let out = s.ok(
        "export.selection",
        json!({ "path": "m.png", "merged": true }),
    );
    assert_eq!(
        out["area"],
        json!({ "x": want.rect.x, "y": want.rect.y, "w": want.rect.width, "h": want.rect.height })
    );
    assert_eq!(
        import_png(&s.host.files["m.png"]).expect("png").rgba,
        want.rgba
    );

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#00ff0040" }));
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 4, "height": 3 }),
    );
    let (region, pixels) = copy_selection(s.image(&id).document(), true).expect("copy");
    let out = s.ok(
        "export.selection",
        json!({ "path": "m.png", "merged": true }),
    );
    assert_eq!(
        out["area"],
        json!({ "x": region.x, "y": region.y, "w": region.w, "h": region.h })
    );
    assert_eq!(
        decode(&s.host.files["m.png"]).expect("png").data(),
        pixels.data()
    );
}

#[test]
fn export_selection_cut_clears_like_the_editors_cut() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 1 }),
    );
    let depth = undo_depth(&mut s);
    let out = s.ok(
        "export.selection",
        json!({ "path": "cut.png", "cut": true }),
    );
    assert_eq!(out["changed"], true);
    assert_eq!(undo_depth(&mut s), depth + 1);
    let rgba = s.cel_rgba(&id, 0, 0);
    assert_eq!(px(&rgba, 6, 1, 1), [0, 0, 0, 0]);
    assert_eq!(
        px(&rgba, 6, 2, 2),
        [0, 0, 255, 128],
        "outside the selection"
    );
    let cut = import_png(&s.host.files["cut.png"]).expect("png");
    assert_eq!(cut.rgba, vec![255, 0, 0, 255, 0, 255, 0, 255]);
    s.ok("doc.undo", json!({}));
    assert!(s.host.files.contains_key("cut.png"), "the file stays");
    assert_eq!(px(&s.cel_rgba(&id, 0, 0), 6, 1, 1), [255, 0, 0, 255]);
    // No selection: the whole layer.
    s.ok("select.clear", json!({}));
    s.ok(
        "export.selection",
        json!({ "path": "all.png", "cut": true }),
    );
    assert!(s.cel_rgba(&id, 0, 0).iter().all(|&b| b == 0));

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.ellipse",
        json!({ "x": 0, "y": 0, "width": 6, "height": 5, "feather": 1 }),
    );
    let want = image_editor(&s, &id, CommandSpec::DeleteSelection { layer: 0 });
    let (_, copied) = copy_selection(s.image(&id).document(), false).expect("copy");
    let before = s.image(&id).document().clone();
    let depth = undo_depth(&mut s);
    s.ok("export.selection", json!({ "path": "c.png", "cut": true }));
    assert_eq!(undo_depth(&mut s), depth + 1);
    assert_eq!(s.image(&id).document().layers(), want.layers());
    assert_eq!(
        decode(&s.host.files["c.png"]).expect("png").data(),
        copied.data(),
        "written as for Copy"
    );
    s.ok("doc.undo", json!({}));
    assert!(s.host.files.contains_key("c.png"), "the file stays");
    assert_eq!(s.image(&id).document().layers(), before.layers());
    // No selection: the whole layer.
    s.ok("select.clear", json!({}));
    let want = image_editor(&s, &id, CommandSpec::DeleteSelection { layer: 0 });
    s.ok(
        "export.selection",
        json!({ "path": "all.png", "cut": true }),
    );
    assert_eq!(s.image(&id).document().layers(), want.layers());
    assert!(
        s.image(&id).document().layers()[0]
            .pixels
            .data()
            .iter()
            .all(|&b| b == 0),
        "the whole layer is cleared"
    );
}

#[test]
fn export_selection_round_trips_through_layer_import() {
    for image in [false, true] {
        let mut s = S::new();
        let id = if image {
            image_doc(&mut s)
        } else {
            pixel_doc(&mut s)
        };
        s.ok(
            "select.rect",
            json!({ "x": 1, "y": 1, "width": 3, "height": 3 }),
        );
        let out = s.ok("export.selection", json!({ "path": "r.png" }));
        let (x, y) = (out["area"]["x"].clone(), out["area"]["y"].clone());
        let file = s.host.files["r.png"].clone();
        let other = if image {
            s.new_image(8, 6)
        } else {
            s.new_doc(6, 5)
        };
        s.host.files.insert("r.png".into(), file.clone());
        let imported = s.ok(
            "layer.import",
            json!({ "path": "r.png", "x": x, "y": y, "doc": other }),
        );
        let png = import_png(&file).expect("png");
        let (x, y) = (x.as_i64().expect("x") as u32, y.as_i64().expect("y") as u32);
        if image {
            let d = s.image(&other).document();
            let idx = s
                .image(&other)
                .layer_index(imported["layer"].as_u64().expect("id") as u32)
                .expect("i");
            let layer = &d.layers()[idx].pixels;
            for py in 0..png.height {
                for pxx in 0..png.width {
                    let c = layer.get_pixel(x + pxx, y + py).expect("pixel");
                    let i = ((py * png.width + pxx) * 4) as usize;
                    assert_eq!([c.r, c.g, c.b, c.a], png.rgba[i..i + 4], "({pxx}, {py})");
                }
            }
        } else {
            let rgba = s.cel_rgba(&other, imported["layer"].as_u64().expect("id") as u32, 0);
            for py in 0..png.height {
                for pxx in 0..png.width {
                    let i = ((py * png.width + pxx) * 4) as usize;
                    assert_eq!(
                        px(&rgba, 6, x + pxx, y + py),
                        png.rgba[i..i + 4],
                        "({pxx}, {py})"
                    );
                }
            }
        }
        let _ = id;
    }
}

#[test]
fn export_selection_bad_combinations_write_nothing() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    s.ws.get_mut(&id)
        .expect("doc")
        .sprite
        .layers
        .push(pincel_core::Layer::tilemap(
            LayerId::new(7),
            "Map",
            pincel_core::TilesetId::new(0),
        ));
    for (args, needle) in [
        (
            json!({ "path": "a.png", "cut": true, "merged": true }),
            "`cut` and `merged`",
        ),
        (
            json!({ "path": "a.png", "merged": true, "layer": 0 }),
            "`merged` and `layer`",
        ),
        (
            json!({ "path": "a.png", "cut": true, "layer": 0 }),
            "locked",
        ),
        (
            json!({ "path": "a.png", "cut": true, "layer": 1 }),
            "a group",
        ),
        (
            json!({ "path": "a.png", "cut": true, "layer": 7 }),
            "a tilemap layer",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err("export.selection", args.clone());
        assert!(e.contains(needle), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
    assert!(s.host.files.is_empty());

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 8, "height": 6, "mode": "subtract" }),
    );
    let before = s.image(&id).document().clone();
    let e = s.err("export.selection", json!({ "path": "a.png" }));
    assert!(e.contains("no selected pixel"), "{e}");
    assert_eq!(s.image(&id).document(), &before);
    assert!(s.host.files.is_empty());

    // Image: the combinations, and a cut on a locked layer, a group and a
    // layer inside a locked group.
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 3, "height": 3 }),
    );
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok(
        "layer.move",
        json!({ "layer": 0, "target": 1, "place": "into" }),
    );
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    let check = |s: &mut S, args: Value, needles: &[&str]| {
        let before = s.image(&id).document().clone();
        let e = s.err("export.selection", args.clone());
        assert!(needles.iter().all(|n| e.contains(n)), "{args}: {e}");
        assert_eq!(s.image(&id).document(), &before, "{args}");
        assert!(s.host.files.is_empty(), "{args}: nothing is written");
    };
    check(
        &mut s,
        json!({ "path": "a.png", "cut": true, "merged": true }),
        &["`cut` and `merged`"],
    );
    check(
        &mut s,
        json!({ "path": "a.png", "merged": true, "layer": 0 }),
        &["`merged` and `layer`"],
    );
    check(
        &mut s,
        json!({ "path": "a.png", "cut": true, "layer": 0 }),
        &["layer 0", "locked"],
    );
    check(
        &mut s,
        json!({ "path": "a.png", "cut": true, "layer": 1 }),
        &["layer 1", "group"],
    );
    s.ok("layer.set", json!({ "layer": 0, "locked": false }));
    s.ok("layer.set", json!({ "layer": 1, "locked": true }));
    check(
        &mut s,
        json!({ "path": "a.png", "cut": true, "layer": 0 }),
        &["layer 0", "locked"],
    );
}

#[test]
fn export_selection_clips_to_the_canvas_and_cuts_the_selection_shape() {
    // AC-14: the area is the selection's bounds clipped to the canvas, or
    // the whole canvas without a selection.
    let mut s = S::new();
    pixel_doc(&mut s);
    let out = s.ok("export.selection", json!({ "path": "all.png" }));
    assert_eq!(out["area"], json!({ "x": 0, "y": 0, "w": 6, "h": 5 }));
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 3, "height": 2 }),
    );
    s.ok("select.move_content", json!({ "dx": 4, "dy": 0 }));
    let out = s.ok("export.selection", json!({ "path": "edge.png" }));
    assert_eq!(out["area"], json!({ "x": 5, "y": 1, "w": 1, "h": 2 }));
    let png = decode(&s.host.files["edge.png"]).expect("png");
    assert_eq!((png.width(), png.height()), (1, 2));

    // AC-16: a cut clears only the selection's shape.
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 2, "y": 1, "width": 1, "height": 1, "mode": "subtract" }),
    );
    s.ok("export.selection", json!({ "path": "c.png", "cut": true }));
    let cel = s.cel_rgba(&id, 0, 0);
    assert_eq!(px(&cel, 6, 1, 1), [0, 0, 0, 0], "cut");
    assert_eq!(px(&cel, 6, 2, 2), [0, 0, 0, 0], "cut");
    assert_eq!(px(&cel, 6, 2, 1), [0, 255, 0, 255], "outside the shape");
    // The file holds the copied shape only (AC-14).
    let png = decode(&s.host.files["c.png"]).expect("png");
    assert_eq!(
        png.data(),
        [[255, 0, 0, 255], [0; 4], [0; 4], [0, 0, 255, 128]].concat(),
        "the cut's file"
    );

    // AC-16: a cut of another layer and frame than the active ones.
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok("frame.add", json!({}));
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 1 }),
    );
    let depth = undo_depth(&mut s);
    s.ok(
        "export.selection",
        json!({ "path": "c.png", "cut": true, "layer": 0, "frame": 0 }),
    );
    assert_eq!(undo_depth(&mut s), depth + 1);
    let cel = s.cel_rgba(&id, 0, 0);
    assert_eq!(px(&cel, 6, 1, 1), [0, 0, 0, 0]);
    assert_eq!(px(&cel, 6, 2, 1), [0, 0, 0, 0]);
    assert_eq!(
        px(&cel, 6, 5, 4),
        [255, 255, 255, 255],
        "outside the selection"
    );
    let png = decode(&s.host.files["c.png"]).expect("png");
    assert_eq!(png.data()[..4], [255, 0, 0, 255]);

    // Image: a cut of a layer other than the active one.
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    let want = image_editor(&s, &id, CommandSpec::DeleteSelection { layer: 0 });
    s.ok(
        "export.selection",
        json!({ "path": "i.png", "cut": true, "layer": 0 }),
    );
    assert_eq!(s.image(&id).document().layers(), want.layers());
}

/// The Image editor's Copy of the layer with id `layer`: its
/// `copy_selection` with that layer active.
fn image_copy_of(s: &S, id: &str, layer: u32) -> (pincel_image_core::Rect, ImageBuffer) {
    let mut d = s.image(id).document().clone();
    let index = s.image(id).layer_index(layer).expect("layer");
    d.set_active_layer(index).expect("activate");
    copy_selection(&d, false).expect("copy")
}

#[test]
fn export_selection_copies_the_target_layer_at_the_target_frame() {
    // Pixel: the default is the active layer whatever its kind.
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#123456" }] }),
    );
    s.ok("select.all", json!({}));
    let copy = |s: &S, layer: u32, frame: u32| {
        let d = s.doc(&id);
        copy_pixels(
            &d.sprite,
            &d.cels,
            FrameIndex::new(frame),
            CopySource::Layer(LayerId::new(layer)),
        )
        .expect("copy")
        .rgba
    };
    let file = |s: &S, path: &str| import_png(&s.host.files[path]).expect("png").rgba;
    assert_ne!(copy(&s, 1, 0), copy(&s, 0, 0));
    s.ok("export.selection", json!({ "path": "active.png" }));
    assert_eq!(file(&s, "active.png"), copy(&s, 1, 0), "the active layer 1");
    s.ok(
        "export.selection",
        json!({ "path": "zero.png", "layer": 0 }),
    );
    assert_eq!(file(&s, "zero.png"), copy(&s, 0, 0), "`layer` 0");
    // An active group: its rendered pixels, not the paint target's.
    s.ok("layer.add", json!({ "kind": "group" }));
    for layer in [0, 1] {
        s.ok(
            "layer.move",
            json!({ "layer": layer, "target": 2, "place": "into" }),
        );
    }
    assert_eq!(
        s.doc(&id).active_layer,
        Some(LayerId::new(2)),
        "the group is active"
    );
    let group = copy(&s, 2, 0);
    assert!(group != copy(&s, 0, 0) && group != copy(&s, 1, 0));
    let before = s.state(&id);
    s.ok("export.selection", json!({ "path": "group.png" }));
    assert_eq!(file(&s, "group.png"), group, "the active group");
    assert_eq!(s.state(&id), before);
    // A `frame` other than the active frame.
    s.ok("frame.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "layer": 1, "frame": 1, "pixels": [{ "x": 4, "y": 3, "color": "#abcdef" }] }),
    );
    assert_eq!(s.ok("doc.info", json!({}))["active_frame"], 1);
    assert_ne!(copy(&s, 1, 0), copy(&s, 1, 1));
    s.ok(
        "export.selection",
        json!({ "path": "f0.png", "layer": 1, "frame": 0 }),
    );
    assert_eq!(file(&s, "f0.png"), copy(&s, 1, 0), "`frame` 0");
    s.ok("export.selection", json!({ "path": "f1.png", "layer": 1 }));
    assert_eq!(file(&s, "f1.png"), copy(&s, 1, 1), "the active frame 1");

    // Image: `layer`, else the active layer whatever its kind.
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 4, "y0": 0, "x1": 7, "y1": 2, "fill": "#00ff00", "anti_alias": false }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 6, "height": 4 }),
    );
    let decoded = |s: &S, path: &str| decode(&s.host.files[path]).expect("png");
    assert_ne!(
        image_copy_of(&s, &id, 0).1.data(),
        image_copy_of(&s, &id, 1).1.data()
    );
    let before = s.image(&id).document().clone();
    s.ok(
        "export.selection",
        json!({ "path": "zero.png", "layer": 0 }),
    );
    assert_eq!(
        decoded(&s, "zero.png").data(),
        image_copy_of(&s, &id, 0).1.data(),
        "`layer` 0"
    );
    s.ok("export.selection", json!({ "path": "active.png" }));
    assert_eq!(
        decoded(&s, "active.png").data(),
        image_copy_of(&s, &id, 1).1.data(),
        "the active layer 1"
    );
    assert_eq!(
        s.image(&id).document(),
        &before,
        "the document is unchanged"
    );
    // An active group: the Image editor's Copy has no pixels to copy there
    // (`copy_selection` gives none), so the call fails naming the group.
    s.ok("layer.add", json!({ "kind": "group" }));
    let group = s.ok("doc.info", json!({}))["active_layer"]
        .as_u64()
        .expect("id") as u32;
    let mut d = s.image(&id).document().clone();
    d.set_active_layer(s.image(&id).layer_index(group).expect("group"))
        .expect("activate");
    assert!(
        copy_selection(&d, false).is_none(),
        "the editor copies nothing"
    );
    let before = s.image(&id).document().clone();
    let e = s.err("export.selection", json!({ "path": "group.png" }));
    assert!(
        e.contains(&format!("layer {group}")) && e.contains("is a group"),
        "{e}"
    );
    assert!(
        !s.host.files.contains_key("group.png"),
        "nothing is written"
    );
    assert_eq!(s.image(&id).document(), &before);
}

/// A file host whose writes fail after the path check passed (an I/O
/// error).
#[derive(Default)]
struct FailingWrites;

impl crate::FileHost for FailingWrites {
    fn read(&mut self, path: &str) -> Result<Vec<u8>, String> {
        Err(format!("{path}: no such file"))
    }

    fn write(&mut self, _path: &str, _bytes: &[u8]) -> Result<(), String> {
        Err("disk full".into())
    }

    fn check_write(&mut self, _path: &str) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn export_selection_cut_with_a_failing_write_fails_like_export_png() {
    for mode in ["pixel", "image"] {
        let mut ws = crate::Workspace::default();
        let mut host = FailingWrites;
        let mut call =
            |name: &str, args: Value| crate::call(&mut ws, &mut host, name, args).map(|o| o.json);
        call("doc.new", json!({ "mode": mode, "width": 4, "height": 4 })).expect("new");
        call("draw.fill", json!({ "x": 0, "y": 0, "color": "#ff0000" })).expect("fill");
        call(
            "select.rect",
            json!({ "x": 0, "y": 0, "width": 2, "height": 2 }),
        )
        .expect("select");
        let depth = call("doc.info", json!({})).expect("info")["undo_depth"].clone();
        let png = call("export.png", json!({ "path": "out.png" }))
            .expect_err("export.png fails")
            .to_string();
        let cut = call(
            "export.selection",
            json!({ "path": "out.png", "cut": true }),
        )
        .expect_err("export.selection fails")
        .to_string();
        assert!(
            cut.contains("`out.png`") && cut.contains("disk full"),
            "{mode}: {cut}"
        );
        assert_eq!(cut, png, "{mode}: fails as export.png does");
        assert_eq!(
            call("doc.info", json!({})).expect("info")["undo_depth"],
            depth,
            "{mode}: no cut step"
        );
        let pixel = call(
            "doc.read_pixels",
            json!({ "region": { "x": 0, "y": 0, "width": 1, "height": 1 } }),
        )
        .expect("read");
        assert!(
            pixel.to_string().to_lowercase().contains("ff0000"),
            "{mode}: the pixel stays: {pixel}"
        );
    }
}
