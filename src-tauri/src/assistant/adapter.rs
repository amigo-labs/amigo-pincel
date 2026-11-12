//! What the four supported agent CLIs share and where they differ
//! (docs/specs/ai-chat-clis.md): how one answer is launched, how its
//! output is read, how a failure is named, and which models are local.
//! Each CLI's own flags and output format live in its module.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;

use super::run::Answer;
use super::{ChatEvent, Hint, claude, codex, gemini, opencode};

/// The supported CLIs (ai-chat-clis AC-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Claude Code (`claude`).
    Claude,
    /// OpenAI Codex CLI (`codex`).
    Codex,
    /// Gemini CLI (`gemini`).
    Gemini,
    /// opencode (`opencode`), which also runs local models.
    Opencode,
}

/// How one answer starts: the CLI's arguments, extra environment, and the
/// files written to the chat folder before it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Launch {
    /// The arguments.
    pub args: Vec<String>,
    /// Environment variables set for the CLI (and its children).
    pub env: Vec<(String, String)>,
    /// Files written owner-only into the chat folder: name and text.
    pub files: Vec<(&'static str, String)>,
    /// The files among them that carry the chat's token: removed when the
    /// answer ends, so the token stays on disk only while it runs.
    pub secret: Vec<&'static str>,
}

/// The launch of `answer` for its CLI, run in the chat folder `dir`.
pub fn launch(answer: &Answer, dir: &Path) -> Launch {
    match answer.kind {
        Kind::Claude => claude::launch(answer, dir),
        Kind::Codex => codex::launch(answer, dir),
        Kind::Gemini => gemini::launch(answer, dir),
        Kind::Opencode => opencode::launch(answer, dir),
    }
}

/// Reads one CLI's output, line by line, into chat events.
pub trait Stream: Send {
    /// The events of one output line; `Err` for a line that is not JSON
    /// (ai-chat-panel AC-20). Empty lines and unknown events give none.
    fn line(&mut self, line: &str) -> Result<Vec<ChatEvent>, ()>;
    /// The CLI reported that the answer finished without an error.
    fn finished_ok(&self) -> bool;
    /// The error text the CLI reported, if any.
    fn error_text(&self) -> Option<&str>;
    /// The CLI denied a tool it would have had to ask about
    /// (ai-chat-panel AC-27).
    fn denied(&self) -> bool {
        false
    }
}

/// The output reader for `kind`.
pub fn stream(kind: Kind) -> Box<dyn Stream> {
    match kind {
        Kind::Claude => Box::<claude::Parser>::default(),
        Kind::Codex => Box::<codex::Parser>::default(),
        Kind::Gemini => Box::<gemini::Parser>::default(),
        Kind::Opencode => Box::<opencode::Parser>::default(),
    }
}

/// Model providers that run on the user's machine (ai-chat-clis AC-06).
const LOCAL_PROVIDERS: &[&str] = &["ollama", "lmstudio"];

/// `true` when `model` runs on the user's machine: an opencode model of the
/// Ollama or LM Studio provider (`ollama/llama3.2`). Every other model,
/// and every CLI's default model, counts as cloud.
pub fn is_local(kind: Kind, model: Option<&str>) -> bool {
    kind == Kind::Opencode
        && model
            .and_then(|m| m.trim().split_once('/'))
            .is_some_and(|(provider, _)| LOCAL_PROVIDERS.contains(&provider))
}

/// The hint for a failure text (ai-chat-panel AC-20, ai-chat-clis AC-08
/// and the tool-calling edge case). `local`: the answer ran a local model.
pub fn classify(text: &str, model: Option<&str>, local: bool) -> Hint {
    let t = text.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| t.contains(w));
    if any(&[
        "does not support tools",
        "doesn't support tools",
        "does not support tool",
        "tool use is not supported",
        "tools are not supported",
        "tool calling is not supported",
    ]) {
        Hint::NoTools
    } else if any(&[
        "not logged in",
        "/login",
        "invalid api key",
        "authentication",
        "oauth token",
        "unauthorized",
        "codex login",
        "auth method",
        "gemini_api_key",
        "providerautherror",
    ]) {
        Hint::SignIn
    } else if local
        && any(&[
            "econnrefused",
            "connection refused",
            "unable to connect",
            "cannot connect",
            "fetch failed",
            "connect error",
            "connection error",
        ])
    {
        Hint::LocalServer
    } else if any(&[
        "enotfound",
        "econnrefused",
        "eai_again",
        "etimedout",
        "getaddrinfo",
        "fetch failed",
        "connection error",
        "unable to connect",
        "network",
    ]) {
        Hint::Offline
    } else if model.is_some()
        && t.contains("model")
        && any(&[
            "not_found",
            "not found",
            "notfound",
            "does not exist",
            "may not exist",
            "invalid model",
            "unknown model",
        ])
    {
        Hint::UnknownModel
    } else if any(&[
        "permission requested",
        "requires approval",
        "approval required",
    ]) {
        Hint::Permission
    } else {
        Hint::Unexpected
    }
}

/// Pincel's tool names keyed by the wire form CLIs give them (anything but
/// letters, digits, `_` and `-` becomes `_`) and by their own name.
fn tool_names() -> &'static HashMap<String, &'static str> {
    static NAMES: OnceLock<HashMap<String, &'static str>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut map = HashMap::new();
        for tool in pincel_agent::list_tools() {
            let wire: String = tool
                .name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            map.insert(wire, tool.name);
            map.insert(tool.name.to_owned(), tool.name);
        }
        map
    })
}

/// The real name of a Pincel tool from the name a CLI shows it under:
/// the first of `prefixes` it starts with is dropped, then the wire form
/// is mapped back (`mcp__pincel__draw_rect` → `draw.rect`). A name that
/// is not Pincel's (web search) stays as it is.
pub fn real_name(name: &str, prefixes: &[&str]) -> String {
    let names = tool_names();
    if let Some(real) = names.get(name) {
        return (*real).to_owned();
    }
    prefixes
        .iter()
        .find_map(|p| name.strip_prefix(p))
        .map_or_else(
            || name.to_owned(),
            |short| {
                names
                    .get(short)
                    .map_or_else(|| short.to_owned(), |n| (*n).to_owned())
            },
        )
}

/// Width and height of a base64 PNG, from its IHDR.
pub fn png_size(data: &str) -> Option<[u32; 2]> {
    let bytes = pincel_agent::base64::decode(data)?;
    let be = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    };
    (bytes.get(12..16) == Some(b"IHDR")).then_some([be(16)?, be(20)?])
}

/// The text and image sizes of MCP content blocks (`[{type: text, text},
/// {type: image, data}]`, or Claude Code's `source.data` images).
pub fn content_parts(parts: &[Value]) -> (String, Vec<[u32; 2]>) {
    let mut text = Vec::new();
    let mut images = Vec::new();
    for part in parts {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(s) = part.get("text").and_then(Value::as_str) {
                    text.push(s.to_owned());
                }
            }
            Some("image") => {
                if let Some(size) = part
                    .get("data")
                    .or_else(|| part.pointer("/source/data"))
                    .and_then(Value::as_str)
                    .and_then(png_size)
                {
                    images.push(size);
                }
            }
            _ => {}
        }
    }
    (text.join("\n"), images)
}

/// A string field of `v`.
pub fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// The path of `name` in the chat folder `dir`, as an argument.
pub fn in_dir(dir: &Path, name: &str) -> String {
    dir.join(name).to_string_lossy().into_owned()
}
