//! `layer.translate`, `select.move_content` and `select.move` (AC-02 … AC-08, AC-21).

use super::*;

#[test]
fn layer_translate_and_select_move_on_a_pixel_document_fail_naming_the_mode() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("select.all", json!({}));
    for tool in ["layer.translate", "select.move"] {
        let before = s.state(&id);
        let e = s.err(tool, json!({ "dx": 1, "dy": 0 }));
        assert!(e.contains("Pixel document"), "{tool}: {e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn layer_translate_equals_the_editors_move_tool() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let want = image_editor(
        &s,
        &id,
        CommandSpec::TranslateLayer {
            layer: 0,
            dx: 3,
            dy: -2,
        },
    );
    let out = s.ok("layer.translate", json!({ "dx": 3, "dy": -2 }));
    assert_eq!(out, json!({ "doc": id, "layer": 0, "changed": true }));
    assert_eq!(s.image(&id).document().layers(), want.layers());
    // A group moves its subtree.
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok(
        "layer.move",
        json!({ "layer": 0, "target": 1, "place": "into" }),
    );
    let g = s.image(&id).layer_index(1).expect("group");
    let want = image_editor(
        &s,
        &id,
        CommandSpec::TranslateLayer {
            layer: g,
            dx: -1,
            dy: 1,
        },
    );
    s.ok("layer.translate", json!({ "dx": -1, "dy": 1, "layer": 1 }));
    assert_eq!(s.image(&id).document().layers(), want.layers());
}

#[test]
fn select_move_content_equals_the_editors_move_selected_pixels() {
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
    let want = pixel_editor(
        &s,
        &id,
        MoveSelectionContent::new(LayerId::new(0), FrameIndex::new(0), 2, 1).into(),
    );
    let out = s.ok("select.move_content", json!({ "dx": 2, "dy": 1 }));
    assert_eq!(out["changed"], true);
    assert_eq!(s.state(&id), want);
    let rgba = s.cel_rgba(&id, 0, 0);
    assert_eq!(
        px(&rgba, 6, 2, 1),
        [0, 255, 0, 255],
        "pixel in the hole stays"
    );
    assert_eq!(px(&rgba, 6, 3, 2), [255, 0, 0, 255], "moved pixel");
    assert_eq!(px(&rgba, 6, 1, 1), [0, 0, 0, 0], "transparency behind");

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.ellipse",
        json!({ "x": 1, "y": 1, "width": 4, "height": 4 }),
    );
    let want = image_editor(
        &s,
        &id,
        CommandSpec::TranslateSelection {
            layer: 0,
            dx: 2,
            dy: 1,
        },
    );
    s.ok("select.move_content", json!({ "dx": 2, "dy": 1 }));
    assert_eq!(s.image(&id).document().layers(), want.layers());
    assert_eq!(s.image(&id).document().selection, want.selection);
}

#[test]
fn select_move_content_without_selection_follows_each_editor() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let want = image_editor(
        &s,
        &id,
        CommandSpec::TranslateSelection {
            layer: 0,
            dx: 1,
            dy: 1,
        },
    );
    s.ok("select.move_content", json!({ "dx": 1, "dy": 1 }));
    assert_eq!(s.image(&id).document().layers(), want.layers());

    let mut s = S::new();
    let id = pixel_doc(&mut s);
    let before = s.state(&id);
    let e = s.err("select.move_content", json!({ "dx": 1, "dy": 1 }));
    assert!(
        e.contains("needs a selection") && e.contains("select."),
        "{e}"
    );
    assert_eq!(s.state(&id), before);
}

