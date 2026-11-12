//! Live mode: the tools on a document the desktop editor owns, lent to them
//! for one call (docs/specs/mcp-live-desktop.md, Decision Log 2026-10-29).
//!
//! The editor's wasm document is moved into a one-document [`Workspace`]
//! under the tab's id, the tool runs exactly as in the headless server, and
//! the document moves back. File access goes to the user instead of a
//! sandbox: a reading tool gets only the file the user picked in the open
//! dialog, and a writing tool's files are handed to the host, which writes
//! them where the user confirms in the save dialog (AC-16).

use std::collections::HashMap;

use pincel_core::{Bus, CelMap, FrameIndex, LayerId, Sprite};
use pincel_image_core::Document;
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::StrokeScratch;
use serde_json::{Value, json};

use crate::image::ImageDoc;
use crate::workspace::{Doc, Entry};
use crate::{
    AgentError, FileHost, FontFaceInfo, FontProvider, ToolOutput, Workspace, base64, call,
};

/// Tools the desktop app serves through its own New, Open, Close and Save
/// flows and its tab list (AC-10, AC-15); a live call refuses them.
pub const SHELL_TOOLS: &[&str] = &["doc.new", "doc.open", "doc.close", "doc.list", "doc.save"];

/// The first stroke id of the agent's Image strokes. The editor numbers its
/// own strokes from 1, and strokes sharing an id merge into one undo step,
/// so the agent's ids start far above them.
pub const LIVE_STROKE_BASE: u64 = 1 << 62;

/// A Pixel document the editor owns, lent to one call.
pub struct PixelLoan<'a> {
    /// The sprite.
    pub sprite: &'a mut Sprite,
    /// Its cels.
    pub cels: &'a mut CelMap,
    /// Its undo bus; a mutating call adds exactly one step.
    pub bus: &'a mut Bus,
    /// The layer the editor selected as paint target.
    pub active_layer: &'a mut Option<LayerId>,
    /// The frame the editor shows.
    pub active_frame: &'a mut FrameIndex,
}

/// What the agent remembers about one Image document between live calls:
/// its stable layer ids and its stroke counter. The host keeps one per
/// document.
#[derive(Debug, Clone)]
pub struct ImageAgentState {
    ids: Vec<(u128, u32)>,
    next_stroke: u64,
}

impl Default for ImageAgentState {
    fn default() -> Self {
        Self {
            ids: Vec::new(),
            next_stroke: LIVE_STROKE_BASE,
        }
    }
}

impl ImageAgentState {
    /// The stroke id of the agent's last stroke on this document
    /// ([`LIVE_STROKE_BASE`] before the first).
    pub fn last_stroke(&self) -> u64 {
        self.next_stroke
    }
}

/// An Image document the editor owns, lent to one call.
pub struct ImageLoan<'a> {
    /// The document and its undo stack; a mutating call adds exactly one
    /// step.
    pub bus: &'a mut CommandBus,
    /// The editor's per-document tool memory.
    pub scratch: &'a mut StrokeScratch,
    /// The agent's state for this document.
    pub state: &'a mut ImageAgentState,
}

/// The file the user picked in the open dialog for a reading tool
/// (`layer.import`, `palette.import`), under the path the call named.
#[derive(Debug, Clone, Copy)]
pub struct LiveFile<'a> {
    /// The `path` argument of the call.
    pub path: &'a str,
    /// The file's content.
    pub bytes: &'a [u8],
}

/// The outcome of one live call.
#[derive(Debug)]
pub struct LiveReply {
    /// The tool's result.
    pub result: Result<ToolOutput, AgentError>,
    /// The files the tool wrote, by the path it named, in write order; the
    /// host saves them where the user confirms.
    pub written: Vec<(String, Vec<u8>)>,
}

impl LiveReply {
    /// The reply as the desktop app passes it on: `{"ok": true, "json",
    /// "png"?, "files": [{"path", "data"}]}` with base64 bytes, or `{"ok":
    /// false, "error", "unknown"?}` (`unknown` for a tool that does not
    /// exist, a protocol error rather than a tool error).
    pub fn to_json(&self) -> Value {
        match &self.result {
            Ok(out) => {
                let files: Vec<Value> = self
                    .written
                    .iter()
                    .map(|(path, bytes)| json!({ "path": path, "data": base64::encode(bytes) }))
                    .collect();
                let mut v = json!({ "ok": true, "json": out.json, "files": files });
                if let Some(png) = &out.image_png {
                    v["png"] = Value::String(base64::encode(png));
                }
                v
            }
            Err(e @ AgentError::UnknownTool(_)) => {
                json!({ "ok": false, "unknown": true, "error": e.to_string() })
            }
            Err(e) => json!({ "ok": false, "error": e.to_string() }),
        }
    }
}

/// The editor's fonts for one live call (docs/specs/system-fonts-mcp.md
/// AC-11): the faces its font list shows and the faces the webview read
/// for this call. In wasm no font file can be read synchronously, so the
/// webview reads the face `draw.text` will use before forwarding the call;
/// a face it did not read cannot be used.
#[derive(Debug, Clone, Default)]
pub struct LiveFonts {
    faces: Vec<FontFaceInfo>,
    read: HashMap<u64, Result<(Vec<u8>, u32), String>>,
}

