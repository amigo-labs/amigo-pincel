//! `export.selection` of a shaped selection and Copy Merged with a hidden
//! layer (AC-14, AC-15).

use super::*;

/// Select the 2×2 square at (1, 1) without its top-right pixel (2, 1).
fn select_notched_square(s: &mut S) {
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 2, "y": 1, "width": 1, "height": 1, "mode": "subtract" }),
    );
}

#[test]
fn export_selection_leaves_pixels_outside_the_shape_transparent() {
    // Pixel: canvas (2, 1) holds green but lies outside the shape.
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    select_notched_square(&mut s);
    let before = s.state(&id);
    let out = s.ok("export.selection", json!({ "path": "p.png" }));
    assert_eq!(out["area"], json!({ "x": 1, "y": 1, "w": 2, "h": 2 }));
    let png = decode(&s.host.files["p.png"]).expect("png");
    assert_eq!((png.width(), png.height()), (2, 2));
    assert_eq!(px(png.data(), 2, 0, 0), [255, 0, 0, 255], "inside");
    assert_eq!(px(png.data(), 2, 1, 0), [0, 0, 0, 0], "outside the shape");
    assert_eq!(px(png.data(), 2, 1, 1), [0, 0, 255, 128], "inside");
    let d = s.doc(&id);
    let want = copy_pixels(
        &d.sprite,
        &d.cels,
        FrameIndex::new(0),
        CopySource::Layer(LayerId::new(0)),
    )
    .expect("copy");
    assert_eq!(png.data(), want.rgba.as_slice());
    assert_eq!(s.state(&id), before);

    // Image: canvas (2, 1) holds the blue fill but lies outside the shape.
    let mut s = S::new();
    let id = image_doc(&mut s);
    select_notched_square(&mut s);
    let before = s.image(&id).document().layers().to_vec();
    let out = s.ok("export.selection", json!({ "path": "i.png" }));
    assert_eq!(out["area"], json!({ "x": 1, "y": 1, "w": 2, "h": 2 }));
    let png = decode(&s.host.files["i.png"]).expect("png");
    assert_eq!((png.width(), png.height()), (2, 2));
    assert_eq!(px(png.data(), 2, 0, 0), [0x33, 0x66, 0xcc, 255], "inside");
    assert_eq!(px(png.data(), 2, 1, 0), [0, 0, 0, 0], "outside the shape");
    let (_, want) = copy_selection(s.image(&id).document(), false).expect("copy");
    assert_eq!(png.data(), want.data());
    assert_eq!(s.image(&id).document().layers(), before.as_slice());

    // Image: a feathered selection scales each pixel's alpha by its
    // coverage.
    s.ok(
        "select.ellipse",
        json!({ "x": 0, "y": 0, "width": 6, "height": 5, "feather": 2 }),
    );
    let out = s.ok("export.selection", json!({ "path": "f.png" }));
    let png = decode(&s.host.files["f.png"]).expect("png");
    let d = s.image(&id).document();
    let (sel, layer) = (d.selection.as_ref().expect("sel"), &d.layers()[0].pixels);
    let (ax, ay) = (out["area"]["x"].as_u64(), out["area"]["y"].as_u64());
    let (ax, ay) = (ax.expect("x") as u32, ay.expect("y") as u32);
    let mut partial = 0;
    for y in 0..png.height() {
        for x in 0..png.width() {
            let (cx, cy) = (ax + x, ay + y);
            let src = px(layer.data(), layer.width(), cx, cy);
            let cov = u32::from(sel.coverage(cx, cy));
            let alpha = (u32::from(src[3]) * cov + 127) / 255;
            let want = match alpha {
                0 => [0; 4],
                a => [src[0], src[1], src[2], a as u8],
            };
            partial += usize::from(0 < alpha && alpha < u32::from(src[3]));
            assert_eq!(px(png.data(), png.width(), x, y), want, "({cx}, {cy})");
        }
    }
    assert!(partial > 0, "the feather scales some alpha");
}

#[test]
fn export_selection_merged_leaves_hidden_layers_out() {
    // Pixel: a hidden top layer over the red pixel at (1, 1).
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "layer": 1, "pixels": [{ "x": 1, "y": 1, "color": "#ffff00" }] }),
    );
    s.ok(
        "select.rect",
        json!({ "x": 0, "y": 0, "width": 3, "height": 3 }),
    );
    let merged = |s: &S| {
        let d = s.doc(&id);
        copy_pixels(&d.sprite, &d.cels, FrameIndex::new(0), CopySource::Merged)
            .expect("copy")
            .rgba
    };
    let with_top = merged(&s);
    s.ok("layer.set", json!({ "layer": 1, "visible": false }));
    let want = merged(&s);
    assert_ne!(want, with_top, "the hidden layer covers a selected pixel");
    s.ok(
        "export.selection",
        json!({ "path": "m.png", "merged": true }),
    );
    assert_eq!(import_png(&s.host.files["m.png"]).expect("png").rgba, want);

    // Image: a hidden opaque top layer.
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok("layer.add", json!({}));
    s.ok("draw.fill", json!({ "x": 0, "y": 0, "color": "#00ff00" }));
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 4, "height": 3 }),
    );
    let (_, with_top) = copy_selection(s.image(&id).document(), true).expect("copy");
    s.ok("layer.set", json!({ "layer": 1, "visible": false }));
    let (_, want) = copy_selection(s.image(&id).document(), true).expect("copy");
    assert_ne!(want.data(), with_top.data(), "the hidden layer covers it");
    s.ok(
        "export.selection",
        json!({ "path": "m.png", "merged": true }),
    );
    assert_eq!(
        decode(&s.host.files["m.png"]).expect("png").data(),
        want.data()
    );
}
