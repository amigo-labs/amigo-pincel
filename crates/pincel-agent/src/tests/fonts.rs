//! `font.list` and the `font` argument of `draw.text` over a fake host font
//! provider with fixture faces (system-fonts-mcp AC-01 … AC-08, AC-10,
//! AC-13).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pincel_core::{
    Bus, Cel, ColorMode, DrawText, FrameIndex, LayerId, PixelBuffer, Rgba, TextAlign, TextStyle,
    layer_canvas,
};
use pincel_image_core::FontFace;
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use serde_json::{Value, json};

use super::{MemHost, S};
use crate::image::FONT;
use crate::{FontFaceInfo, FontProvider, Workspace, list_tools};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/../pincel-core/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// How the fake host answers a read of one face.
type Read = Result<(Vec<u8>, u32), String>;

/// Host fonts from fixture files; counts every face read.
struct FakeFonts {
    faces: Vec<FontFaceInfo>,
    files: HashMap<u64, Read>,
    reads: Arc<Mutex<HashMap<u64, usize>>>,
}

impl FontProvider for FakeFonts {
    fn faces(&mut self) -> Vec<FontFaceInfo> {
        self.faces.clone()
    }

    fn read(&mut self, id: u64) -> Result<(Vec<u8>, u32), String> {
        *self.reads.lock().expect("reads").entry(id).or_default() += 1;
        self.files
            .get(&id)
            .cloned()
            .unwrap_or_else(|| Err(format!("no face {id}")))
    }
}

fn info(id: u64, family: &str, style: &str, weight: u16, italic: bool) -> FontFaceInfo {
    FontFaceInfo {
        id,
        family: family.to_owned(),
        style: style.to_owned(),
        weight,
        italic,
        stretch: 5,
    }
}

const SERIF: &str = "Pincel Test Serif";

/// The fake host's faces: (id, family, style, weight, italic, read).
fn host_faces() -> Vec<(FontFaceInfo, Read)> {
    let ok = |name: &str, index: u32| Ok((fixture(name), index));
    vec![
        // Listed out of order on purpose: font.list sorts.
        (
            info(4, SERIF, "Bold Italic", 700, true),
            ok("PincelTestSerif-BoldItalic.ttf", 0),
        ),
        (
            info(1, SERIF, "Regular", 400, false),
            ok("PincelTestSerif-Regular.ttf", 0),
        ),
        // The Bold face lives in a collection, as face 1 (system-fonts AC-08).
        (
            info(2, SERIF, "Bold", 700, false),
            ok("PincelTestSerif.ttc", 1),
        ),
        (
            info(3, SERIF, "Italic", 400, true),
            ok("PincelTestSerif-Italic.ttf", 0),
        ),
        (
            info(5, "Serif Upright", "Regular", 400, false),
            ok("PincelTestSerif-Regular.ttf", 0),
        ),
        // An installed Liberation Sans: its Regular never replaces the
        // bundled face, its Bold serves Bold (system-fonts AC-23). The
        // bytes are a stand-in serif so the two are told apart.
        (
            info(6, "Liberation Sans", "Bold", 700, false),
            ok("PincelTestSerif-Bold.ttf", 0),
        ),
        (
            info(7, "Liberation Sans", "Regular", 400, false),
            ok("PincelTestSerif-Regular.ttf", 0),
        ),
        (
            info(8, "Arial", "Regular", 400, false),
            ok("PincelTestSerif-Regular.ttf", 0),
        ),
        (
            info(9, "ARIAL", "Regular", 400, false),
            ok("PincelTestSerif-Bold.ttf", 0),
        ),
        (
            info(10, "alpha Sans", "Light", 300, false),
            ok("PincelTestSerif-Regular.ttf", 0),
        ),
        (
            info(11, "Broken Font", "Regular", 400, false),
            Ok((b"not a font file".to_vec(), 0)),
        ),
        (
            info(12, "Gone Font", "Regular", 400, false),
            Err("file not found".to_owned()),
        ),
    ]
}

