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
            None,
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
        None,
        "doc.info",
        "{not json",
    );
    let wire: Value = serde_json::from_str(&reply).expect("json");
    assert_eq!(wire["ok"], false);
    assert!(wire["error"].as_str().is_some_and(|e| e.contains("JSON")));
}

// --- The editor's fonts (docs/specs/system-fonts-mcp.md AC-11) -------------

fn font_fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../pincel-core/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The webview's font list as it hands it over: Pincel Test Serif with its
/// Regular and Bold (face 1 of the collection) and Italic faces, plus the
/// faces it read for this call.
fn wire_fonts(read: Value) -> String {
    json!({
        "faces": [
            { "family": "Pincel Test Serif", "style": "Regular", "weight": 400, "italic": false, "stretch": 5 },
            { "family": "Pincel Test Serif", "style": "Bold", "weight": 700, "italic": false, "stretch": 5 },
            { "family": "Pincel Test Serif", "style": "Italic", "weight": 400, "italic": true, "stretch": 5 },
        ],
        "read": read,
    })
    .to_string()
}

/// The core's font registry in the test: id 9 is the collection's Bold.
fn registered(id: u32) -> Result<(Vec<u8>, u32), String> {
    match id {
        9 => Ok((font_fixture("PincelTestSerif.ttc"), 1)),
        _ => Err(format!("unknown font id {id}")),
    }
}

fn image_wire(ed: &mut ImageEditor, fonts: Option<&str>, name: &str, args: Value) -> Value {
    let reply = live::call_image_wire(
        "tab-2",
        ImageLoan {
            bus: &mut ed.bus,
            scratch: &mut ed.scratch,
            state: &mut ed.state,
        },
        None,
        fonts.map(|json| live::WireFonts {
            json,
            registered: &registered,
        }),
        name,
        &args.to_string(),
    );
    serde_json::from_str(&reply).expect("json")
}

fn bold_text() -> Value {
    json!({ "text": "Ag", "x": 2, "y": 2, "size": 24, "font": "Pincel Test Serif", "bold": true })
}

#[test]
fn call_image_wire_draws_text_with_the_face_the_editor_read() {
    let mut ed = ImageEditor::new(48, 32);
    let fonts = wire_fonts(json!([{ "face": 1, "font": 9 }]));
    let wire = image_wire(&mut ed, Some(&fonts), "draw.text", bold_text());
    assert_eq!(wire["ok"], true, "{wire}");
    // The editor's Text tool with the same face.
    let mut reference = CommandBus::new(Document::new(48, 32).expect("doc"));
    let spec: pincel_image_core::command::spec::CommandSpec = serde_json::from_value(json!({
        "type": "draw_text", "layer": 0, "font_id": 0, "text": "Ag", "x": 2, "y": 2,
        "size": 24, "color": [0, 0, 0, 255], "bold": false, "italic": false,
        "anti_alias": true, "align": "left"
    }))
    .expect("spec");
    let ttc = font_fixture("PincelTestSerif.ttc");
    pincel_image_core::command::spec::run_command(
        &mut reference,
        spec,
        &mut StrokeScratch::default(),
        &[pincel_image_core::FontFace::new(&ttc, 1)],
    )
    .expect("runs");
    let drawn = ed.bus.document.layers()[0].pixels.data().to_vec();
    assert!(drawn.iter().any(|&v| v != 0), "text was drawn");
    assert_eq!(drawn, reference.document.layers()[0].pixels.data());
}

#[test]
fn call_pixel_wire_draws_text_with_the_face_the_editor_read() {
    let mut ed = Editor::new(48, 32);
    let fonts = wire_fonts(json!([{ "face": 1, "font": 9 }]));
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
        Some(live::WireFonts {
            json: &fonts,
            registered: &registered,
        }),
        "draw.text",
        &json!({ "text": "Ag", "x": 1, "y": 1, "size": 16, "font": "Pincel Test Serif", "bold": true }).to_string(),
    );
    let wire: Value = serde_json::from_str(&reply).expect("json");
    assert_eq!(wire["ok"], true, "{wire}");
    // The Pixel editor's Text dialog with the same face.
    let mut reference = Editor::new(48, 32);
    let (l0, f0) = (LayerId(0), FrameIndex(0));
    let style = pincel_core::TextStyle {
        size: 16.0,
        color: pincel_core::Rgba::BLACK,
        bold: false,
        italic: false,
        anti_alias: false,
        align: pincel_core::TextAlign::Left,
    };
    if reference.cels.get(l0, f0).is_none() {
        reference.cels.insert(pincel_core::Cel::image(
            l0,
            f0,
            pincel_core::PixelBuffer::empty(48, 32, pincel_core::ColorMode::Rgba),
        ));
    }
    let cmd = pincel_core::DrawText::new(
        l0,
        f0,
        (1, 1),
        "Ag",
        font_fixture("PincelTestSerif.ttc"),
        style,
    )
    .with_face_index(1);
    Bus::new()
        .execute(cmd.into(), &mut reference.sprite, &mut reference.cels)
        .expect("editor command");
    let canvas = |e: &Editor| pincel_core::layer_canvas(&e.sprite, &e.cels, l0, f0).data;
    let drawn = canvas(&ed);
    assert!(drawn.iter().any(|&v| v != 0), "text was drawn");
    assert_eq!(drawn, canvas(&reference));
}

#[test]
fn call_wire_fonts_report_a_face_the_editor_could_not_read() {
    let mut ed = ImageEditor::new(48, 32);
    // The webview's read failed: its reason reaches the agent (AC-08).
    let failed = wire_fonts(json!([{ "face": 1, "error": "the font file is gone" }]));
    let wire = image_wire(&mut ed, Some(&failed), "draw.text", bold_text());
    assert_eq!(wire["ok"], false);
    let error = wire["error"].as_str().expect("error");
    assert!(error.contains("Pincel Test Serif"), "{error}");
    assert!(error.contains("the font file is gone"), "{error}");
    // A face the webview did not read, or an id the registry does not know.
    for read in [json!([]), json!([{ "face": 1, "font": 3 }])] {
        let fonts = wire_fonts(read);
        let wire = image_wire(&mut ed, Some(&fonts), "draw.text", bold_text());
        assert_eq!(wire["ok"], false, "{wire}");
        assert!(
            wire["error"]
                .as_str()
                .is_some_and(|e| e.contains("Pincel Test Serif"))
        );
    }
    // Malformed font lists are refused.
    let wire = image_wire(&mut ed, Some("{\"faces\": 3}"), "draw.text", bold_text());
    assert_eq!(wire["ok"], false);
    assert!(
        wire["error"]
            .as_str()
            .is_some_and(|e| e.contains("font list")),
        "{wire}"
    );
    assert_eq!(ed.bus.history.undo_depth(), 0, "nothing was drawn");
}

#[test]
fn call_wire_fonts_feed_font_list_and_matching() {
    let mut ed = ImageEditor::new(8, 8);
    let fonts = wire_fonts(json!([]));
    let wire = image_wire(&mut ed, Some(&fonts), "font.list", json!({}));
    assert_eq!(
        wire["json"],
        json!({ "families": [
            { "name": "Liberation Sans", "styles": ["Regular"] },
            { "name": "Pincel Test Serif", "styles": ["Regular", "Italic", "Bold"] },
        ] })
    );
    // Without the editor's fonts only the bundled family is known.
    let wire = image_wire(&mut ed, None, "draw.text", bold_text());
    assert_eq!(wire["ok"], false);
    assert!(
        wire["error"]
            .as_str()
            .is_some_and(|e| e.contains("font.list"))
    );
}
