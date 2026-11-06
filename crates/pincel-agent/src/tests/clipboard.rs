//! Moving content and the headless clipboard (mcp-parity-selection-transform-content
//! AC-01 … AC-22).

use pincel_core::{
    AnyCommand, Bus, CelData, CelMap, ColorMode, CopySource, FrameIndex, Layer, LayerId,
    MoveSelectionContent, PasteLayer, PastedImage, Sprite, copy_pixels, import_png,
};
use pincel_image_core::Document;
use pincel_image_core::codec::{decode, to_bmp_bytes, to_jpeg_bytes, to_png_bytes};
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{ImageBuffer, copy_selection};
use serde_json::{Value, json};

use super::{S, px};

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

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

/// A Pixel document with a few colored pixels on layer 0, frame 0.
fn pixel_doc(s: &mut S) -> String {
    let id = s.new_doc(6, 5);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [
            { "x": 1, "y": 1, "color": "#ff0000" },
            { "x": 2, "y": 1, "color": "#00ff00" },
            { "x": 2, "y": 2, "color": "#0000ff80" },
            { "x": 5, "y": 4, "color": "#ffffff" },
        ] }),
    );
    id
}

/// An Image document with a painted layer 0.
fn image_doc(s: &mut S) -> String {
    let id = s.new_image(8, 6);
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 1, "y0": 1, "x1": 5, "y1": 4, "fill": "#3366cc", "anti_alias": false }),
    );
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 2, "y0": 2, "x1": 3, "y1": 3, "fill": "#ff000080", "anti_alias": false }),
    );
    id
}

fn png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let buf = ImageBuffer::from_raw(w, h, rgba.to_vec()).expect("buffer");
    to_png_bytes(&buf, 1).expect("png")
}

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
    let out = s.ok("select.move", json!({ "dx": 2, "dy": 0 }));
    assert_eq!(out["changed"], false, "no selection");
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
        let before = s.image(&id).document().clone();
        let e = s.err(tool, json!({ "dx": 1, "dy": 0, "layer": 0 }));
        assert!(e.contains("locked") && e.contains("layer 0"), "{tool}: {e}");
        assert_eq!(s.image(&id).document(), &before);
    }
}

#[test]
fn layer_import_pastes_above_the_active_layer_like_the_editor() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({ "name": "Top" }));
    s.ok("layer.set", json!({ "layer": 0, "name": "Base" }));
    // The duplicate (above Base, below Top) becomes the active layer.
    s.ok("layer.duplicate", json!({ "layer": 0 }));
    s.host
        .files
        .insert("a.png".into(), png(2, 1, &[255, 0, 0, 255, 0, 255, 0, 128]));
    let depth = undo_depth(&mut s);
    let out = s.ok("layer.import", json!({ "path": "a.png", "x": 3, "y": 2 }));
    assert_eq!(undo_depth(&mut s), depth + 1);
    let new = out["layer"].as_u64().expect("id") as u32;
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    let names: Vec<&str> = list
        .as_array()
        .expect("l")
        .iter()
        .map(|l| l["name"].as_str().expect("n"))
        .collect();
    assert_eq!(names, ["Base", "Base copy", "Layer 4", "Top"]);
    assert_eq!(s.ok("doc.info", json!({}))["active_layer"], new);
    let rgba = s.cel_rgba(&id, new, 0);
    assert_eq!(px(&rgba, 6, 3, 2), [255, 0, 0, 255]);
    assert_eq!(px(&rgba, 6, 4, 2), [0, 255, 0, 128]);

    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.host
        .files
        .insert("a.png".into(), png(2, 1, &[255, 0, 0, 255, 0, 255, 0, 128]));
    let want = {
        let mut bus = CommandBus::new(s.image(&id).document().clone());
        let buf = decode(&s.host.files["a.png"]).expect("decode");
        bus.apply(Box::new(pincel_image_core::command::PasteLayer::at(
            buf, 3, 2, false,
        )))
        .expect("paste");
        bus.document
    };
    let out = s.ok("layer.import", json!({ "path": "a.png", "x": 3, "y": 2 }));
    assert_eq!(out["layer"], s.ok("doc.info", json!({}))["active_layer"]);
    let view = |d: &Document| -> Vec<(String, Vec<u8>)> {
        d.layers()
            .iter()
            .map(|l| (l.name.clone(), l.pixels.data().to_vec()))
            .collect()
    };
    assert_eq!(view(s.image(&id).document()), view(&want));
    assert_eq!(s.image(&id).document().layers()[2].name, "Layer 3");
}

