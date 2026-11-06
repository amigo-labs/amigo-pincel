//! Moving content and the headless clipboard (mcp-parity-selection-transform-content
//! AC-01 … AC-22).

mod copy;
mod copy_shape;
mod moves;
mod paste;

use pincel_core::{
    AnyCommand, Bus, CelData, CelMap, ClearRegion, ColorMode, CopySource, FrameIndex, Layer,
    LayerId, MoveSelectionContent, PasteLayer, PastedImage, PixelBuffer, Rect, Sprite, copy_pixels,
    import_png,
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
        (
            "layer.import",
            json!({ "path": "a.png", "x": 0.5 }),
            "x must",
        ),
        (
            "layer.import",
            json!({ "path": "a.png", "y": 0.5 }),
            "y must",
        ),
        (
            "select.move_content",
            json!({ "dx": 1, "dy": 0.5 }),
            "dy must",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(e.contains(needle), "{tool} {args}: {e}");
        assert_eq!(s.state(&id), before);
    }
    assert!(s.host.files.keys().all(|k| k == "a.png"), "nothing written");
    let mut s = S::new();
    let id = image_doc(&mut s);
    s.ok(
        "select.rect",
        json!({ "x": 1, "y": 1, "width": 2, "height": 2 }),
    );
    s.host
        .files
        .insert("a.png".into(), png(1, 1, &[1, 2, 3, 255]));
    for (tool, args, needle) in [
        (
            "layer.translate",
            json!({ "dx": 1, "dy": 0, "layer": 9 }),
            "`layer`",
        ),
        (
            "select.move_content",
            json!({ "dx": 1, "dy": 0, "layer": 9 }),
            "`layer`",
        ),
        (
            "export.selection",
            json!({ "path": "x.png", "layer": 9 }),
            "`layer`",
        ),
        ("layer.translate", json!({ "dx": 1, "dy": 0.5 }), "dy must"),
        ("layer.translate", json!({ "dx": 0.5, "dy": 0 }), "dx must"),
        (
            "select.move_content",
            json!({ "dx": 0.5, "dy": 0 }),
            "dx must",
        ),
        ("select.move", json!({ "dx": 0.5, "dy": 0 }), "dx must"),
        ("select.move", json!({ "dx": 0, "dy": 1.5 }), "dy must"),
        (
            "layer.import",
            json!({ "path": "a.png", "x": 0.5 }),
            "x must",
        ),
        (
            "layer.import",
            json!({ "path": "a.png", "y": 0.5 }),
            "y must",
        ),
        // Image documents have one frame: any other is missing.
        (
            "layer.translate",
            json!({ "dx": 1, "dy": 0, "frame": 9 }),
            "frame 9",
        ),
        (
            "select.move_content",
            json!({ "dx": 1, "dy": 0, "frame": 9 }),
            "frame 9",
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
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, args.clone());
        assert!(e.contains(needle), "{tool} {args}: {e}");
        if needle == "`layer`" {
            assert!(e.contains('9'), "{tool}: {e}");
        }
        assert_eq!(s.image(&id).document(), &before, "{tool} {args}");
    }
}

#[test]
fn a_refused_cut_fails_with_the_commands_reason_and_writes_nothing() {
    // An indexed sprite, as `doc.open` reads one: Copy works, but the
    // Cut's `ClearRegion` refuses an indexed cel (AC-21).
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    {
        let d = s.ws.get_mut(&id).expect("doc");
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        d.sprite.color_mode = mode;
        let mut buf = PixelBuffer::empty(4, 4, mode);
        buf.data[0] = 1;
        d.cels
            .get_mut(LayerId::new(0), FrameIndex::new(0))
            .expect("cel")
            .data = CelData::Image(buf);
    }
    let copied = s.ok("export.selection", json!({ "path": "copy.png" }));
    assert_eq!(copied["area"], json!({ "x": 0, "y": 0, "w": 4, "h": 4 }));
    let refusal = {
        let (mut sprite, mut cels) = s.state(&id);
        Bus::new()
            .execute(
                ClearRegion::new(LayerId::new(0), FrameIndex::new(0), Rect::new(0, 0, 4, 4)).into(),
                &mut sprite,
                &mut cels,
            )
            .expect_err("the editor's Cut refuses")
            .to_string()
    };
    let before = s.state(&id);
    let depth = undo_depth(&mut s);
    let e = s.err(
        "export.selection",
        json!({ "path": "cut.png", "cut": true }),
    );
    assert!(e.contains(&refusal), "{e} carries {refusal}");
    assert_eq!(s.state(&id), before);
    assert_eq!(undo_depth(&mut s), depth);
    assert!(
        !s.host.files.contains_key("cut.png"),
        "a refused cut writes nothing"
    );
}