impl LiveFonts {
    /// The faces of the editor's font list, the bundled face aside; their
    /// ids are what [`LiveFonts::with_read`] names.
    pub fn new(faces: Vec<FontFaceInfo>) -> Self {
        Self {
            faces,
            read: HashMap::new(),
        }
    }

    /// Face `id` as the webview read it: the font file and the face's
    /// index inside it, or why it could not be read.
    pub fn with_read(mut self, id: u64, face: Result<(Vec<u8>, u32), String>) -> Self {
        self.read.insert(id, face);
        self
    }

    /// The fonts as the webview passes them ([`WireFonts`]).
    fn from_wire(fonts: WireFonts<'_>) -> Result<Self, String> {
        let bad = || "the editor's font list is malformed".to_owned();
        let v: Value = serde_json::from_str(fonts.json).map_err(|e| format!("{}: {e}", bad()))?;
        let mut faces = Vec::new();
        for (i, f) in v["faces"].as_array().ok_or_else(bad)?.iter().enumerate() {
            let num = |key: &str| {
                f[key]
                    .as_u64()
                    .and_then(|n| u16::try_from(n).ok())
                    .ok_or_else(bad)
            };
            faces.push(FontFaceInfo {
                id: i as u64,
                family: f["family"].as_str().ok_or_else(bad)?.to_owned(),
                style: f["style"].as_str().ok_or_else(bad)?.to_owned(),
                weight: num("weight")?,
                italic: f["italic"].as_bool().ok_or_else(bad)?,
                stretch: num("stretch")?,
            });
        }
        let mut live = Self::new(faces);
        for r in v["read"].as_array().ok_or_else(bad)? {
            let id = r["face"].as_u64().ok_or_else(bad)?;
            let face = match (r["font"].as_u64(), r["error"].as_str()) {
                (Some(font), _) => u32::try_from(font)
                    .map_err(|_| format!("unknown font id {font}"))
                    .and_then(fonts.registered),
                (None, Some(error)) => Err(error.to_owned()),
                (None, None) => return Err(bad()),
            };
            live = live.with_read(id, face);
        }
        Ok(live)
    }
}

impl FontProvider for LiveFonts {
    fn faces(&mut self) -> Vec<FontFaceInfo> {
        self.faces.clone()
    }

    fn read(&mut self, id: u64) -> Result<(Vec<u8>, u32), String> {
        self.read
            .remove(&id)
            .unwrap_or_else(|| Err("the editor did not read the face".to_owned()))
    }
}

/// The editor's fonts as a wasm bridge passes them: JSON `{"faces":
/// [{"family", "style", "weight", "italic", "stretch"}, …], "read":
/// [{"face", "font"} | {"face", "error"}, …]}`. A face's id is its index in
/// `faces`; `read` names each face the webview read for the call by the id
/// its core's font registry gave it, which `registered` turns into the font
/// file and face index, or says why the face could not be read.
#[derive(Clone, Copy)]
pub struct WireFonts<'a> {
    /// The JSON text.
    pub json: &'a str,
    /// The core's font registry: file bytes and face index of a registered
    /// font id.
    pub registered: &'a dyn Fn(u32) -> Result<(Vec<u8>, u32), String>,
}

/// The session a live call runs in: the editor's fonts, or the bundled face
/// alone.
fn live_workspace(fonts: Option<LiveFonts>) -> Workspace {
    match fonts {
        Some(fonts) => Workspace::with_fonts(fonts),
        None => Workspace::new(),
    }
}

/// File access of a live call: no file system, only the user's choices.
struct LiveHost<'a> {
    input: Option<LiveFile<'a>>,
    written: Vec<(String, Vec<u8>)>,
}

impl FileHost for LiveHost<'_> {
    fn read(&mut self, path: &str) -> Result<Vec<u8>, String> {
        match self.input {
            Some(f) if f.path == path => Ok(f.bytes.to_vec()),
            _ => Err(format!(
                "{path}: in the desktop app the user picks the file in the open dialog"
            )),
        }
    }

    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        self.written.push((path.to_owned(), bytes.to_vec()));
        Ok(())
    }

    fn check_write(&mut self, _path: &str) -> Result<(), String> {
        Ok(())
    }
}

/// The refusal of a tool the desktop app serves itself, if `name` is one.
fn shell_tool(name: &str) -> Option<LiveReply> {
    SHELL_TOOLS.contains(&name).then(|| LiveReply {
        result: Err(AgentError::tool(format!(
            "`{name}` is served by the desktop app, not by the document"
        ))),
        written: Vec::new(),
    })
}