#[test]
fn layer_import_lands_on_the_target_frame() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok("frame.add", json!({}));
    s.host
        .files
        .insert("a.png".into(), png(1, 1, &[255, 0, 0, 255]));
    for frame in [0, 1] {
        let new = s.ok("layer.import", json!({ "path": "a.png", "frame": frame }))["layer"]
            .as_u64()
            .expect("id") as u32;
        let other = 1 - frame;
        assert_eq!(px(&s.cel_rgba(&id, new, frame), 4, 0, 0), [255, 0, 0, 255]);
        assert!(
            s.doc(&id)
                .cels
                .get(LayerId::new(new), FrameIndex::new(other))
                .is_none(),
            "nothing lands on frame {other}"
        );
    }
    // Without `frame`: the active frame.
    let active = s.ok("doc.info", json!({}))["active_frame"]
        .as_u64()
        .expect("active frame") as u32;
    let new = s.ok("layer.import", json!({ "path": "a.png" }))["layer"]
        .as_u64()
        .expect("id") as u32;
    assert_eq!(px(&s.cel_rgba(&id, new, active), 4, 0, 0), [255, 0, 0, 255]);
    assert!(
        s.doc(&id)
            .cels
            .get(LayerId::new(new), FrameIndex::new(1 - active))
            .is_none()
    );
}

#[test]
fn layer_import_expands_or_crops_like_the_editors_paste() {
    // A 3×3 image whose pixel (i, j) is [10i + 1, 10j + 1, 7, 255].
    let src: Vec<u8> = (0..3u8)
        .flat_map(|j| (0..3u8).flat_map(move |i| [10 * i + 1, 10 * j + 1, 7, 255]))
        .collect();
    let bytes = png(3, 3, &src);
    let img = |i: u8, j: u8| [10 * i + 1, 10 * j + 1, 7, 255];
    // (x, y, expand, canvas after, the image pixels that land on the canvas
    // as (canvas x, canvas y, image i, image j))
    type Case = (i32, i32, bool, (u32, u32), &'static [(u32, u32, u8, u8)]);
    let cases: [Case; 4] = [
        // Grows right and down to cover the image.
        (
            2,
            3,
            true,
            (5, 6),
            &[(2, 3, 0, 0), (4, 5, 2, 2), (3, 4, 1, 1)],
        ),
        // Negative offsets are cut off even with expand.
        (-2, -1, true, (5, 6), &[(0, 0, 2, 1), (0, 1, 2, 2)]),
        // Without expand: cropped at the right and bottom edges.
        (4, 4, false, (5, 6), &[(4, 4, 0, 0), (4, 5, 0, 1)]),
        // Wholly outside: an empty layer.
        (50, 50, false, (5, 6), &[]),
    ];

    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.host.files.insert("b.png".into(), bytes.clone());
    for (x, y, expand, (w, h), landed) in cases {
        let want = {
            let d = s.doc(&id);
            let new_id = d
                .sprite
                .layers
                .iter()
                .map(|l| l.id.0)
                .max()
                .map_or(0, |m| m + 1);
            let name = format!("Layer {}", d.sprite.layers.len() + 1);
            let png = import_png(&bytes).expect("png");
            let image = PastedImage {
                width: png.width,
                height: png.height,
                rgba: png.rgba,
            };
            let cmd = PasteLayer::new(
                Layer::image(LayerId::new(new_id), name),
                d.active_frame,
                image,
            )
            .at(x, y)
            .above(d.active_layer)
            .expanding(expand);
            pixel_editor(&s, &id, cmd.into())
        };
        let out = s.ok(
            "layer.import",
            json!({ "path": "b.png", "x": x, "y": y, "expand": expand }),
        );
        assert_eq!(
            (out["width"].clone(), out["height"].clone()),
            (json!(w), json!(h))
        );
        assert_eq!(
            s.state(&id),
            want,
            "Pixel paste at ({x}, {y}), expand {expand}"
        );
        let new = out["layer"].as_u64().expect("id") as u32;
        let rgba = s.cel_rgba(&id, new, 0);
        for &(cx, cy, i, j) in landed {
            assert_eq!(
                px(&rgba, w, cx, cy),
                img(i, j),
                "({x}, {y}) at ({cx}, {cy})"
            );
        }
        let opaque = rgba.iter().skip(3).step_by(4).filter(|&&a| a != 0).count();
        let visible =
            (x.max(0)..(x + 3).min(w as i32)).len() * (y.max(0)..(y + 3).min(h as i32)).len();
        assert_eq!(
            opaque, visible,
            "Pixel paste at ({x}, {y}): every other pixel is dropped"
        );
    }

    let mut s = S::new();
    let id = s.new_image(4, 4);
    s.host.files.insert("b.png".into(), bytes.clone());
    let view = |d: &Document| -> Vec<(String, Vec<u8>)> {
        d.layers()
            .iter()
            .map(|l| (l.name.clone(), l.pixels.data().to_vec()))
            .collect()
    };
    for (x, y, expand, (w, h), landed) in cases {
        let want = {
            let mut bus = CommandBus::new(s.image(&id).document().clone());
            let buf = decode(&bytes).expect("decode");
            bus.apply(Box::new(pincel_image_core::command::PasteLayer::at(
                buf, x, y, expand,
            )))
            .expect("paste");
            bus.document
        };
        let out = s.ok(
            "layer.import",
            json!({ "path": "b.png", "x": x, "y": y, "expand": expand }),
        );
        assert_eq!(
            (out["width"].clone(), out["height"].clone()),
            (json!(w), json!(h))
        );
        let doc = s.image(&id).document();
        assert_eq!(
            view(doc),
            view(&want),
            "Image paste at ({x}, {y}), expand {expand}"
        );
        let index = doc.active_layer_index();
        let layer = &doc.layers()[index].pixels;
        for &(cx, cy, i, j) in landed {
            let p = layer.get_pixel(cx, cy).expect("pixel");
            assert_eq!(
                [p.r, p.g, p.b, p.a],
                img(i, j),
                "({x}, {y}) at ({cx}, {cy})"
            );
        }
        let opaque = layer
            .data()
            .iter()
            .skip(3)
            .step_by(4)
            .filter(|&&a| a != 0)
            .count();
        let visible =
            (x.max(0)..(x + 3).min(w as i32)).len() * (y.max(0)..(y + 3).min(h as i32)).len();
        assert_eq!(
            opaque, visible,
            "Image paste at ({x}, {y}): every other pixel is dropped"
        );
    }
}

/// `buf` encoded by the `image` crate (WebP, GIF and TIFF have no encoder
/// in `pincel-image-core` the tests can call).
fn encode_with_image(buf: &ImageBuffer, format: image::ImageFormat) -> Vec<u8> {
    let img = image::RgbaImage::from_raw(buf.width(), buf.height(), buf.data().to_vec())
        .expect("rgba image");
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), format)
        .expect("encode");
    out
}

