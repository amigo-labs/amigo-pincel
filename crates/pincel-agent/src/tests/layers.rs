//! Layer structure tools in both modes: placement, duplicate, move,
//! merges, flatten, Image groups and locks (mcp-parity-structure AC-02 …
//! AC-13, AC-25, AC-27 … AC-29).

use pincel_core::{
    AnyCommand, Bus, CelMap, FlattenImage, Layer, LayerId, LayerPlacement, MergeDown, MoveLayerTo,
    Sprite, TilesetId,
};
use pincel_image_core::command::Command;
use pincel_image_core::{Document, LayerKind as ImageKind};
use serde_json::{Value, json};

use super::S;

fn ids(list: &Value) -> Vec<u64> {
    list.as_array()
        .expect("layers")
        .iter()
        .map(|l| l["id"].as_u64().expect("id"))
        .collect()
}

fn layers(s: &mut S) -> Value {
    s.ok("layer.list", json!({}))["layers"].clone()
}

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// The Pixel state after running `cmd` on a copy of the document, the way
/// the editor's command would leave it.
fn pixel_editor(s: &S, id: &str, cmd: AnyCommand) -> (Sprite, CelMap) {
    let (mut sprite, mut cels) = s.state(id);
    Bus::new()
        .execute(cmd, &mut sprite, &mut cels)
        .expect("editor command");
    (sprite, cels)
}

/// The Image document after `cmd` on a copy, as the editor leaves it.
fn image_editor(s: &S, id: &str, mut cmd: impl Command) -> Document {
    let mut doc = s.image(id).document().clone();
    cmd.apply(&mut doc).expect("editor command");
    doc
}

/// Image layers as the editor shows them (no UUIDs): name, kind, depth,
/// pixels, visibility, opacity and lock.
type LayerView = (String, bool, usize, Vec<u8>, bool, u32, bool);

fn image_view(doc: &Document) -> Vec<LayerView> {
    doc.layers()
        .iter()
        .enumerate()
        .map(|(i, l)| {
            (
                l.name.clone(),
                matches!(l.kind, ImageKind::Group),
                doc.depth(i),
                l.pixels.data().to_vec(),
                l.visible,
                (l.opacity * 255.0).round() as u32,
                l.locked,
            )
        })
        .collect()
}

/// A Pixel document with a group `G` (id 1) holding `A` (id 2) and `B`
/// (id 3), on top of `Layer 1` (id 0): stack 0, 1, 2, 3.
fn pixel_tree(s: &mut S) -> String {
    let id = s.new_doc(4, 4);
    s.ok("layer.add", json!({ "name": "G", "kind": "group" }));
    s.ok("layer.add", json!({ "name": "A", "parent": 1 }));
    s.ok("layer.add", json!({ "name": "B", "parent": 1 }));
    id
}

/// An Image document: `Layer 1` (id 0), group `Group 1` (id 1) holding
/// `A` (id 2) and `B` (id 3), each layer painted.
fn image_tree(s: &mut S) -> String {
    let id = s.new_image(4, 4);
    s.ok(
        "draw.fill",
        json!({ "x": 0, "y": 0, "color": "#ff000080", "layer": 0 }),
    );
    let g = s.ok("layer.add", json!({ "kind": "group" }))["layer"].clone();
    assert_eq!(g, 1);
    s.ok("layer.add", json!({ "name": "A", "parent": 1 }));
    s.ok(
        "draw.fill",
        json!({ "x": 0, "y": 0, "color": "#00ff0080", "layer": 2 }),
    );
    s.ok(
        "layer.add",
        json!({ "name": "B", "target": 2, "place": "above" }),
    );
    s.ok(
        "draw.stroke",
        json!({ "points": [[0.5, 0.5], [3.5, 3.5]], "color": "#0000ff", "size": 1, "layer": 3 }),
    );
    id
}