#[test]
fn select_move_shifts_only_the_selection() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    let before = s.image(&id).document().clone();
    let depth = undo_depth(&mut s);
    let out = s.ok("select.move", json!({ "dx": 2, "dy": 0 }));
    assert_eq!(out["changed"], false, "no selection");
    assert_eq!(s.image(&id).document(), &before, "no selection: unchanged");
    assert_eq!(undo_depth(&mut s), depth, "no selection: no undo step");
    s.ok(
        "select.rect",
        json!({ "x": 5, "y": 1, "width": 3, "height": 2 }),
    );
    let want = image_editor(
        &s,
        &id,
        CommandSpec::TranslateSelectionMask { dx: 2, dy: 1 },
    );
    let layers = s.image(&id).document().layers().to_vec();
    let out = s.ok("select.move", json!({ "dx": 2, "dy": 1 }));
    assert_eq!(out["changed"], true);
    assert_eq!(s.image(&id).document().selection, want.selection);
    assert_eq!(s.image(&id).document().layers(), layers.as_slice());
    assert_eq!(out["selection"], json!({ "x": 7, "y": 2, "w": 1, "h": 2 }));
}

#[test]
fn moves_are_one_undo_step_restoring_pixels_and_selection() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    let before = s.state(&id);
    let depth = undo_depth(&mut s);
    s.ok("select.move_content", json!({ "dx": 1, "dy": 2 }));
    assert_eq!(undo_depth(&mut s), depth + 1);
    s.ok("doc.undo", json!({}));
    assert_eq!(s.state(&id), before, "pixels and selection restored");
    assert_eq!(
        s.ok("select.move_content", json!({ "dx": 0, "dy": 0 }))["changed"],
        false
    );
    assert_eq!(undo_depth(&mut s), depth);
    // Transparent selected pixels still move the selection: one step.
    s.ok(
        "select.rect",
        json!({ "x": 3, "y": 3, "width": 1, "height": 1 }),
    );
    let out = s.ok("select.move_content", json!({ "dx": 1, "dy": 0 }));
    assert_eq!(out["changed"], true);
    assert_eq!(undo_depth(&mut s), depth + 1);
    assert_eq!(
        s.ok("doc.info", json!({}))["selection"],
        json!({ "x": 4, "y": 3, "w": 1, "h": 1 })
    );

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 3, "height": 3 }),
    );
    let doc_before = s.image(&id).document().clone();
    for (tool, args) in [
        ("layer.translate", json!({ "dx": 1, "dy": 0 })),
        ("select.move_content", json!({ "dx": 0, "dy": 1 })),
        ("select.move", json!({ "dx": 1, "dy": 1 })),
    ] {
        let depth = undo_depth(&mut s);
        assert_eq!(s.ok(tool, args.clone())["changed"], true, "{tool}");
        assert_eq!(undo_depth(&mut s), depth + 1, "{tool}");
        assert_eq!(
            s.ok(tool, json!({ "dx": 0, "dy": 0 }))["changed"],
            false,
            "{tool}"
        );
        assert_eq!(undo_depth(&mut s), depth + 1, "{tool} zero offset");
    }
    for _ in 0..3 {
        s.ok("doc.undo", json!({}));
    }
    assert_eq!(s.image(&id).document(), &doc_before);
    s.ok("layer.add", json!({}));
    let depth = undo_depth(&mut s);
    assert_eq!(
        s.ok("layer.translate", json!({ "dx": 2, "dy": 2 }))["changed"],
        false,
        "transparent layer"
    );
    assert_eq!(undo_depth(&mut s), depth);
    // Image: transparent selected pixels still move the selection, one
    // step that undo takes back.
    s.ok(
        "select.rect",
        json!({ "x": 6, "y": 0, "width": 2, "height": 1 }),
    );
    let before = s.image(&id).document().clone();
    let depth = undo_depth(&mut s);
    let out = s.ok("select.move_content", json!({ "dx": -1, "dy": 0 }));
    assert_eq!(out["changed"], true);
    assert_eq!(undo_depth(&mut s), depth + 1);
    assert_eq!(
        s.ok("doc.info", json!({}))["selection"],
        json!({ "x": 5, "y": 0, "w": 2, "h": 1 })
    );
    s.ok("doc.undo", json!({}));
    assert_eq!(s.image(&id).document(), &before);
}

