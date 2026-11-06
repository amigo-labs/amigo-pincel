//! `image.auto_crop` and Supersampling in `image.scale`
//! (docs/specs/image-resize-autocrop.md AC-17 – AC-20).

use pincel_image_core::Document;
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use serde_json::json;

use super::S;
use crate::list_tools;

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

/// A 10 × 8 Image document: white with a 3 × 3 red block at (3, 2) and a
/// full selection.
fn bordered(s: &mut S) -> String {
    let id = s.new_image(10, 8);
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 10, "y1": 8, "fill": "#ffffff", "anti_alias": false }),
    );
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 3, "y0": 2, "x1": 6, "y1": 5, "fill": "#ff0000", "anti_alias": false }),
    );
    s.ok("select.all", json!({}));
    id
}

#[test]
fn image_scale_supersampling_equals_the_editor() {
    let mut s = S::new();
    let id = bordered(&mut s);
    let want = image_editor(
        &s,
        &id,
        CommandSpec::ScaleImage {
            width: 3,
            height: 2,
            interpolation: "supersampling".into(),
        },
    );
    let bilinear = image_editor(
        &s,
        &id,
        CommandSpec::ScaleImage {
            width: 3,
            height: 2,
            interpolation: "bilinear".into(),
        },
    );
    assert_ne!(want, bilinear, "the filters must differ on this image");
    s.ok(
        "image.scale",
        json!({ "width": 3, "height": 2, "interpolation": "supersampling" }),
    );
    assert_eq!(s.image(&id).document(), &want);
}

#[test]
fn image_auto_crop_equals_the_editor_in_one_undo_step() {
    let mut s = S::new();
    let id = bordered(&mut s);
    let want = image_editor(&s, &id, CommandSpec::AutoCrop);
    let depth = undo_depth(&mut s);
    let out = s.ok("image.auto_crop", json!({}));
    assert_eq!(
        out,
        json!({ "cropped": true, "width": 3, "height": 3, "rect": { "x": 3, "y": 2, "w": 3, "h": 3 } })
    );
    assert_eq!(s.image(&id).document(), &want);
    assert!(s.image(&id).document().selection.is_none());
    assert_eq!(undo_depth(&mut s), depth + 1);
    s.ok("doc.undo", json!({}));
    assert_eq!(s.image(&id).document().canvas.width(), 10);
}

#[test]
fn image_auto_crop_with_nothing_to_crop_changes_nothing() {
    let mut s = S::new();
    let id = s.new_image(6, 6);
    let before = s.image(&id).document().clone();
    let depth = undo_depth(&mut s);
    assert_eq!(
        s.ok("image.auto_crop", json!({})),
        json!({ "cropped": false })
    );
    assert_eq!(s.image(&id).document(), &before);
    assert_eq!(undo_depth(&mut s), depth);
}

#[test]
fn image_auto_crop_on_a_pixel_document_names_its_mode() {
    let mut s = S::new();
    s.new_doc(8, 8);
    let msg = s.err("image.auto_crop", json!({}));
    assert!(msg.contains("Pixel"), "{msg}");
}

#[test]
fn auto_crop_and_supersampling_are_listed_and_documented() {
    let tools = list_tools();
    let crop = tools
        .iter()
        .find(|t| t.name == "image.auto_crop")
        .expect("image.auto_crop listed");
    assert!(!crop.description.is_empty());
    let scale = tools
        .iter()
        .find(|t| t.name == "image.scale")
        .expect("image.scale listed");
    let filters = &scale.input_schema["properties"]["interpolation"]["enum"];
    assert!(
        filters
            .as_array()
            .is_some_and(|a| a.contains(&json!("supersampling"))),
        "{filters}"
    );
    let guide = crate::resources::GUIDE;
    assert!(guide.contains("`image.auto_crop`"));
    assert!(guide.contains("supersampling"));
}
