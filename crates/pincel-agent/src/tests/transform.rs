//! Canvas and layer transforms in both modes (mcp-parity-selection-transform
//! AC-24 … AC-34, AC-36, AC-38).

use pincel_core::{
    Anchor, AnyCommand, Bus, CelMap, FrameIndex, Interpolation, Layer, LayerId, Orientation, Rect,
    ReframeCanvas, ScaleImage, Sprite, TilesetId, TransformCanvas, TransformCel,
};
use pincel_image_core::Document;
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use serde_json::{Value, json};

use super::S;

fn pixel_editor(s: &S, id: &str, cmd: AnyCommand) -> (Sprite, CelMap) {
    let (mut sprite, mut cels) = s.state(id);
    Bus::new()
        .execute(cmd, &mut sprite, &mut cels)
        .expect("editor command");
    (sprite, cels)
}

fn image_editor(s: &S, id: &str, spec: CommandSpec) -> Document {
    let mut bus = CommandBus::new(s.image(id).document().clone());
    run_command(&mut bus, spec, &mut StrokeScratch::default(), &[]).expect("editor");
    bus.document
}

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// A 6×4 Pixel document with an asymmetric drawing in two frames and a
/// slice.
fn pixel_doc(s: &mut S) -> String {
    let id = s.new_doc(6, 4);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }, { "x": 5, "y": 1, "color": "#00ff00" }, { "x": 2, "y": 3, "color": "#0000ff" }] }),
    );
    s.ok("frame.add", json!({ "clone_from": 0 }));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 2, "color": "#ffff00" }], "frame": 1 }),
    );
    s.ws.get_mut(&id)
        .expect("doc")
        .sprite
        .slices
        .push(pincel_core::Slice {
            id: pincel_core::SliceId::new(0),
            name: "s".into(),
            color: pincel_core::Rgba::new(0, 0, 255, 255),
            keys: vec![pincel_core::SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::new(1, 0, 2, 3),
                center: None,
                pivot: Some((1, 1)),
            }],
            user_data: None,
        });
    id
}

/// A 6×4 Image document with an asymmetric drawing.
fn image_doc(s: &mut S) -> String {
    let id = s.new_image(6, 4);
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 2, "y1": 1, "fill": "#ff0000", "anti_alias": false }),
    );
    id
}

#[test]
fn pixel_flip_and_rotate_equal_the_editors_canvas_commands() {
    for (tool, args, op) in [
        (
            "image.flip",
            json!({ "direction": "horizontal" }),
            Orientation::FlipHorizontal,
        ),
        (
            "image.flip",
            json!({ "direction": "vertical" }),
            Orientation::FlipVertical,
        ),
        (
            "image.rotate",
            json!({ "angle": 90 }),
            Orientation::Rotate90Cw,
        ),
        (
            "image.rotate",
            json!({ "angle": 180 }),
            Orientation::Rotate180,
        ),
        (
            "image.rotate",
            json!({ "angle": 270 }),
            Orientation::Rotate90Ccw,
        ),
    ] {
        let mut s = S::new();
        let id = pixel_doc(&mut s);
        let want = pixel_editor(&s, &id, TransformCanvas::new(op).into());
        let out = s.ok(tool, args.clone());
        assert_eq!(s.state(&id), want, "{tool} {args}");
        assert_eq!(out["width"], want.0.width);
        assert_eq!(out["height"], want.0.height);
        assert_eq!(out["changed"], true);
        if op.swaps_axes() {
            assert_eq!((want.0.width, want.0.height), (4, 6));
        }
    }
}

#[test]
fn pixel_resize_and_scale_equal_the_editors_commands() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    let anchor = Anchor::from_name("bottom").expect("anchor");
    let want = pixel_editor(&s, &id, ReframeCanvas::resize(6, 4, 9, 7, anchor).into());
    let out = s.ok(
        "canvas.resize",
        json!({ "width": 9, "height": 7, "anchor": "bottom_center" }),
    );
    assert_eq!(
        out,
        json!({ "doc": id, "width": 9, "height": 7, "changed": true })
    );
    assert_eq!(s.state(&id), want);
    let want = pixel_editor(
        &s,
        &id,
        ScaleImage::new(18, 7, Interpolation::Bicubic).into(),
    );
    s.ok(
        "image.scale",
        json!({ "width": 18, "height": 7, "interpolation": "bicubic" }),
    );
    assert_eq!(s.state(&id), want);
}