#[test]
fn layer_add_without_target_places_the_layer_like_the_editor() {
    // Pixel: always on top of the root stack, whatever is active.
    let mut s = S::new();
    pixel_tree(&mut s);
    s.ok("layer.set", json!({ "layer": 0, "name": "Base" }));
    let out = s.ok("layer.add", json!({ "name": "Top" }));
    let list = out["layers"].clone();
    assert_eq!(ids(&list), [0, 1, 2, 3, 4]);
    assert_eq!(list[4]["parent"], Value::Null);
    let out = s.ok("layer.add", json!({ "name": "In G", "parent": 1 }));
    assert_eq!(ids(&out["layers"]), [0, 1, 2, 3, 5, 4]);
    assert_eq!(out["layers"][4]["parent"], 1);

    // Image: directly above the active layer, in its group.
    let mut s = S::new();
    image_tree(&mut s);
    // B (id 3) is active: the new layer goes above it inside the group.
    let out = s.ok("layer.add", json!({}));
    assert_eq!(out["layer"], 4);
    assert_eq!(ids(&out["layers"]), [0, 1, 2, 3, 4]);
    assert_eq!(out["layers"][4]["parent"], 1);
    let mut s2 = S::new();
    image_tree(&mut s2);
    s2.ok(
        "layer.move",
        json!({ "layer": 1, "target": 0, "place": "below" }),
    );
    // Active is now the group (layer.move activates the moved layer):
    // the new layer goes above the group's whole subtree.
    let out = s2.ok("layer.add", json!({}));
    let list = out["layers"].clone();
    assert_eq!(ids(&list), [1, 2, 3, 4, 0]);
    assert_eq!(list[3]["parent"], Value::Null);
    // A new group goes on top of the root stack, named like the editor's.
    let out = s2.ok("layer.add", json!({ "kind": "group" }));
    let list = out["layers"].clone();
    assert_eq!(list[5]["name"], "Group 2");
    assert_eq!(list[5]["kind"], "group");
}

#[test]
fn layer_add_with_target_and_place_puts_the_layer_there() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            image_tree(&mut s);
        } else {
            pixel_tree(&mut s);
        }
        let out = s.ok(
            "layer.add",
            json!({ "name": "U", "target": 2, "place": "below" }),
        );
        let u = out["layer"].as_u64().expect("id");
        assert_eq!(ids(&out["layers"]), [0, 1, u, 2, 3], "image={image}");
        assert_eq!(out["layers"][2]["parent"], 1);
        let out = s.ok(
            "layer.add",
            json!({ "name": "V", "target": 1, "place": "above" }),
        );
        let v = out["layer"].as_u64().expect("id");
        assert_eq!(ids(&out["layers"]), [0, 1, u, 2, 3, v]);
        assert_eq!(out["layers"][5]["parent"], Value::Null);
        let out = s.ok(
            "layer.add",
            json!({ "name": "W", "target": 1, "place": "into" }),
        );
        let w = out["layer"].as_u64().expect("id");
        assert_eq!(ids(&out["layers"]), [0, 1, u, 2, 3, w, v]);
        assert_eq!(out["layers"][5]["parent"], 1);
        assert_eq!(s.ok("doc.info", json!({}))["active_layer"], w);
    }
}

#[test]
fn layer_duplicate_copies_the_layer_directly_above_its_source() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#ff0000" }] }),
    );
    s.ok("frame.add", json!({}));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 2, "y": 3, "color": "#00ff00" }], "frame": 1 }),
    );
    s.ok("layer.add", json!({ "name": "Top" }));
    s.ok(
        "layer.set",
        json!({ "layer": 0, "visible": false, "opacity": 90, "blend": "screen", "locked": true }),
    );
    let out = s.ok("layer.duplicate", json!({ "layer": 0 }));
    assert_eq!(out["layer"], 2);
    let list = out["layers"].clone();
    assert_eq!(ids(&list), [0, 2, 1]);
    let copy = &list[1];
    assert_eq!(copy["name"], "Layer 1 copy");
    assert_eq!(copy["visible"], false);
    assert_eq!(copy["opacity"], 90);
    assert_eq!(copy["blend"], "screen");
    assert_eq!(copy["locked"], true);
    for f in 0..2 {
        assert_eq!(s.cel_rgba(&id, 2, f), s.cel_rgba(&id, 0, f), "frame {f}");
    }
    assert_eq!(s.ok("doc.info", json!({}))["active_layer"], 2);

    // Image: a group is copied with its whole subtree.
    let mut s = S::new();
    let id = image_tree(&mut s);
    let before = image_view(s.image(&id).document());
    let out = s.ok("layer.duplicate", json!({ "layer": 1 }));
    let copy = out["layer"].as_u64().expect("id");
    let list = out["layers"].clone();
    assert_eq!(list[4]["id"], copy);
    assert_eq!(list[4]["name"], "Group 1 copy");
    assert_eq!(list[5]["parent"], copy);
    assert_eq!(list[6]["parent"], copy);
    let after = image_view(s.image(&id).document());
    assert_eq!(after[5..7], before[2..4]);
    assert_eq!(s.ok("doc.info", json!({}))["active_layer"], copy);
}