/// A session over the fake host fonts and the read counter.
fn session() -> (S, Arc<Mutex<HashMap<u64, usize>>>) {
    let reads = Arc::new(Mutex::new(HashMap::new()));
    let (faces, files): (Vec<_>, Vec<_>) = host_faces().into_iter().unzip();
    let files = faces.iter().map(|f| f.id).zip(files).collect();
    let fonts = FakeFonts {
        faces,
        files,
        reads: Arc::clone(&reads),
    };
    let s = S {
        ws: Workspace::with_fonts(fonts),
        host: MemHost::default(),
    };
    (s, reads)
}

fn total_reads(reads: &Arc<Mutex<HashMap<u64, usize>>>) -> usize {
    reads.lock().expect("reads").values().sum()
}

/// The pixels of Image layer 0 after the editor's Text tool draws with
/// face `index` of `bytes`.
fn image_editor(s: &S, id: &str, bytes: &[u8], index: u32, cmd: Value) -> Vec<u8> {
    let mut bus = CommandBus::new(s.image(id).document().clone());
    let spec: CommandSpec = serde_json::from_value(cmd).expect("valid CommandSpec");
    run_command(
        &mut bus,
        spec,
        &mut StrokeScratch::default(),
        &[FontFace::new(bytes, index)],
    )
    .expect("runs");
    bus.document.layers()[0].pixels.data().to_vec()
}

fn image_pixels(s: &S, id: &str) -> Vec<u8> {
    s.image(id).document().layers()[0].pixels.data().to_vec()
}

/// The editor's `draw_text` command for "Ag" at (2, 2), size 24.
fn image_cmd(bold: bool, italic: bool) -> Value {
    json!({
        "type": "draw_text", "layer": 0, "font_id": 0, "text": "Ag", "x": 2, "y": 2,
        "size": 24, "color": [0, 0, 0, 255], "bold": bold, "italic": italic,
        "anti_alias": true, "align": "left"
    })
}

fn text_args(font: Option<&str>, bold: bool, italic: bool) -> Value {
    let mut v = json!({ "text": "Ag", "x": 2, "y": 2, "size": 24, "bold": bold, "italic": italic });
    if let Some(f) = font {
        v["font"] = json!(f);
    }
    v
}

/// Layer 0 at frame 0 of a Pixel document after the Pixel editor's Text
/// dialog places "Ag" at (1, 1), size 16, with face `index` of `bytes`.
fn pixel_editor(s: &S, id: &str, bytes: &[u8], index: u32, bold: bool, italic: bool) -> Vec<u8> {
    let (l0, f0) = (LayerId(0), FrameIndex(0));
    let (mut sprite, mut cels) = s.state(id);
    if cels.get(l0, f0).is_none() {
        let (w, h) = (sprite.width, sprite.height);
        cels.insert(Cel::image(
            l0,
            f0,
            PixelBuffer::empty(w, h, ColorMode::Rgba),
        ));
    }
    let style = TextStyle {
        size: 16.0,
        color: Rgba::BLACK,
        bold,
        italic,
        anti_alias: false,
        align: TextAlign::Left,
    };
    let cmd = DrawText::new(l0, f0, (1, 1), "Ag", bytes.to_vec(), style).with_face_index(index);
    Bus::new()
        .execute(cmd.into(), &mut sprite, &mut cels)
        .expect("editor command");
    layer_canvas(&sprite, &cels, l0, f0).data
}

fn pixel_canvas(s: &S, id: &str) -> Vec<u8> {
    let d = s.doc(id);
    layer_canvas(&d.sprite, &d.cels, LayerId(0), FrameIndex(0)).data
}

fn pixel_args(font: Option<&str>, bold: bool, italic: bool) -> Value {
    let mut v = json!({ "text": "Ag", "x": 1, "y": 1, "size": 16, "bold": bold, "italic": italic });
    if let Some(f) = font {
        v["font"] = json!(f);
    }
    v
}

fn names(list: &Value) -> Vec<String> {
    list["families"]
        .as_array()
        .expect("families")
        .iter()
        .map(|f| f["name"].as_str().expect("name").to_owned())
        .collect()
}

#[test]
fn list_tools_has_font_list_and_the_font_argument() {
    let tools = list_tools();
    let list = tools
        .iter()
        .find(|t| t.name == "font.list")
        .expect("font.list is listed");
    assert!(!list.description.is_empty());
    let props = &list.input_schema["properties"];
    assert_eq!(props["query"]["type"], "string");
    let required = list.input_schema["required"].as_array().expect("required");
    assert!(!required.iter().any(|r| r == "query"), "query is optional");

    let text = tools
        .iter()
        .find(|t| t.name == "draw.text")
        .expect("draw.text");
    assert_eq!(text.input_schema["properties"]["font"]["type"], "string");
    let required = text.input_schema["required"].as_array().expect("required");
    assert!(!required.iter().any(|r| r == "font"), "font is optional");
}

