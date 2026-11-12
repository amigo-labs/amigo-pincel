//! The Claude Code adapter (docs/specs/ai-chat-panel.md, Technical notes):
//! its arguments, its MCP config and its `stream-json` output.
//!
//! Flags as of Claude Code 2.1 (`claude --help`, 2026-11): `-p` with the
//! prompt on stdin, `--output-format stream-json --verbose
//! --include-partial-messages`, `--mcp-config <file> --strict-mcp-config`,
//! `--tools` (the built-in tools it has at all), `--allowedTools`,
//! `--disallowedTools`, `--append-system-prompt-file`, `--model`,
//! `--resume`.

use std::collections::HashMap;
use std::path::Path;

use serde_json::{Value, json};

use super::run::Answer;
use super::{ChatEvent, Hint};

/// The prefix Claude Code gives the tools of the MCP server `pincel`.
const PINCEL_PREFIX: &str = "mcp__pincel__";

/// Built-in tools the agent never gets (AC-16): shell, code, files, fetch.
const DENIED: &[&str] = &[
    "Bash",
    "PowerShell",
    "BashOutput",
    "KillShell",
    "Read",
    "Edit",
    "MultiEdit",
    "Write",
    "NotebookEdit",
    "Glob",
    "Grep",
    "LS",
    "WebFetch",
    "Task",
];

/// The file the MCP config goes to in the chat folder.
pub const MCP_CONFIG: &str = "mcp.json";
/// The file the system prompt goes to in the chat folder.
pub const SYSTEM_PROMPT: &str = "system-prompt.md";

/// The arguments for one answer, run in the chat folder `dir`. The prompt
/// goes to stdin and the system prompt to a file: a Windows `.cmd` shim
/// cannot pass a line break in an argument.
pub fn args(answer: &Answer, dir: &Path) -> Vec<String> {
    let path = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let mut args: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--mcp-config",
        &path(MCP_CONFIG),
        "--strict-mcp-config",
        "--tools",
        "WebSearch",
        "--allowedTools",
        "mcp__pincel__*,WebSearch",
        "--disallowedTools",
        &DENIED.join(","),
        "--append-system-prompt-file",
        &path(SYSTEM_PROMPT),
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    if let Some(model) = answer.model.as_deref().filter(|m| !m.trim().is_empty()) {
        args.extend(["--model".to_owned(), model.trim().to_owned()]);
    }
    if let Some(session) = &answer.session {
        args.extend(["--resume".to_owned(), session.clone()]);
    }
    args
}

/// The MCP config: the chat's internal session on the live endpoint, with
/// the chat's token (`mcp-live-desktop` AC-22).
pub fn mcp_config(answer: &Answer) -> String {
    json!({ "mcpServers": { "pincel": {
        "type": "http",
        "url": answer.mcp_url,
        "headers": { "Authorization": format!("Bearer {}", answer.mcp_token) },
    } } })
    .to_string()
}

/// The hint for a failure text (AC-20, AC-27 has its own path).
pub fn classify(text: &str, model: Option<&str>) -> Hint {
    let t = text.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| t.contains(w));
    if any(&[
        "not logged in",
        "/login",
        "invalid api key",
        "authentication",
        "oauth token",
        "unauthorized",
    ]) {
        Hint::SignIn
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
            "does not exist",
            "may not exist",
            "invalid model",
            "unknown model",
        ])
    {
        Hint::UnknownModel
    } else {
        Hint::Unexpected
    }
}

/// The real name of a Pincel tool from the name Claude Code shows it under
/// (`mcp__pincel__draw_rect` → `draw.rect`); other tools keep theirs.
fn display_name(names: &HashMap<String, &'static str>, name: &str) -> String {
    match name.strip_prefix(PINCEL_PREFIX) {
        Some(short) => names
            .get(short)
            .map_or_else(|| short.to_owned(), |n| (*n).to_owned()),
        None => name.to_owned(),
    }
}

/// The tool names as Claude Code sends them (anything but letters, digits,
/// `_` and `-` becomes `_`), mapped to the real names.
fn tool_names() -> HashMap<String, &'static str> {
    pincel_agent::list_tools()
        .into_iter()
        .map(|t| {
            let wire: String = t
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
            (wire, t.name)
        })
        .collect()
}

/// Width and height of a base64 PNG, from its IHDR.
fn png_size(data: &str) -> Option<[u32; 2]> {
    let bytes = pincel_agent::base64::decode(data)?;
    let be = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    };
    (bytes.get(12..16) == Some(b"IHDR")).then_some([be(16)?, be(20)?])
}