#[test]
fn layer_duplicate_of_a_pixel_group_fails_and_changes_nothing() {
    let mut s = S::new();
    let id = pixel_tree(&mut s);
    let before = s.state(&id);
    let e = s.err("layer.duplicate", json!({ "layer": 1 }));
    assert!(e.contains("Pixel groups cannot be duplicated"), "{e}");
    assert_eq!(s.state(&id), before);
}

#[test]
fn layer_move_equals_the_editors_move() {
    let mut s = S::new();
    let id = pixel_tree(&mut s);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }], "layer": 2 }),
    );
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#0000ff80" }], "layer": 0 }),
    );
    for (layer, target, place, lp) in [
        (0, 3, "above", LayerPlacement::Above),
        (0, 1, "above", LayerPlacement::Above),
        (1, 0, "below", LayerPlacement::Below),
        (0, 1, "into", LayerPlacement::Into),
    ] {
        let want = pixel_editor(
            &s,
            &id,
            MoveLayerTo::new(LayerId::new(layer), LayerId::new(target), lp).into(),
        );
        s.ok(
            "layer.move",
            json!({ "layer": layer, "target": target, "place": place }),
        );
        let got = s.state(&id);
        assert_eq!(got.0.layers, want.0.layers, "{layer} {place} {target}");
        assert_eq!(got.1, want.1);
    }

    let mut s = S::new();
    let id = image_tree(&mut s);
    use pincel_image_core::LayerPlacement as P;
    for (layer, target, place, lp) in [
        (1u32, 0u32, "below", P::Below),
        (2, 0, "above", P::Above),
        (0, 1, "into", P::Into),
    ] {
        let d = s.image(&id);
        let (i, t) = (
            d.layer_index(layer).expect("i"),
            d.layer_index(target).expect("t"),
        );
        let want = image_editor(&s, &id, pincel_image_core::MoveLayerTo::new(i, t, lp));
        s.ok(
            "layer.move",
            json!({ "layer": layer, "target": target, "place": place }),
        );
        assert_eq!(s.image(&id).document(), &want, "{layer} {place} {target}");
        assert_eq!(
            pincel_image_core::compose(s.image(&id).document().layers()),
            pincel_image_core::compose(want.layers())
        );
    }
}

#[test]
fn layer_move_and_add_with_bad_placement_fail_naming_the_argument() {
    for image in [false, true] {
        let mut s = S::new();
        let id = if image {
            image_tree(&mut s)
        } else {
            pixel_tree(&mut s)
        };
        let cases: Vec<(&str, Value, &str)> = vec![
            (
                "layer.move",
                json!({ "layer": 2, "target": 3, "place": "into" }),
                "place",
            ),
            (
                "layer.move",
                json!({ "layer": 2, "target": 2, "place": "above" }),
                "target",
            ),
            (
                "layer.move",
                json!({ "layer": 1, "target": 3, "place": "below" }),
                "target",
            ),
            (
                "layer.move",
                json!({ "layer": 2, "place": "above" }),
                "place",
            ),
            ("layer.move", json!({ "layer": 2, "target": 0 }), "target"),
            (
                "layer.add",
                json!({ "target": 0, "place": "into" }),
                "place",
            ),
            ("layer.add", json!({ "place": "above" }), "place"),
            ("layer.add", json!({ "target": 0 }), "target"),
            (
                "layer.add",
                json!({ "parent": 1, "target": 0, "place": "above" }),
                "parent",
            ),
        ];
        for (tool, args, arg) in cases {
            let before = (
                s.ws.get(&id).map(|d| (d.sprite.clone(), d.cels.clone())),
                s.ws.get_image(&id).map(|d| d.document().clone()),
            );
            let e = s.err(tool, args.clone());
            assert!(e.contains(&format!("`{arg}`")), "{tool} {args}: {e}");
            let after = (
                s.ws.get(&id).map(|d| (d.sprite.clone(), d.cels.clone())),
                s.ws.get_image(&id).map(|d| d.document().clone()),
            );
            assert_eq!(after, before, "{tool} {args}");
        }
    }
}

