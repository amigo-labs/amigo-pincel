//! Live mode: the tools on a document an editor owns, lent for one call
//! (docs/specs/mcp-live-desktop.md AC-07, AC-16).

use pincel_core::{Bus, CelMap, FrameIndex, LayerId, Palette, Sprite};
use pincel_image_core::Document;
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::StrokeScratch;
use serde_json::{Value, json};

use crate::live::{self, ImageAgentState, ImageLoan, LiveFile, PixelLoan};
use crate::workspace::Doc;

/// The editor's side of a Pixel document.
struct Editor {
    sprite: Sprite,
    cels: CelMap,
    bus: Bus,
    active_layer: Option<LayerId>,
    active_frame: FrameIndex,
}

impl Editor {
    fn new(w: u32, h: u32) -> Self {
        let doc = Doc::new_pixel(w, h, Palette::db32()).expect("doc");
        Self {
            sprite: doc.sprite,
            cels: doc.cels,
            bus: doc.bus,
            active_layer: None,
            active_frame: FrameIndex::new(0),
        }
    }

    fn call(&mut self, name: &str, args: Value, input: Option<LiveFile<'_>>) -> live::LiveReply {
        live::call_pixel(
            "tab-7",
            PixelLoan {
                sprite: &mut self.sprite,
                cels: &mut self.cels,
                bus: &mut self.bus,
                active_layer: &mut self.active_layer,
                active_frame: &mut self.active_frame,
            },
            input,
            name,
            args,
        )
    }

    fn ok(&mut self, name: &str, args: Value) -> Value {
        match self.call(name, args, None).result {
            Ok(out) => out.json,
            Err(e) => panic!("{name} failed: {e}"),
        }
    }
}

/// The editor's side of an Image document.
struct ImageEditor {
    bus: CommandBus,
    scratch: StrokeScratch,
    state: ImageAgentState,
}

impl ImageEditor {
    fn new(w: u32, h: u32) -> Self {
        Self {
            bus: CommandBus::new(Document::new(w, h).expect("doc")),
            scratch: StrokeScratch::default(),
            state: ImageAgentState::default(),
        }
    }

    fn call(&mut self, name: &str, args: Value) -> live::LiveReply {
        live::call_image(
            "tab-2",
            ImageLoan {
                bus: &mut self.bus,
                scratch: &mut self.scratch,
                state: &mut self.state,
            },
            None,
            name,
            args,
        )
    }

    fn ok(&mut self, name: &str, args: Value) -> Value {
        match self.call(name, args).result {
            Ok(out) => out.json,
            Err(e) => panic!("{name} failed: {e}"),
        }
    }
}

#[test]
fn call_pixel_draws_on_the_lent_document_as_one_undo_step() {
    let mut ed = Editor::new(8, 8);
    let before = (ed.sprite.clone(), ed.cels.clone());
    ed.ok(
        "draw.rect",
        json!({ "x0": 1, "y0": 1, "x1": 5, "y1": 5, "color": "#ff0000", "filled": true }),
    );
    assert_ne!(ed.cels, before.1, "the editor's cels changed");
    assert_eq!(ed.bus.undo_depth(), 1, "one call is one undo step");
    assert!(ed.bus.undo(&mut ed.sprite, &mut ed.cels));
    assert_eq!((ed.sprite.clone(), ed.cels.clone()), before);
}

#[test]
fn call_pixel_reports_the_hosts_document_id_and_keeps_the_active_layer() {
    let mut ed = Editor::new(4, 4);
    let added = ed.ok("layer.add", json!({ "name": "Ink" }));
    let info = ed.ok("doc.info", json!({}));
    assert_eq!(info["doc"], "tab-7");
    assert_eq!(
        info["doc"],
        ed.ok("doc.info", json!({ "doc": "tab-7" }))["doc"]
    );
    let id = added["layer"].as_u64().expect("layer id");
    assert_eq!(ed.active_layer.map(|l| u64::from(l.0)), Some(id));
}

#[test]
fn call_pixel_refuses_the_tools_the_desktop_app_serves_itself() {
    let mut ed = Editor::new(4, 4);
    for tool in ["doc.new", "doc.open", "doc.close", "doc.list", "doc.save"] {
        let reply = ed.call(tool, json!({}), None);
        let e = reply.result.expect_err(tool).to_string();
        assert!(e.contains("desktop app"), "{tool}: {e}");
    }
    assert_eq!(ed.bus.undo_depth(), 0);
}

#[test]
fn call_pixel_export_hands_the_file_to_the_host_instead_of_writing_it() {
    let mut ed = Editor::new(4, 4);
    let reply = ed.call("export.png", json!({ "path": "out/sprite.png" }), None);
    let out = reply.result.as_ref().expect("export");
    assert_eq!(reply.written.len(), 1);
    assert_eq!(reply.written[0].0, "out/sprite.png");
    assert!(reply.written[0].1.starts_with(b"\x89PNG"));
    let wire = reply.to_json();
    assert_eq!(wire["ok"], true);
    assert_eq!(wire["json"], out.json);
    assert_eq!(wire["files"][0]["path"], "out/sprite.png");
    let data = wire["files"][0]["data"].as_str().expect("base64");
    assert_eq!(
        crate::base64::decode(data).expect("decode"),
        reply.written[0].1
    );
}