#[test]
fn font_list_lists_each_family_once_sorted_with_its_styles() {
    let (mut s, _) = session();
    let out = s.ok("font.list", json!({}));
    assert_eq!(
        out,
        json!({ "families": [
            { "name": "alpha Sans", "styles": ["Light"] },
            { "name": "ARIAL", "styles": ["Regular"] },
            { "name": "Arial", "styles": ["Regular"] },
            { "name": "Broken Font", "styles": ["Regular"] },
            { "name": "Gone Font", "styles": ["Regular"] },
            { "name": "Liberation Sans", "styles": ["Regular", "Bold"] },
            { "name": SERIF, "styles": ["Regular", "Italic", "Bold", "Bold Italic"] },
            { "name": "Serif Upright", "styles": ["Regular"] },
        ] })
    );
}

#[test]
fn font_list_without_host_fonts_lists_liberation_sans() {
    let mut s = S::new();
    assert_eq!(
        s.ok("font.list", json!({})),
        json!({ "families": [{ "name": "Liberation Sans", "styles": ["Regular"] }] })
    );
}

#[test]
fn font_list_query_filters_by_name_ignoring_case() {
    let (mut s, _) = session();
    let out = s.ok("font.list", json!({ "query": "SERIF" }));
    assert_eq!(names(&out), vec![SERIF, "Serif Upright"]);
    assert_eq!(
        out["families"][0]["styles"],
        json!(["Regular", "Italic", "Bold", "Bold Italic"])
    );
    let out = s.ok("font.list", json!({ "query": "ari" }));
    assert_eq!(names(&out), vec!["ARIAL", "Arial"]);
    let all = s.ok("font.list", json!({}));
    assert_eq!(s.ok("font.list", json!({ "query": "" })), all);
    assert_eq!(
        names(&s.ok("font.list", json!({ "query": "zzz" }))).len(),
        0
    );
}

#[test]
fn draw_text_without_font_or_with_regular_liberation_sans_uses_the_bundled_face() {
    let (mut s, reads) = session();
    for (font, bold, italic) in [
        (None, false, false),
        (None, true, false),
        (None, true, true),
        (Some("Liberation Sans"), false, false),
    ] {
        let id = s.new_image(80, 40);
        // Before this spec: the bundled face, Bold / Italic synthesized.
        let want = image_editor(&s, &id, FONT, 0, image_cmd(bold, italic));
        s.ok("draw.text", text_args(font, bold, italic));
        assert_eq!(
            image_pixels(&s, &id),
            want,
            "{font:?} bold {bold} italic {italic}"
        );

        let id = s.new_doc(64, 32);
        let want = pixel_editor(&s, &id, FONT, 0, bold, italic);
        s.ok("draw.text", pixel_args(font, bold, italic));
        assert_eq!(
            pixel_canvas(&s, &id),
            want,
            "Pixel {font:?} {bold} {italic}"
        );
    }
    assert_eq!(total_reads(&reads), 0, "the bundled face is no host read");
}

#[test]
fn draw_text_with_a_family_matches_the_editor_on_image_documents() {
    let (mut s, _) = session();
    // (font, bold, italic) → the face the editor picks and what it
    // synthesizes (system-fonts AC-07, AC-23).
    let cases: [(&str, bool, bool, &str, u32, bool, bool); 7] = [
        (
            SERIF,
            false,
            false,
            "PincelTestSerif-Regular.ttf",
            0,
            false,
            false,
        ),
        (SERIF, true, false, "PincelTestSerif.ttc", 1, false, false),
        (
            SERIF,
            false,
            true,
            "PincelTestSerif-Italic.ttf",
            0,
            false,
            false,
        ),
        (
            SERIF,
            true,
            true,
            "PincelTestSerif-BoldItalic.ttf",
            0,
            false,
            false,
        ),
        (
            "Serif Upright",
            true,
            true,
            "PincelTestSerif-Regular.ttf",
            0,
            true,
            true,
        ),
        (
            "Liberation Sans",
            true,
            false,
            "PincelTestSerif-Bold.ttf",
            0,
            false,
            false,
        ),
        (
            "Liberation Sans",
            false,
            true,
            "LiberationSans-Regular.ttf",
            0,
            false,
            true,
        ),
    ];
    for (font, bold, italic, file, index, synth_bold, synth_italic) in cases {
        let id = s.new_image(80, 40);
        let want = image_editor(
            &s,
            &id,
            &fixture(file),
            index,
            image_cmd(synth_bold, synth_italic),
        );
        s.ok("draw.text", text_args(Some(font), bold, italic));
        let got = image_pixels(&s, &id);
        assert!(got.chunks(4).any(|p| p[3] > 0), "{font}: text was drawn");
        assert_eq!(got, want, "{font} bold {bold} italic {italic}");
    }
}