#[test]
fn layer_merge_down_equals_the_editors_merge_down() {
    let mut s = S::new();
    let id = s.new_doc(4, 4);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }] }),
    );
    s.ok("layer.add", json!({ "opacity": 128, "blend": "multiply" }));
    s.ok("draw.pixels", json!({ "pixels": [{ "x": 0, "y": 0, "color": "#00ff00" }, { "x": 1, "y": 0, "color": "#0000ff" }] }));
    let want = pixel_editor(&s, &id, MergeDown::new(LayerId::new(1)).into());
    s.ok("layer.merge_down", json!({ "layer": 1 }));
    assert_eq!(s.state(&id), want);
    assert_eq!(s.ok("doc.info", json!({}))["active_layer"], 0);

    let mut s = S::new();
    let id = image_tree(&mut s);
    let want = image_editor(&s, &id, pincel_image_core::MergeDown::at(3));
    s.ok("layer.merge_down", json!({ "layer": 3 }));
    assert_eq!(s.image(&id).document(), &want);
}

#[test]
fn layer_merge_down_the_editor_refuses_fails_with_the_reason() {
    let mut s = S::new();
    let id = pixel_tree(&mut s);
    s.ok("layer.add", json!({ "name": "T" }));
    s.ws.get_mut(&id)
        .expect("doc")
        .sprite
        .layers
        .push(Layer::tilemap(LayerId::new(9), "Map", TilesetId::new(0)));
    s.ok("layer.add", json!({ "name": "Over map" }));
    for (layer, reason) in [
        (0, "bottom layer"),
        (1, "is a group"),
        (2, "no image layer directly below"),
        (4, "no image layer directly below"),
        (9, "tilemap layer"),
        (10, "no image layer directly below"),
    ] {
        let before = s.state(&id);
        let e = s.err("layer.merge_down", json!({ "layer": layer }));
        assert!(e.contains(reason), "{layer}: {e}");
        assert_eq!(s.state(&id), before, "{layer}");
    }

    let mut s = S::new();
    let id = image_tree(&mut s);
    for (layer, reason) in [
        (0, "bottom layer"),
        (1, "is a group"),
        (2, "no image layer directly below"),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err("layer.merge_down", json!({ "layer": layer }));
        assert!(e.contains(reason), "{layer}: {e}");
        assert_eq!(s.image(&id).document(), &before);
    }
}

#[test]
fn layer_merge_visible_and_merge_group_equal_the_editors() {
    let mut s = S::new();
    let id = image_tree(&mut s);
    let i = s.image(&id).layer_index(1).expect("group");
    let want = image_editor(&s, &id, pincel_image_core::MergeGroup::at(i));
    s.ok("layer.merge_group", json!({ "layer": 1 }));
    assert_eq!(s.image(&id).document(), &want);
    s.ok("doc.undo", json!({}));

    s.ok(
        "layer.add",
        json!({ "name": "Hidden", "visible": false, "target": 1, "place": "above" }),
    );
    let want = image_editor(&s, &id, pincel_image_core::MergeVisible::new());
    s.ok("layer.merge_visible", json!({}));
    assert_eq!(image_view(s.image(&id).document()), image_view(&want));
    let list = layers(&mut s);
    assert_eq!(list[0]["name"], "Merged");
    assert_eq!(list[1]["name"], "Hidden");

    let hidden = list[1]["id"].clone();
    let before = s.image(&id).document().clone();
    let e = s.err("layer.merge_group", json!({ "layer": hidden }));
    assert!(e.contains("`layer`") && e.contains("not a group"), "{e}");
    assert_eq!(s.image(&id).document(), &before);
}

#[test]
fn layer_flatten_equals_the_editors_flatten() {
    let mut s = S::new();
    let id = pixel_tree(&mut s);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }], "layer": 2 }),
    );
    s.ok("frame.add", json!({ "clone_from": 0 }));
    s.ok("layer.add", json!({ "name": "Hidden", "visible": false }));
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#00ff00" }], "layer": 4 }),
    );
    let want = pixel_editor(&s, &id, FlattenImage::new().into());
    s.ok("layer.flatten", json!({}));
    assert_eq!(s.state(&id), want);

    let mut s = S::new();
    let id = image_tree(&mut s);
    let want = image_editor(&s, &id, pincel_image_core::FlattenImage::new());
    s.ok("layer.flatten", json!({}));
    assert_eq!(image_view(s.image(&id).document()), image_view(&want));
    assert_eq!(layers(&mut s)[0]["name"], "Background");
}