#[test]
fn resize_and_scale_defaults_follow_each_editor() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    let center = Anchor::from_name("center").expect("anchor");
    let want = pixel_editor(&s, &id, ReframeCanvas::resize(6, 4, 10, 8, center).into());
    s.ok("canvas.resize", json!({ "width": 10, "height": 8 }));
    assert_eq!(s.state(&id), want);
    let want = pixel_editor(
        &s,
        &id,
        ScaleImage::new(20, 16, Interpolation::Nearest).into(),
    );
    s.ok("image.scale", json!({ "width": 20, "height": 16 }));
    assert_eq!(s.state(&id), want);

    let mut s = S::new();
    let id = image_doc(&mut s);
    let want = image_editor(
        &s,
        &id,
        CommandSpec::ResizeCanvas {
            width: 9,
            height: 5,
            anchor: "top_left".into(),
        },
    );
    s.ok("canvas.resize", json!({ "width": 9, "height": 5 }));
    assert_eq!(s.image(&id).document(), &want);
    let want = image_editor(
        &s,
        &id,
        CommandSpec::ScaleImage {
            width: 5,
            height: 3,
            interpolation: "bilinear".into(),
        },
    );
    s.ok("image.scale", json!({ "width": 5, "height": 3 }));
    assert_eq!(s.image(&id).document(), &want);
    // Every anchor name works in both modes.
    for anchor in [
        "top_left",
        "top_center",
        "top_right",
        "center_left",
        "center",
        "center_right",
        "bottom_left",
        "bottom_center",
        "bottom_right",
    ] {
        s.ok(
            "canvas.resize",
            json!({ "width": 7, "height": 7, "anchor": anchor }),
        );
        let mut p = S::new();
        p.new_doc(4, 4);
        p.ok(
            "canvas.resize",
            json!({ "width": 7, "height": 7, "anchor": anchor }),
        );
    }
}

#[test]
fn pixel_crop_to_selection_crops_to_the_clipped_bounds() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 3, "y": -2, "width": 9, "height": 4 }),
    );
    let want = pixel_editor(&s, &id, ReframeCanvas::new(Rect::new(3, 0, 3, 2)).into());
    let out = s.ok("image.crop_to_selection", json!({}));
    assert_eq!(
        (out["width"].clone(), out["height"].clone()),
        (json!(3), json!(2))
    );
    assert_eq!(s.state(&id), want);
}

#[test]
fn pixel_crop_without_selection_fails_and_changes_nothing() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    let before = s.state(&id);
    let e = s.err("image.crop_to_selection", json!({}));
    assert!(e.contains("needs a selection"), "{e}");
    assert_eq!(s.state(&id), before);
}