#[test]
fn call_pixel_reads_only_the_file_the_user_picked() {
    let mut ed = Editor::new(4, 4);
    let gpl = b"GIMP Palette\n10 20 30 Dark\n200 100 0\n";
    let reply = ed.call(
        "palette.import",
        json!({ "path": "p.gpl" }),
        Some(LiveFile {
            path: "p.gpl",
            bytes: gpl,
        }),
    );
    assert!(reply.result.is_ok(), "{:?}", reply.result);
    assert_eq!(ed.sprite.palette.len(), 2);
    let reply = ed.call("palette.import", json!({ "path": "other.gpl" }), None);
    assert!(reply.result.is_err());
}

#[test]
fn call_pixel_failure_and_unknown_tool_map_to_the_wire_shape() {
    let mut ed = Editor::new(4, 4);
    let wire = ed.call("draw.spray", json!({}), None).to_json();
    assert_eq!(wire["ok"], false);
    assert_eq!(wire["unknown"], true);
    let wire = ed
        .call(
            "draw.pixels",
            json!({ "pixels": [{ "x": 9, "y": 9, "color": "#fff" }] }),
            None,
        )
        .to_json();
    assert_eq!(wire["ok"], false);
    assert_eq!(wire["unknown"], Value::Null);
    assert!(wire["error"].as_str().is_some_and(|e| !e.is_empty()));
}

#[test]
fn call_image_draws_on_the_lent_bus_as_one_undo_step() {
    let mut ed = ImageEditor::new(16, 8);
    let before = ed.bus.document.clone();
    ed.ok(
        "draw.stroke",
        json!({ "points": [[1, 1], [12, 6]], "size": 3, "color": "#00ff00" }),
    );
    assert_ne!(ed.bus.document, before);
    assert_eq!(ed.bus.history.undo_depth(), 1);
    ed.ok(
        "draw.stroke",
        json!({ "points": [[2, 6], [12, 1]], "size": 3, "color": "#0000ff" }),
    );
    assert_eq!(
        ed.bus.history.undo_depth(),
        2,
        "two calls never merge into one step"
    );
    assert!(ed.bus.undo().expect("undo"));
    assert!(ed.bus.undo().expect("undo"));
    assert_eq!(ed.bus.document, before);
}

#[test]
fn call_image_keeps_agent_layer_ids_across_calls() {
    let mut ed = ImageEditor::new(4, 4);
    let a = ed.ok("layer.add", json!({ "name": "A" }))["layer"].clone();
    let b = ed.ok("layer.add", json!({ "name": "B" }))["layer"].clone();
    ed.ok("layer.remove", json!({ "layer": a }));
    let list = ed.ok("layer.list", json!({}));
    let ids: Vec<Value> = list["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| l["id"].clone())
        .collect();
    assert!(ids.contains(&b), "{list}");
    assert!(!ids.contains(&a), "{list}");
    assert_eq!(ed.ok("doc.info", json!({}))["doc"], "tab-2");
}

#[test]
fn image_agent_state_starts_stroke_ids_far_from_the_editors() {
    let mut ed = ImageEditor::new(8, 8);
    // The editor's own strokes count up from 1; an agent stroke with the
    // same id would merge into the user's last stroke.
    ed.ok(
        "draw.stroke",
        json!({ "points": [[1, 1], [6, 6]], "size": 2, "color": "#ff0000" }),
    );
    assert!(ed.state.last_stroke() >= live::LIVE_STROKE_BASE);
}

#[test]
fn call_wire_parses_arguments_and_reports_malformed_json() {
    let mut ed = Editor::new(4, 4);
    let loan = |ed: &mut Editor| {
        let reply = live::call_pixel_wire(
            "tab-1",
            PixelLoan {
                sprite: &mut ed.sprite,
                cels: &mut ed.cels,
                bus: &mut ed.bus,
                active_layer: &mut ed.active_layer,
                active_frame: &mut ed.active_frame,
            },
            None,
            "doc.info",
            "",
        );
        serde_json::from_str::<Value>(&reply).expect("json")
    };
    let wire = loan(&mut ed);
    assert_eq!(wire["ok"], true);
    assert_eq!(wire["json"]["doc"], "tab-1");
    let mut img = ImageEditor::new(4, 4);
    let reply = live::call_image_wire(
        "tab-2",
        ImageLoan {
            bus: &mut img.bus,
            scratch: &mut img.scratch,
            state: &mut img.state,
        },
        None,
        "doc.info",
        "{not json",
    );
    let wire: Value = serde_json::from_str(&reply).expect("json");
    assert_eq!(wire["ok"], false);
    assert!(wire["error"].as_str().is_some_and(|e| e.contains("JSON")));
}