#[test]
fn merge_and_flatten_refuse_locked_layers_while_duplicate_and_move_work() {
    let mut s = S::new();
    let id = pixel_tree(&mut s);
    s.ok("layer.set", json!({ "layer": 2, "locked": true }));
    for (tool, args) in [
        ("layer.merge_down", json!({ "layer": 3 })),
        ("layer.flatten", json!({})),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args);
        assert!(e.contains("layer 2") && e.contains("locked"), "{tool}: {e}");
        assert_eq!(s.state(&id), before);
    }
    s.ok("layer.duplicate", json!({ "layer": 2 }));
    s.ok(
        "layer.move",
        json!({ "layer": 2, "target": 0, "place": "below" }),
    );

    // A layer inside a locked group (Pixel): the group's lock reaches it.
    s.ok("layer.set", json!({ "layer": 2, "locked": false }));
    s.ok("layer.set", json!({ "layer": 1, "locked": true }));
    let before = s.state(&id);
    let e = s.err("layer.merge_down", json!({ "layer": 3 }));
    assert!(e.contains("layer 3") && e.contains("locked"), "{e}");
    assert_eq!(s.state(&id), before);

    let mut s = S::new();
    let id = image_tree(&mut s);
    s.ok("layer.set", json!({ "layer": 1, "locked": true }));
    // Every refused tool names a layer of the locked group's subtree: the
    // group itself or the image layer inside it that the merge would change.
    for (tool, args, named) in [
        ("layer.merge_down", json!({ "layer": 3 }), "layer 3"),
        ("layer.merge_group", json!({ "layer": 1 }), "layer 1"),
        ("layer.merge_visible", json!({}), "layer 1"),
        ("layer.flatten", json!({}), "layer 1"),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, args);
        assert!(e.contains("locked") && e.contains(named), "{tool}: {e}");
        assert_eq!(s.image(&id).document(), &before);
    }
    s.ok("layer.duplicate", json!({ "layer": 2 }));
    s.ok(
        "layer.move",
        json!({ "layer": 1, "target": 0, "place": "below" }),
    );
}

#[test]
fn image_groups_are_listed_with_kind_and_parent() {
    let mut s = S::new();
    let id = s.new_image(4, 4);
    let g = s.ok("layer.add", json!({ "kind": "group" }));
    assert_eq!(g["layers"][1]["name"], "Group 1");
    let named = s.ok(
        "layer.add",
        json!({ "kind": "group", "name": "Icons", "parent": 1 }),
    );
    let icons = named["layer"].as_u64().expect("id");
    s.ok("layer.set", json!({ "layer": 0, "parent": icons }));
    let list = layers(&mut s);
    let tree: Vec<(u64, &str, Value)> = list
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| {
            (
                l["id"].as_u64().expect("id"),
                l["kind"].as_str().expect("kind"),
                l["parent"].clone(),
            )
        })
        .collect();
    assert_eq!(
        tree,
        [
            (1, "group", Value::Null),
            (icons, "group", json!(1)),
            (0, "image", json!(icons)),
        ]
    );
    let d = s.image(&id).document();
    assert!(d.is_well_formed());
    assert_eq!(d.parent_index(2), Some(1));
    s.ok(
        "layer.move",
        json!({ "layer": 0, "target": 1, "place": "above" }),
    );
    assert_eq!(layers(&mut s)[2]["parent"], Value::Null);
    s.ok("layer.add", json!({ "name": "Keep" }));
    s.ok("layer.remove", json!({ "layer": 1 }));
    let list = layers(&mut s);
    let names: Vec<&str> = list
        .as_array()
        .expect("l")
        .iter()
        .map(|l| l["name"].as_str().expect("n"))
        .collect();
    assert_eq!(names, ["Layer 1", "Keep"]);
}