#[test]
fn draw_text_with_a_family_matches_the_editor_on_pixel_documents() {
    let (mut s, _) = session();
    let cases: [(&str, bool, bool, &str, u32, bool, bool); 4] = [
        (
            SERIF,
            false,
            false,
            "PincelTestSerif-Regular.ttf",
            0,
            false,
            false,
        ),
        (SERIF, true, false, "PincelTestSerif.ttc", 1, false, false),
        (
            SERIF,
            true,
            true,
            "PincelTestSerif-BoldItalic.ttf",
            0,
            false,
            false,
        ),
        (
            "Serif Upright",
            false,
            true,
            "PincelTestSerif-Regular.ttf",
            0,
            false,
            true,
        ),
    ];
    for (font, bold, italic, file, index, synth_bold, synth_italic) in cases {
        let id = s.new_doc(64, 32);
        let want = pixel_editor(&s, &id, &fixture(file), index, synth_bold, synth_italic);
        s.ok("draw.text", pixel_args(Some(font), bold, italic));
        let got = pixel_canvas(&s, &id);
        assert!(got.chunks(4).any(|p| p[3] > 0), "{font}: text was drawn");
        assert_eq!(got, want, "Pixel {font} bold {bold} italic {italic}");
    }
}

#[test]
fn draw_text_font_matches_one_family_ignoring_case() {
    let (mut s, _) = session();
    let id = s.new_image(80, 40);
    s.ok("draw.text", text_args(Some(SERIF), true, false));
    let exact = image_pixels(&s, &id);
    let id = s.new_image(80, 40);
    s.ok(
        "draw.text",
        text_args(Some("pincel TEST serif"), true, false),
    );
    assert_eq!(image_pixels(&s, &id), exact);
}

#[test]
fn draw_text_ambiguous_font_fails_listing_every_match() {
    let (mut s, _) = session();
    let id = s.new_image(80, 40);
    let before = image_pixels(&s, &id);
    let depth = s.image(&id).bus.history.undo_depth();
    let e = s.err("draw.text", text_args(Some("arial"), false, false));
    assert_eq!(
        e,
        "font \"arial\" is ambiguous: ARIAL, Arial; pass the exact name"
    );
    assert_eq!(image_pixels(&s, &id), before);
    assert_eq!(s.image(&id).bus.history.undo_depth(), depth);
    // The exact name picks one family.
    s.ok("draw.text", text_args(Some("Arial"), false, false));
    let pid = s.new_doc(32, 32);
    let e = s.err("draw.text", pixel_args(Some("ARIAl"), false, false));
    assert!(e.contains("is ambiguous: ARIAL, Arial"), "{e}");
    assert_eq!(s.ok("doc.info", json!({ "doc": pid }))["undo_depth"], 0);
}

#[test]
fn draw_text_unknown_or_empty_font_fails_and_points_to_font_list() {
    let (mut s, _) = session();
    let id = s.new_image(80, 40);
    let before = image_pixels(&s, &id);
    for font in ["Comic Nope", ""] {
        let e = s.err("draw.text", text_args(Some(font), false, false));
        assert!(e.contains(&format!("font \"{font}\"")), "{e}");
        assert!(e.contains("font.list"), "{e}");
    }
    assert_eq!(image_pixels(&s, &id), before);
    assert_eq!(s.image(&id).bus.history.undo_depth(), 0);
    let pid = s.new_doc(32, 32);
    let e = s.err("draw.text", pixel_args(Some("Comic Nope"), false, false));
    assert!(
        e.contains("\"Comic Nope\"") && e.contains("font.list"),
        "{e}"
    );
    assert_eq!(s.ok("doc.info", json!({ "doc": pid }))["undo_depth"], 0);
}