/// Run the tool `name` on a Pixel document the editor owns, under the
/// editor's id `doc_id`, with the editor's `fonts` (the bundled face alone
/// without). A mutating call adds exactly one undo step to `loan.bus`; a
/// failed call leaves the document unchanged.
pub fn call_pixel(
    doc_id: &str,
    loan: PixelLoan<'_>,
    input: Option<LiveFile<'_>>,
    fonts: Option<LiveFonts>,
    name: &str,
    args: Value,
) -> LiveReply {
    if let Some(refusal) = shell_tool(name) {
        return refusal;
    }
    let placeholder = Sprite::builder(1, 1)
        .build()
        .expect("a 1×1 sprite without layers is valid");
    let mut doc = Doc::from_parts(
        std::mem::replace(loan.sprite, placeholder),
        std::mem::take(loan.cels),
        None,
    );
    doc.bus = std::mem::replace(loan.bus, Bus::new());
    doc.active_layer = *loan.active_layer;
    doc.active_frame = *loan.active_frame;
    let mut ws = live_workspace(fonts);
    ws.insert_as(doc_id.to_owned(), Entry::Pixel(doc));
    let mut host = LiveHost {
        input,
        written: Vec::new(),
    };
    let result = call(&mut ws, &mut host, name, args);
    // No tool but the refused shell tools adds or removes a document.
    match ws.remove(doc_id) {
        Some(Entry::Pixel(doc)) => {
            *loan.sprite = doc.sprite;
            *loan.cels = doc.cels;
            *loan.bus = doc.bus;
            *loan.active_layer = doc.active_layer;
            *loan.active_frame = doc.active_frame;
        }
        _ => unreachable!("the lent Pixel document stays in the workspace"),
    }
    LiveReply {
        result,
        written: host.written,
    }
}

/// Run the tool `name` on an Image document the editor owns, under the
/// editor's id `doc_id`, with the editor's `fonts` (the bundled face alone
/// without). A mutating call adds exactly one undo step to `loan.bus`; a
/// failed call leaves the document unchanged.
pub fn call_image(
    doc_id: &str,
    loan: ImageLoan<'_>,
    input: Option<LiveFile<'_>>,
    fonts: Option<LiveFonts>,
    name: &str,
    args: Value,
) -> LiveReply {
    if let Some(refusal) = shell_tool(name) {
        return refusal;
    }
    let placeholder = CommandBus::new(Document::new(1, 1).expect("a 1×1 document is valid"));
    let doc = ImageDoc::from_live(
        std::mem::replace(loan.bus, placeholder),
        std::mem::take(loan.scratch),
        std::mem::take(&mut loan.state.ids),
        loan.state.next_stroke,
    );
    let mut ws = live_workspace(fonts);
    ws.insert_as(doc_id.to_owned(), Entry::Image(doc));
    let mut host = LiveHost {
        input,
        written: Vec::new(),
    };
    let result = call(&mut ws, &mut host, name, args);
    match ws.remove(doc_id) {
        Some(Entry::Image(doc)) => {
            let (bus, scratch, ids, next_stroke) = doc.into_live();
            *loan.bus = bus;
            *loan.scratch = scratch;
            loan.state.ids = ids;
            loan.state.next_stroke = next_stroke;
        }
        _ => unreachable!("the lent Image document stays in the workspace"),
    }
    LiveReply {
        result,
        written: host.written,
    }
}

/// A failed call that never reached a tool.
fn refused(message: String) -> LiveReply {
    LiveReply {
        result: Err(AgentError::tool(message)),
        written: Vec::new(),
    }
}

/// Parse a call's arguments and fonts as the wasm bridges pass them:
/// arguments as a JSON object, or an empty string for none.
fn parse_wire(
    args_json: &str,
    fonts: Option<WireFonts<'_>>,
) -> Result<(Value, Option<LiveFonts>), LiveReply> {
    let args = if args_json.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(args_json)
            .map_err(|e| refused(format!("arguments are not valid JSON: {e}")))?
    };
    let fonts = fonts
        .map(LiveFonts::from_wire)
        .transpose()
        .map_err(refused)?;
    Ok((args, fonts))
}

/// [`call_pixel`] for the wasm bridge: arguments, the editor's fonts and the
/// reply as JSON text ([`WireFonts`], [`LiveReply::to_json`]).
pub fn call_pixel_wire(
    doc_id: &str,
    loan: PixelLoan<'_>,
    input: Option<LiveFile<'_>>,
    fonts: Option<WireFonts<'_>>,
    name: &str,
    args_json: &str,
) -> String {
    let reply = match parse_wire(args_json, fonts) {
        Ok((args, fonts)) => call_pixel(doc_id, loan, input, fonts, name, args),
        Err(reply) => reply,
    };
    reply.to_json().to_string()
}

/// [`call_image`] for the wasm bridge: arguments, the editor's fonts and the
/// reply as JSON text ([`WireFonts`], [`LiveReply::to_json`]).
pub fn call_image_wire(
    doc_id: &str,
    loan: ImageLoan<'_>,
    input: Option<LiveFile<'_>>,
    fonts: Option<WireFonts<'_>>,
    name: &str,
    args_json: &str,
) -> String {
    let reply = match parse_wire(args_json, fonts) {
        Ok((args, fonts)) => call_image(doc_id, loan, input, fonts, name, args),
        Err(reply) => reply,
    };
    reply.to_json().to_string()
}