#[test]
fn image_pixel_tools_on_a_group_fail_saying_it_is_a_group() {
    let mut s = S::new();
    let id = image_tree(&mut s);
    for (tool, args) in [
        (
            "draw.fill",
            json!({ "x": 0, "y": 0, "color": "#ffffff", "layer": 1 }),
        ),
        (
            "draw.stroke",
            json!({ "points": [[0, 0], [2, 2]], "color": "#ffffff", "layer": 1 }),
        ),
        ("adjust.apply", json!({ "id": "invert", "layer": 1 })),
        ("effect.apply", json!({ "id": "gaussian_blur", "layer": 1 })),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, args);
        assert!(e.contains("is a group"), "{tool}: {e}");
        assert_eq!(s.image(&id).document(), &before);
    }
}

#[test]
fn layer_tools_record_one_undo_step_and_none_without_a_change() {
    for image in [false, true] {
        let mut s = S::new();
        if image {
            image_tree(&mut s);
        } else {
            pixel_tree(&mut s);
        }
        let mut calls: Vec<(&str, Value)> = vec![
            ("layer.add", json!({ "target": 0, "place": "above" })),
            ("layer.duplicate", json!({ "layer": 2 })),
            (
                "layer.move",
                json!({ "layer": 0, "target": 3, "place": "above" }),
            ),
            ("layer.merge_down", json!({ "layer": 3 })),
        ];
        if image {
            calls.push(("layer.merge_group", json!({ "layer": 1 })));
            calls.push(("layer.merge_visible", json!({})));
        }
        calls.push(("layer.flatten", json!({})));
        for (tool, args) in calls {
            let depth = undo_depth(&mut s);
            s.ok(tool, args.clone());
            assert_eq!(undo_depth(&mut s), depth + 1, "image={image} {tool} {args}");
        }
        // Nothing to change: no undo step.
        let mut s = S::new();
        if image {
            image_tree(&mut s);
        } else {
            pixel_tree(&mut s);
        }
        let depth = undo_depth(&mut s);
        let out = s.ok(
            "layer.move",
            json!({ "layer": 3, "target": 2, "place": "above" }),
        );
        assert_eq!(out["changed"], false);
        assert_eq!(undo_depth(&mut s), depth, "image={image}");
        if image {
            for l in [0, 1] {
                s.ok("layer.set", json!({ "layer": l, "visible": false }));
            }
            let depth = undo_depth(&mut s);
            assert_eq!(s.ok("layer.merge_visible", json!({}))["changed"], false);
            assert_eq!(undo_depth(&mut s), depth);
        }
    }
}

#[test]
fn layer_tools_naming_a_missing_layer_fail_with_argument_and_value() {
    for image in [false, true] {
        let mut s = S::new();
        let id = if image {
            image_tree(&mut s)
        } else {
            pixel_tree(&mut s)
        };
        for (tool, args, arg) in [
            ("layer.duplicate", json!({ "layer": 42 }), "layer"),
            (
                "layer.move",
                json!({ "layer": 42, "target": 0, "place": "above" }),
                "layer",
            ),
            (
                "layer.move",
                json!({ "layer": 0, "target": 42, "place": "above" }),
                "target",
            ),
            (
                "layer.add",
                json!({ "target": 42, "place": "below" }),
                "target",
            ),
            ("layer.merge_down", json!({ "layer": 42 }), "layer"),
        ] {
            let pixel_before = (!image).then(|| s.state(&id));
            let image_before = image.then(|| s.image(&id).document().clone());
            let e = s.err(tool, args);
            assert!(
                e.contains(&format!("`{arg}`")) && e.contains("42"),
                "{tool}: {e}"
            );
            if image {
                assert_eq!(Some(s.image(&id).document()), image_before.as_ref());
            } else {
                assert_eq!(Some(s.state(&id)), pixel_before, "{tool}");
            }
        }
    }
    let mut s = S::new();
    let id = image_tree(&mut s);
    let before = s.image(&id).document().clone();
    let e = s.err("layer.merge_group", json!({ "layer": 42 }));
    assert!(e.contains("`layer`") && e.contains("42"), "{e}");
    assert_eq!(s.image(&id).document(), &before);
}

#[test]
fn image_only_merges_on_a_pixel_document_fail_naming_the_mode() {
    let mut s = S::new();
    let id = pixel_tree(&mut s);
    for (tool, args) in [
        ("layer.merge_visible", json!({})),
        ("layer.merge_group", json!({ "layer": 1 })),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args);
        assert!(e.contains("Pixel document"), "{tool}: {e}");
        assert_eq!(s.state(&id), before);
    }
}