#[test]
fn pixel_sizes_outside_1_to_4096_fail() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    for (tool, args, msg) in [
        (
            "canvas.resize",
            json!({ "width": 0, "height": 4 }),
            "width must be 1–4096",
        ),
        (
            "canvas.resize",
            json!({ "width": 4, "height": 4097 }),
            "height must be 1–4096",
        ),
        (
            "image.scale",
            json!({ "width": 5000, "height": 4 }),
            "width must be 1–4096",
        ),
        (
            "image.scale",
            json!({ "width": 4, "height": -1 }),
            "height must be 1–4096",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args);
        assert!(e.contains(msg), "{tool}: {e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn pixel_transforms_refuse_tilemap_documents() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ws.get_mut(&id)
        .expect("doc")
        .sprite
        .layers
        .push(Layer::tilemap(LayerId::new(5), "Map", TilesetId::new(0)));
    for (tool, args) in [
        ("image.flip", json!({ "direction": "vertical" })),
        ("image.rotate", json!({ "angle": 90 })),
        ("image.scale", json!({ "width": 3, "height": 3 })),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args);
        assert!(
            e.contains("tilemap layers cannot be transformed"),
            "{tool}: {e}"
        );
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn layer_flip_and_rotate_equal_the_editors_layer_commands() {
    for (tool, args, op) in [
        (
            "layer.flip",
            json!({ "direction": "horizontal" }),
            Orientation::FlipHorizontal,
        ),
        (
            "layer.flip",
            json!({ "direction": "vertical", "frame": 1 }),
            Orientation::FlipVertical,
        ),
        (
            "layer.rotate",
            json!({ "angle": 90 }),
            Orientation::Rotate90Cw,
        ),
        (
            "layer.rotate",
            json!({ "angle": 180, "frame": 1 }),
            Orientation::Rotate180,
        ),
        (
            "layer.rotate",
            json!({ "angle": 270 }),
            Orientation::Rotate90Ccw,
        ),
    ] {
        let mut s = S::new();
        let id = pixel_doc(&mut s);
        // Without `frame`, the active frame (frame.add made frame 1 active).
        let frame = args["frame"]
            .as_u64()
            .map_or(s.doc(&id).active_frame, |f| FrameIndex::new(f as u32));
        let want = pixel_editor(
            &s,
            &id,
            TransformCel::new(LayerId::new(0), frame, op).into(),
        );
        let out = s.ok(tool, args.clone());
        assert_eq!(
            out,
            json!({ "doc": id, "layer": 0, "changed": true }),
            "{tool} {args}"
        );
        assert_eq!(s.state(&id), want, "{tool} {args}");
        assert_eq!((s.doc(&id).sprite.width, s.doc(&id).sprite.height), (6, 4));
    }

    for (tool, args, spec) in [
        (
            "layer.flip",
            json!({ "direction": "horizontal" }),
            r#"{"type":"transform_layer","layer":1,"op":"flip_h"}"#,
        ),
        (
            "layer.flip",
            json!({ "direction": "vertical" }),
            r#"{"type":"transform_layer","layer":1,"op":"flip_v"}"#,
        ),
        (
            "layer.rotate",
            json!({ "angle": 90 }),
            r#"{"type":"rotate_layer_90","layer":1,"ccw":false}"#,
        ),
        (
            "layer.rotate",
            json!({ "angle": 180 }),
            r#"{"type":"transform_layer","layer":1,"op":"rotate_180"}"#,
        ),
        (
            "layer.rotate",
            json!({ "angle": 270 }),
            r#"{"type":"rotate_layer_90","layer":1,"ccw":true}"#,
        ),
    ] {
        // A group (id 1) turns its whole subtree.
        let mut s = S::new();
        let id = image_doc(&mut s);
        s.ok("layer.add", json!({ "kind": "group" }));
        s.ok(
            "layer.move",
            json!({ "layer": 0, "target": 1, "place": "into" }),
        );
        let mut a = args.clone();
        a["layer"] = json!(1);
        let spec: CommandSpec = serde_json::from_str(spec).expect("spec");
        let want = image_editor(&s, &id, spec);
        let out = s.ok(tool, a);
        assert_eq!(out["changed"], true);
        assert_eq!(
            s.image(&id).document().layers(),
            want.layers(),
            "{tool} {args}"
        );
        assert_eq!(s.image(&id).width(), 6);
    }
}

#[test]
fn layer_transforms_follow_each_editors_selection_rule() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 3, "height": 2 }),
    );
    let cmd = TransformCel::new(
        LayerId::new(0),
        FrameIndex::new(1),
        Orientation::FlipHorizontal,
    )
    .within(Rect::new(0, 0, 3, 2));
    let want = pixel_editor(&s, &id, cmd.into());
    s.ok("layer.flip", json!({ "direction": "horizontal" }));
    assert_eq!(s.state(&id), want);
    // Outside the bounds nothing moved.
    assert_eq!(super::px(&s.cel_rgba(&id, 0, 1), 6, 5, 1), [0, 255, 0, 255]);

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 1, "height": 1 }),
    );
    let want = image_editor(
        &s,
        &id,
        serde_json::from_str(r#"{"type":"rotate_layer_90","layer":0,"ccw":false}"#).expect("spec"),
    );
    s.ok("layer.rotate", json!({ "angle": 90 }));
    assert_eq!(s.image(&id).document().layers(), want.layers());
}

#[test]
fn layer_transforms_refuse_locked_group_and_tilemap_targets() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    s.ws.get_mut(&id)
        .expect("doc")
        .sprite
        .layers
        .push(Layer::tilemap(LayerId::new(5), "Map", TilesetId::new(0)));
    for (layer, reason) in [(0, "locked"), (1, "a group"), (5, "a tilemap layer")] {
        for (tool, args) in [
            (
                "layer.flip",
                json!({ "direction": "horizontal", "layer": layer }),
            ),
            ("layer.rotate", json!({ "angle": 90, "layer": layer })),
        ] {
            let before = s.state(&id);
            let e = s.err(tool, args);
            assert!(
                e.contains(reason) && e.contains(&format!("layer {layer}")),
                "{tool}: {e}"
            );
            assert_eq!(s.state(&id), before);
        }
    }

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok(
        "layer.move",
        json!({ "layer": 0, "target": 1, "place": "into" }),
    );
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    for layer in [0, 1] {
        let before = s.image(&id).document().clone();
        let e = s.err(
            "layer.flip",
            json!({ "direction": "vertical", "layer": layer }),
        );
        assert!(e.contains("locked") && e.contains("layer 0"), "{e}");
        assert_eq!(s.image(&id).document(), &before);
    }
}

#[test]
fn layer_transforms_naming_a_missing_layer_or_frame_fail() {
    let mut s = S::new();
    pixel_doc(&mut s);
    let e = s.err(
        "layer.flip",
        json!({ "direction": "horizontal", "layer": 9 }),
    );
    assert!(e.contains("`layer`") && e.contains('9'), "{e}");
    let e = s.err("layer.rotate", json!({ "angle": 90, "frame": 9 }));
    assert!(e.contains("frame 9"), "{e}");
    let mut s = S::new();
    image_doc(&mut s);
    let e = s.err("layer.rotate", json!({ "angle": 90, "layer": 9 }));
    assert!(e.contains("`layer`") && e.contains('9'), "{e}");
}