#[test]
fn moves_on_locked_group_or_tilemap_layers_fail_with_the_reason() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("select.all", json!({}));
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
    for (layer, reason) in [(0, "locked"), (1, "a group"), (7, "a tilemap layer")] {
        let before = s.state(&id);
        let e = s.err(
            "select.move_content",
            json!({ "dx": 1, "dy": 0, "layer": layer }),
        );
        assert!(
            e.contains(reason) && e.contains(&format!("layer {layer}")),
            "{e}"
        );
        assert_eq!(s.state(&id), before);
    }
    // A layer inside a locked group.
    s.ok("layer.set", json!({ "layer": 0, "locked": false }));
    s.ok(
        "layer.move",
        json!({ "layer": 0, "target": 1, "place": "into" }),
    );
    s.ok("layer.set", json!({ "layer": 1, "locked": true }));
    let before = s.state(&id);
    let e = s.err(
        "select.move_content",
        json!({ "dx": 1, "dy": 0, "layer": 0 }),
    );
    assert!(e.contains("locked") && e.contains("layer 0"), "{e}");
    assert_eq!(s.state(&id), before);

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok(
        "layer.move",
        json!({ "layer": 0, "target": 1, "place": "into" }),
    );
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    // (tool, target layer, reason, the layer the message names)
    for (tool, layer, reason, named) in [
        ("layer.translate", 0, "locked", 0),
        ("layer.translate", 1, "locked", 0),
        ("select.move_content", 0, "locked", 0),
        ("select.move_content", 1, "group", 1),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, json!({ "dx": 1, "dy": 0, "layer": layer }));
        assert!(
            e.contains(reason) && e.contains(&format!("layer {named}")),
            "{tool} {layer}: {e}"
        );
        assert_eq!(s.image(&id).document(), &before);
    }
    // A layer inside a locked group.
    s.ok("layer.set", json!({ "layer": 0, "locked": false }));
    s.ok("layer.set", json!({ "layer": 1, "locked": true }));
    for tool in ["layer.translate", "select.move_content"] {
        for (dx, dy) in [(1, 0), (0, 0)] {
            // A zero offset is refused as well: the lock decides first.
            let before = s.image(&id).document().clone();
            let e = s.err(tool, json!({ "dx": dx, "dy": dy, "layer": 0 }));
            assert!(
                e.contains("locked") && e.contains("layer 0"),
                "{tool} ({dx}, {dy}): {e}"
            );
            assert_eq!(s.image(&id).document(), &before);
        }
    }
}