#[test]
fn layer_import_accepts_each_modes_formats_by_content() {
    let buf = ImageBuffer::from_raw(
        2,
        2,
        vec![
            10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 1, 2, 3, 255,
        ],
    )
    .expect("buf");
    // Every file is stored under a name that hides its format.
    let files = [
        ("PNG", png(2, 2, buf.data())),
        ("JPEG", to_jpeg_bytes(&buf, 90).expect("jpeg")),
        ("WebP", encode_with_image(&buf, image::ImageFormat::WebP)),
        ("BMP", to_bmp_bytes(&buf).expect("bmp")),
        ("GIF", encode_with_image(&buf, image::ImageFormat::Gif)),
        ("TIFF", encode_with_image(&buf, image::ImageFormat::Tiff)),
    ];
    let mut s = S::new();
    let id = s.new_image(4, 4);
    for (format, bytes) in &files {
        s.host.files.insert("pasted.dat".into(), bytes.clone());
        // The editor's Paste of the same bytes.
        let want = {
            let mut bus = CommandBus::new(s.image(&id).document().clone());
            let decoded = decode(bytes).expect("decode");
            bus.apply(Box::new(pincel_image_core::command::PasteLayer::at(
                decoded, 1, 1, false,
            )))
            .expect("paste");
            bus.document
        };
        s.ok(
            "layer.import",
            json!({ "path": "pasted.dat", "x": 1, "y": 1 }),
        );
        let got = s.image(&id).document();
        assert_eq!(got.layer_count(), want.layer_count(), "{format}");
        for (g, w) in got.layers().iter().zip(want.layers()) {
            assert_eq!(
                (&g.name, g.pixels.data()),
                (&w.name, w.pixels.data()),
                "{format}"
            );
        }
    }

    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.host
        .files
        .insert("pic.jpg".into(), png(1, 1, &[1, 2, 3, 255]));
    s.ok("layer.import", json!({ "path": "pic.jpg" }));
    for (format, bytes) in files.iter().skip(1) {
        s.host.files.insert("photo.png".into(), bytes.clone());
        let before = s.state(&id);
        let e = s.err("layer.import", json!({ "path": "photo.png" }));
        assert!(e.contains("PNG"), "{format}: {e}");
        assert_eq!(s.state(&id), before, "{format}");
    }
}