#[test]
fn transforms_record_one_undo_step_and_none_without_a_change() {
    /// A full layer with an asymmetric mark and a selection over part of
    /// it, so every transform changes the document.
    fn fresh(image: bool) -> S {
        let mut s = S::new();
        if image {
            s.new_image(6, 4);
            s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#808080" }));
            s.ok(
                "draw.shape",
                json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 2, "y1": 1, "fill": "#ff0000", "anti_alias": false }),
            );
        } else {
            s.new_doc(6, 4);
            s.ok(
                "draw.rect",
                json!({ "x0": 0, "y0": 0, "x1": 5, "y1": 3, "color": "#808080", "filled": true }),
            );
            s.ok(
                "draw.rect",
                json!({ "x0": 0, "y0": 0, "x1": 2, "y1": 1, "color": "#ff0000", "filled": true }),
            );
        }
        s.ok(
            "select.rect",
            json!({ "x": 0, "y": 0, "width": 4, "height": 3 }),
        );
        s
    }
    for image in [false, true] {
        for (tool, args) in [
            ("layer.flip", json!({ "direction": "horizontal" })),
            ("layer.rotate", json!({ "angle": 180 })),
            ("select.delete", json!({})),
            ("image.crop_to_selection", json!({})),
            ("canvas.resize", json!({ "width": 8, "height": 8 })),
            ("image.scale", json!({ "width": 4, "height": 4 })),
            ("image.flip", json!({ "direction": "vertical" })),
            ("image.rotate", json!({ "angle": 90 })),
        ] {
            let mut s = fresh(image);
            let depth = undo_depth(&mut s);
            let out = s.ok(tool, args.clone());
            assert_eq!(out["changed"], true, "image={image} {tool}");
            assert_eq!(undo_depth(&mut s), depth + 1, "image={image} {tool}");
        }
        let mut s = fresh(image);
        s.ok("select.clear", json!({}));
        let depth = undo_depth(&mut s);
        let out = s.ok("canvas.resize", json!({ "width": 6, "height": 4 }));
        assert_eq!(out["changed"], false, "image={image}");
        assert_eq!(undo_depth(&mut s), depth);
        s.ok("layer.add", json!({}));
        let depth = undo_depth(&mut s);
        let out = s.ok("layer.flip", json!({ "direction": "horizontal" }));
        assert_eq!(out["changed"], false, "image={image}: empty layer");
        assert_eq!(undo_depth(&mut s), depth);
    }
}

#[test]
fn pixel_canvas_transforms_clear_the_selection_and_undo_restores_it() {
    for (tool, args) in [
        ("canvas.resize", json!({ "width": 8, "height": 8 })),
        ("image.scale", json!({ "width": 3, "height": 2 })),
        ("image.crop_to_selection", json!({})),
        ("image.flip", json!({ "direction": "horizontal" })),
        ("image.rotate", json!({ "angle": 270 })),
    ] {
        let mut s = S::new();
        let id = pixel_doc(&mut s);
        s.ok(
            "select.ellipse",
            json!({ "x": 0, "y": 0, "width": 5, "height": 4 }),
        );
        let before = s.state(&id);
        s.ok(tool, args);
        assert!(!s.doc(&id).sprite.has_selection(), "{tool}");
        assert_eq!(s.ok("doc.info", json!({}))["selection"], Value::Null);
        s.ok("doc.undo", json!({}));
        assert_eq!(s.state(&id), before, "{tool}");
    }
}

#[test]
fn bad_transform_arguments_fail_naming_the_argument() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            image_doc(&mut s);
        } else {
            pixel_doc(&mut s);
        }
        for (tool, args, arg) in [
            (
                "image.flip",
                json!({ "direction": "diagonal" }),
                "direction",
            ),
            ("layer.flip", json!({ "direction": "up" }), "direction"),
            ("image.rotate", json!({ "angle": 45 }), "angle"),
            ("layer.rotate", json!({ "angle": 360 }), "angle"),
            (
                "canvas.resize",
                json!({ "width": 4, "height": 4, "anchor": "middle" }),
                "anchor",
            ),
            (
                "image.scale",
                json!({ "width": 4, "height": 4, "interpolation": "lanczos" }),
                "interpolation",
            ),
            ("canvas.resize", json!({ "width": 0, "height": 4 }), "width"),
        ] {
            let before = s.ok("doc.info", json!({}));
            let e = s.err(tool, args.clone());
            assert!(e.contains(arg), "image={image} {tool} {args}: {e}");
            assert_eq!(s.ok("doc.info", json!({})), before);
        }
    }
}