#[test]
fn moves_reach_other_layers_frames_and_beyond_the_canvas() {
    // AC-03: `layer` other than the active layer, and a group move is one
    // undo step.
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({}));
    let want = image_editor(
        &s,
        &id,
        CommandSpec::TranslateLayer {
            layer: 0,
            dx: 1,
            dy: 0,
        },
    );
    let out = s.ok("layer.translate", json!({ "dx": 1, "dy": 0, "layer": 0 }));
    assert_eq!(out, json!({ "doc": id, "layer": 0, "changed": true }));
    assert_eq!(s.image(&id).document().layers(), want.layers());

    // AC-04: every selected pixel moved off the canvas is gone; one undo
    // restores pixels and selection, in both modes.
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    let before = s.state(&id);
    let depth = undo_depth(&mut s);
    let want = pixel_editor(
        &s,
        &id,
        MoveSelectionContent::new(LayerId::new(0), FrameIndex::new(0), 10, 0).into(),
    );
    s.ok("select.move_content", json!({ "dx": 10, "dy": 0 }));
    assert_eq!(s.state(&id), want);
    assert_eq!(px(&s.cel_rgba(&id, 0, 0), 6, 1, 1), [0, 0, 0, 0]);
    assert_eq!(undo_depth(&mut s), depth + 1);
    s.ok("doc.undo", json!({}));
    assert_eq!(s.state(&id), before);

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    let before = s.image(&id).document().clone();
    let want = image_editor(
        &s,
        &id,
        CommandSpec::TranslateSelection {
            layer: 0,
            dx: 10,
            dy: 0,
        },
    );
    s.ok("select.move_content", json!({ "dx": 10, "dy": 0 }));
    assert_eq!(s.image(&id).document().layers(), want.layers());
    assert_eq!(s.image(&id).document().selection, want.selection);
    s.ok("doc.undo", json!({}));
    assert_eq!(s.image(&id).document(), &before);

    // AC-15: Copy Merged at a frame other than the active one.
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("frame.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#123456" }] }),
    );
    let d = s.doc(&id);
    let want =
        copy_pixels(&d.sprite, &d.cels, FrameIndex::new(0), CopySource::Merged).expect("copy");
    let other =
        copy_pixels(&d.sprite, &d.cels, FrameIndex::new(1), CopySource::Merged).expect("copy");
    assert_ne!(want.rgba, other.rgba);
    s.ok(
        "export.selection",
        json!({ "path": "m.png", "merged": true, "frame": 0 }),
    );
    assert_eq!(
        import_png(&s.host.files["m.png"]).expect("png").rgba,
        want.rgba
    );
}

#[test]
fn a_refusing_move_command_fails_with_its_reason() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("select.all", json!({}));
    {
        let d = s.ws.get_mut(&id).expect("doc");
        let cel = d
            .cels
            .get_mut(LayerId::new(0), FrameIndex::new(0))
            .expect("cel");
        if let CelData::Image(buf) = &mut cel.data {
            buf.color_mode = ColorMode::Indexed {
                transparent_index: 0,
            };
        }
    }
    let before = s.state(&id);
    let e = s.err("select.move_content", json!({ "dx": 1, "dy": 0 }));
    assert!(e.contains("color mode"), "{e}");
    assert_eq!(s.state(&id), before);
}

#[test]
fn moves_and_clipboard_report_values() {
    // `select.move` reports the moved bounds (Value shape check).
    let mut s = S::new();
    s.new_image(4, 4);
    s.ok(
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 2, "height": 2 }),
    );
    let out: Value = s.ok("select.move", json!({ "dx": 1, "dy": 1 }));
    assert_eq!(out["selection"], json!({ "x": 1, "y": 1, "w": 2, "h": 2 }));
}
#[test]
fn select_move_content_moves_the_target_layer_at_the_target_frame() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok("frame.add", json!({}));
    let info = s.ok("doc.info", json!({}));
    assert_eq!(
        (info["active_layer"].clone(), info["active_frame"].clone()),
        (json!(1), json!(1))
    );
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    let want = pixel_editor(
        &s,
        &id,
        MoveSelectionContent::new(LayerId::new(0), FrameIndex::new(0), 1, 2).into(),
    );
    let out = s.ok(
        "select.move_content",
        json!({ "dx": 1, "dy": 2, "layer": 0, "frame": 0 }),
    );
    assert_eq!(
        (out["layer"].clone(), out["changed"].clone()),
        (json!(0), json!(true))
    );
    assert_eq!(s.state(&id), want);
    assert_eq!(
        px(&s.cel_rgba(&id, 0, 0), 6, 2, 3),
        [255, 0, 0, 255],
        "moved on layer 0, frame 0"
    );

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 3, "height": 3 }),
    );
    let want = image_editor(
        &s,
        &id,
        CommandSpec::TranslateSelection {
            layer: 0,
            dx: 2,
            dy: 1,
        },
    );
    s.ok(
        "select.move_content",
        json!({ "dx": 2, "dy": 1, "layer": 0 }),
    );
    assert_eq!(s.image(&id).document().layers(), want.layers());
    assert_eq!(s.image(&id).document().selection, want.selection);
}