/// Reads Claude Code's `stream-json` lines into chat events.
#[derive(Default)]
pub struct Parser {
    names: Option<HashMap<String, &'static str>>,
    /// Text deltas arrived for the message under way: its full text,
    /// which follows in the `assistant` line, is not shown twice.
    streamed: bool,
    /// The `result` line: `Some(Ok(text))` or `Some(Err(text))`.
    result: Option<Result<String, String>>,
    /// The CLI denied a tool it would have had to ask about (AC-27).
    denied: bool,
}

impl Parser {
    /// The events of one output line; `Err` for a line that is not JSON
    /// (AC-20). Empty lines and unknown event types give none.
    pub fn line(&mut self, line: &str) -> Result<Vec<ChatEvent>, ()> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Vec::new());
        }
        let v: Value = serde_json::from_str(line).map_err(|_| ())?;
        let str_of = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(str::to_owned);
        let mut out = Vec::new();
        match v.get("type").and_then(Value::as_str) {
            Some("system") if str_of(&v, "subtype").as_deref() == Some("init") => {
                if let Some(id) = str_of(&v, "session_id") {
                    out.push(ChatEvent::Session { id });
                }
            }
            Some("stream_event") => {
                let delta = v.pointer("/event/delta");
                if v.pointer("/event/type").and_then(Value::as_str) == Some("content_block_delta")
                    && delta.and_then(|d| d.get("type")).and_then(Value::as_str)
                        == Some("text_delta")
                    && let Some(text) = delta.and_then(|d| str_of(d, "text"))
                {
                    self.streamed = true;
                    out.push(ChatEvent::Text { text });
                }
            }
            Some("assistant") => {
                for block in content(&v) {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") if !self.streamed => {
                            if let Some(text) = str_of(block, "text") {
                                out.push(ChatEvent::Text { text });
                            }
                        }
                        Some("tool_use") => {
                            let names = self.names.get_or_insert_with(tool_names);
                            out.push(ChatEvent::ToolStart {
                                id: str_of(block, "id").unwrap_or_default(),
                                name: display_name(
                                    names,
                                    &str_of(block, "name").unwrap_or_default(),
                                ),
                                args: block.get("input").cloned().unwrap_or(Value::Null),
                            });
                        }
                        _ => {}
                    }
                }
                self.streamed = false;
            }
            Some("user") => {
                for block in content(&v) {
                    if block.get("type").and_then(Value::as_str) == Some("tool_result") {
                        out.push(tool_end(block));
                    }
                }
            }
            Some("result") => {
                let text = str_of(&v, "result").unwrap_or_default();
                let error = v.get("is_error").and_then(Value::as_bool) == Some(true);
                self.result = Some(if error { Err(text) } else { Ok(text) });
                self.denied = v
                    .get("permission_denials")
                    .and_then(Value::as_array)
                    .is_some_and(|d| !d.is_empty());
            }
            _ => {}
        }
        Ok(out)
    }

    /// The CLI reported a result without an error.
    pub fn finished_ok(&self) -> bool {
        matches!(self.result, Some(Ok(_)))
    }

    /// The error text of an error result.
    pub fn error_text(&self) -> Option<&str> {
        match &self.result {
            Some(Err(text)) => Some(text),
            _ => None,
        }
    }

    /// The CLI denied a tool it would have asked about (AC-27).
    pub fn denied(&self) -> bool {
        self.denied
    }
}

fn content(v: &Value) -> &[Value] {
    v.pointer("/message/content")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A `tool_result` block as a tool row's end (AC-06).
fn tool_end(block: &Value) -> ChatEvent {
    let mut text = Vec::new();
    let mut images = Vec::new();
    match block.get("content") {
        Some(Value::String(s)) => text.push(s.clone()),
        Some(Value::Array(parts)) => {
            for part in parts {
                match part.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(s) = part.get("text").and_then(Value::as_str) {
                            text.push(s.to_owned());
                        }
                    }
                    Some("image") => {
                        if let Some(size) = part
                            .pointer("/source/data")
                            .and_then(Value::as_str)
                            .and_then(png_size)
                        {
                            images.push(size);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    ChatEvent::ToolEnd {
        id: block
            .get("tool_use_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        ok: block.get("is_error").and_then(Value::as_bool) != Some(true),
        text: text.join("\n"),
        images,
    }
}