#[test]
fn draw_text_unreadable_face_fails_naming_family_and_reason() {
    let (mut s, _) = session();
    let id = s.new_image(80, 40);
    let before = image_pixels(&s, &id);
    let e = s.err("draw.text", text_args(Some("Broken Font"), false, false));
    assert!(e.contains("Broken Font") && e.contains("parse"), "{e}");
    let e = s.err("draw.text", text_args(Some("Gone Font"), false, false));
    assert!(
        e.contains("Gone Font") && e.contains("file not found"),
        "{e}"
    );
    assert_eq!(image_pixels(&s, &id), before);
    assert_eq!(s.image(&id).bus.history.undo_depth(), 0);
    let pid = s.new_doc(32, 32);
    let e = s.err("draw.text", pixel_args(Some("Broken Font"), false, false));
    assert!(e.contains("Broken Font"), "{e}");
    assert_eq!(s.ok("doc.info", json!({ "doc": pid }))["undo_depth"], 0);
}

#[test]
fn font_list_reads_no_face_and_draw_text_reads_each_face_once() {
    let (mut s, reads) = session();
    s.ok("font.list", json!({}));
    s.ok("font.list", json!({ "query": "serif" }));
    assert_eq!(total_reads(&reads), 0, "font.list reads no face");

    s.new_image(80, 40);
    s.ok("draw.text", text_args(Some(SERIF), false, false));
    s.ok("draw.text", text_args(Some(SERIF), false, false));
    s.new_doc(64, 32);
    s.ok("draw.text", pixel_args(Some(SERIF), false, false));
    s.ok("draw.text", pixel_args(Some(SERIF), true, false));
    s.ok("draw.text", pixel_args(Some(SERIF), true, false));
    let r = reads.lock().expect("reads").clone();
    assert_eq!(r.get(&1), Some(&1), "Regular read once: {r:?}");
    assert_eq!(r.get(&2), Some(&1), "Bold read once: {r:?}");
    assert_eq!(r.values().sum::<usize>(), 2, "only the used faces: {r:?}");
    // A failed read is not retried either.
    s.err("draw.text", pixel_args(Some("Gone Font"), false, false));
    s.err("draw.text", pixel_args(Some("Gone Font"), false, false));
    assert_eq!(reads.lock().expect("reads").get(&12), Some(&1));
}

/// One row of the face-choice table both implementations check: the
/// editor's `faces.ts` (ui/e2e/system-fonts-mcp.spec.ts) and this crate.
#[test]
fn face_choice_cases_match_the_shared_table() {
    let table: Value =
        serde_json::from_str(include_str!("../face-choice-cases.json")).expect("table");
    let cases = table["cases"].as_array().expect("cases");
    assert!(cases.len() >= 10, "the table covers the rules");
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let family = case["family"].as_str().expect("family");
        let faces: Vec<FontFaceInfo> = case["faces"]
            .as_array()
            .expect("faces")
            .iter()
            .enumerate()
            .map(|(i, f)| FontFaceInfo {
                id: i as u64,
                family: family.to_owned(),
                style: f["style"].as_str().expect("style").to_owned(),
                weight: f["weight"].as_u64().expect("weight") as u16,
                italic: f["italic"].as_bool().expect("italic"),
                stretch: f["stretch"].as_u64().unwrap_or(5) as u16,
            })
            .collect();
        let bold = case["bold"].as_bool().expect("bold");
        let italic = case["italic"].as_bool().expect("italic");
        let got = crate::fonts::choose(family, &faces, bold, italic).expect(name);
        let style = match got.face {
            Some(f) => f.style.clone(),
            None => "bundled".to_owned(),
        };
        let want = &case["want"];
        assert_eq!(
            (style.as_str(), got.synth_bold, got.synth_italic),
            (
                want["style"].as_str().expect("want style"),
                want["synth_bold"].as_bool().expect("synth_bold"),
                want["synth_italic"].as_bool().expect("synth_italic"),
            ),
            "{name}"
        );
    }
}