#[test]
fn layer_import_failures_state_the_reason_and_change_nothing() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.host
        .files
        .insert("junk.png".into(), b"not an image".to_vec());
    for (path, needle) in [("missing.png", "missing.png"), ("junk.png", "junk.png")] {
        let before = s.state(&id);
        let e = s.err("layer.import", json!({ "path": path }));
        assert!(e.contains(needle), "{e}");
        assert_eq!(s.state(&id), before);
    }
    s.host
        .files
        .insert("a.png".into(), png(1, 1, &[1, 2, 3, 255]));
    s.ws.get_mut(&id).expect("doc").sprite.color_mode = ColorMode::Indexed {
        transparent_index: 0,
    };
    let before = s.state(&id);
    let e = s.err("layer.import", json!({ "path": "a.png" }));
    assert!(e.contains("indexed"), "{e}");
    assert_eq!(s.state(&id), before);

    let mut s = S::new();
    let id = s.new_image(4, 4);
    s.host
        .files
        .insert("junk.png".into(), b"not an image".to_vec());
    let before = s.image(&id).document().clone();
    let e = s.err("layer.import", json!({ "path": "junk.png" }));
    assert!(e.contains("junk.png"), "{e}");
    assert_eq!(s.image(&id).document(), &before);
}

#[test]
fn layer_import_with_expand_beyond_the_limit_fails_naming_it() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.host
        .files
        .insert("a.png".into(), png(1, 1, &[1, 2, 3, 255]));
    let before = s.state(&id);
    let e = s.err(
        "layer.import",
        json!({ "path": "a.png", "x": 4096, "y": 0, "expand": true }),
    );
    assert!(e.contains("4096") && e.contains("4097×4"), "{e}");
    assert_eq!(s.state(&id), before);

    let mut s = S::new();
    let id = s.new_image(4, 4);
    s.host
        .files
        .insert("a.png".into(), png(1, 1, &[1, 2, 3, 255]));
    let before = s.image(&id).document().clone();
    let e = s.err(
        "layer.import",
        json!({ "path": "a.png", "x": 16384, "y": 0, "expand": true }),
    );
    assert!(e.contains("16384") && e.contains("16385×4"), "{e}");
    let e = s.err(
        "layer.import",
        json!({ "path": "a.png", "x": 9000, "y": 9000, "expand": true }),
    );
    assert!(e.contains("64 Mpx"), "{e}");
    assert_eq!(s.image(&id).document(), &before);
}

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
    s.ok(
        "export.selection",
        json!({ "path": "m.png", "merged": true }),
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
    let (_, pixels) = copy_selection(s.image(&id).document(), true).expect("copy");
    s.ok(
        "export.selection",
        json!({ "path": "m.png", "merged": true }),
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
    let depth = undo_depth(&mut s);
    s.ok("export.selection", json!({ "path": "c.png", "cut": true }));
    assert_eq!(undo_depth(&mut s), depth + 1);
    assert_eq!(s.image(&id).document().layers(), want.layers());
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
}

#[test]
fn clipboard_tools_naming_a_missing_layer_or_frame_or_a_fractional_offset_fail() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("select.all", json!({}));
    s.host
        .files
        .insert("a.png".into(), png(1, 1, &[1, 2, 3, 255]));
    for (tool, args, needle) in [
        (
            "select.move_content",
            json!({ "dx": 1, "dy": 0, "layer": 9 }),
            "`layer`",
        ),
        (
            "select.move_content",
            json!({ "dx": 1, "dy": 0, "frame": 9 }),
            "frame 9",
        ),
        ("select.move_content", json!({ "dx": 1.5, "dy": 0 }), "dx"),
        (
            "export.selection",
            json!({ "path": "x.png", "layer": 9 }),
            "`layer`",
        ),
        (
            "export.selection",
            json!({ "path": "x.png", "frame": 9 }),
            "frame 9",
        ),
        (
            "layer.import",
            json!({ "path": "a.png", "frame": 9 }),
            "frame 9",
        ),
        ("layer.import", json!({ "path": "a.png", "x": 0.5 }), "x"),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(e.contains(needle), "{tool} {args}: {e}");
        assert_eq!(s.state(&id), before);
    }
    let mut s = S::new();
    s.new_image(4, 4);
    for (tool, args) in [
        ("layer.translate", json!({ "dx": 1, "dy": 0, "layer": 9 })),
        (
            "select.move_content",
            json!({ "dx": 1, "dy": 0, "layer": 9 }),
        ),
        ("export.selection", json!({ "path": "x.png", "layer": 9 })),
    ] {
        let e = s.err(tool, args);
        assert!(e.contains("`layer`") && e.contains('9'), "{tool}: {e}");
    }
    let e = s.err("layer.translate", json!({ "dx": 1, "dy": 0.5 }));
    assert!(e.contains("dy"), "{e}");
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
