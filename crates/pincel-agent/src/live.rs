//! Live mode: the tools on a document the desktop editor owns, lent to them
//! for one call (docs/specs/mcp-live-desktop.md, Decision Log 2026-10-29).
//!
//! The editor's wasm document is moved into a one-document [`Workspace`]
//! under the tab's id, the tool runs exactly as in the headless server, and
//! the document moves back. File access goes to the user instead of a
//! sandbox: a reading tool gets only the file the user picked in the open
//! dialog, and a writing tool's files are handed to the host, which writes
//! them where the user confirms in the save dialog (AC-16).

use pincel_core::{Bus, CelMap, FrameIndex, LayerId, Sprite};
use pincel_image_core::Document;
use pincel_image_core::command::CommandBus;
use pincel_image_core::command::spec::StrokeScratch;
use serde_json::{Value, json};

use crate::image::ImageDoc;
use crate::workspace::{Doc, Entry};
use crate::{AgentError, FileHost, ToolOutput, Workspace, base64, call};

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
/// editor's id `doc_id`. A mutating call adds exactly one undo step to
/// `loan.bus`; a failed call leaves the document unchanged.
pub fn call_pixel(
    doc_id: &str,
    loan: PixelLoan<'_>,
    input: Option<LiveFile<'_>>,
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
    let mut ws = Workspace::new();
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
/// editor's id `doc_id`. A mutating call adds exactly one undo step to
/// `loan.bus`; a failed call leaves the document unchanged.
pub fn call_image(
    doc_id: &str,
    loan: ImageLoan<'_>,
    input: Option<LiveFile<'_>>,
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
    let mut ws = Workspace::new();
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

/// Parse a call's arguments as the wasm bridges pass them: a JSON object,
/// or an empty string for none.
fn parse_wire_args(args_json: &str) -> Result<Value, LiveReply> {
    if args_json.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(args_json).map_err(|e| LiveReply {
        result: Err(AgentError::tool(format!(
            "arguments are not valid JSON: {e}"
        ))),
        written: Vec::new(),
    })
}

/// [`call_pixel`] for the wasm bridge: arguments and reply as JSON text
/// ([`LiveReply::to_json`]).
pub fn call_pixel_wire(
    doc_id: &str,
    loan: PixelLoan<'_>,
    input: Option<LiveFile<'_>>,
    name: &str,
    args_json: &str,
) -> String {
    let reply = match parse_wire_args(args_json) {
        Ok(args) => call_pixel(doc_id, loan, input, name, args),
        Err(reply) => reply,
    };
    reply.to_json().to_string()
}

/// [`call_image`] for the wasm bridge: arguments and reply as JSON text
/// ([`LiveReply::to_json`]).
pub fn call_image_wire(
    doc_id: &str,
    loan: ImageLoan<'_>,
    input: Option<LiveFile<'_>>,
    name: &str,
    args_json: &str,
) -> String {
    let reply = match parse_wire_args(args_json) {
        Ok(args) => call_image(doc_id, loan, input, name, args),
        Err(reply) => reply,
    };
    reply.to_json().to_string()
}
