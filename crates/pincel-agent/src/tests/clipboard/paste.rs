//! `layer.import`: the headless Paste (AC-09 … AC-13).

use super::*;

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
    let depth = undo_depth(&mut s);
    let out = s.ok("layer.import", json!({ "path": "a.png", "x": 3, "y": 2 }));
    assert_eq!(undo_depth(&mut s), depth + 1, "one undo step");
    assert_eq!(out["layer"], s.ok("doc.info", json!({}))["active_layer"]);
    assert_eq!(out["layer"], 2, "the new layer's id");
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
    let cases: [Case; 5] = [
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
        // Negative offsets without expand: cut off at the left and top
        // edges, the canvas stays.
        (-1, -2, false, (5, 6), &[(0, 0, 1, 2), (1, 0, 2, 2)]),
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
    for (path, needles) in [
        (
            "missing.png",
            &["cannot read `missing.png`", "no such file"][..],
        ),
        (
            "junk.png",
            &["cannot paste `junk.png`", "PNG files only"][..],
        ),
    ] {
        let before = s.state(&id);
        let e = s.err("layer.import", json!({ "path": path }));
        for needle in needles {
            assert!(e.contains(needle), "{path}: {needle} in {e}");
        }
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
    assert!(
        e.contains("cannot paste `junk.png`") && e.contains("PNG, JPEG, WebP, BMP, GIF or TIFF"),
        "{e}"
    );
    assert_eq!(s.image(&id).document(), &before);
    let e = s.err("layer.import", json!({ "path": "missing.png" }));
    assert!(
        e.contains("cannot read `missing.png`") && e.contains("no such file"),
        "{e}"
    );
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
    assert!(e.contains("9001×9001") && e.contains("64 Mpx"), "{e}");
    assert_eq!(s.image(&id).document(), &before);
}

#[test]
fn layer_import_of_an_image_doc_open_refuses_fails_like_doc_open() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.host.files.insert(
        "big.png".into(),
        png(16385, 1, &[0, 0, 0, 255].repeat(16385)),
    );
    let opened = s.err("doc.open", json!({ "path": "big.png", "mode": "image" }));
    assert!(opened.contains("16385×1 px exceeds"), "{opened}");
    let before = s.image(&id).document().clone();
    let e = s.err("layer.import", json!({ "path": "big.png", "doc": id }));
    assert!(
        e.contains("16385×1 px exceeds the Image document limit of 16384 px per side"),
        "{e}"
    );
    assert_eq!(s.image(&id).document(), &before);
}

#[test]
fn layer_import_in_an_image_document_lands_above_a_middle_active_layer() {
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({ "name": "Top" }));
    // The copy of layer 0 sits between layer 0 and Top and becomes active.
    let middle = s.ok("layer.duplicate", json!({ "layer": 0 }))["layer"].clone();
    assert_eq!(s.ok("doc.info", json!({}))["active_layer"], middle);
    s.host
        .files
        .insert("a.png".into(), png(1, 1, &[255, 0, 0, 255]));
    let want = {
        let mut bus = CommandBus::new(s.image(&id).document().clone());
        let buf = decode(&s.host.files["a.png"]).expect("decode");
        bus.apply(Box::new(pincel_image_core::command::PasteLayer::at(
            buf, 1, 1, false,
        )))
        .expect("paste");
        bus.document
    };
    let depth = undo_depth(&mut s);
    let out = s.ok("layer.import", json!({ "path": "a.png", "x": 1, "y": 1 }));
    assert_eq!(undo_depth(&mut s), depth + 1, "one undo step");
    let list = s.ok("layer.list", json!({}))["layers"].clone();
    let ids: Vec<Value> = list
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| l["id"].clone())
        .collect();
    assert_eq!(
        ids,
        [json!(0), middle.clone(), out["layer"].clone(), json!(1)],
        "directly above the active layer, below Top"
    );
    assert_eq!(list[2]["name"], "Layer 4", "named after the layer count");
    assert_eq!(s.ok("doc.info", json!({}))["active_layer"], out["layer"]);
    let view = |d: &Document| -> Vec<(String, Vec<u8>)> {
        d.layers()
            .iter()
            .map(|l| (l.name.clone(), l.pixels.data().to_vec()))
            .collect()
    };
    assert_eq!(
        view(s.image(&id).document()),
        view(&want),
        "the editor's Paste"
    );
}
