//! The Claude Code adapter (docs/specs/ai-chat-panel.md, Technical notes):
//! its arguments, its MCP config and its `stream-json` output.
//!
//! Flags as of Claude Code 2.1 (`claude --help`, 2026-11): `-p` with the
//! prompt on stdin, `--output-format stream-json --verbose
//! --include-partial-messages`, `--mcp-config <file> --strict-mcp-config`,
//! `--tools` (the built-in tools it has at all), `--allowedTools`,
//! `--disallowedTools`, `--append-system-prompt-file`, `--model`,
//! `--resume`.

use std::path::Path;

use serde_json::{Value, json};

use super::ChatEvent;
use super::adapter::{Launch, Stream, content_parts, in_dir, real_name, str_of};
use super::run::Answer;

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

/// The launch of one answer, run in the chat folder `dir`. The prompt
/// goes to stdin and the system prompt to a file: a Windows `.cmd` shim
/// cannot pass a line break in an argument.
pub fn launch(answer: &Answer, dir: &Path) -> Launch {
    let mut args: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--mcp-config",
        &in_dir(dir, MCP_CONFIG),
        "--strict-mcp-config",
        "--tools",
        "WebSearch",
        "--allowedTools",
        "mcp__pincel__*,WebSearch",
        "--disallowedTools",
        &DENIED.join(","),
        "--append-system-prompt-file",
        &in_dir(dir, SYSTEM_PROMPT),
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
    Launch {
        args,
        env: Vec::new(),
        files: vec![
            (MCP_CONFIG, mcp_config(answer)),
            (SYSTEM_PROMPT, answer.system_prompt.clone()),
        ],
        secret: vec![MCP_CONFIG],
    }
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

/// Reads Claude Code's `stream-json` lines into chat events.
#[derive(Default)]
pub struct Parser {
    /// Text deltas arrived for the message under way: its full text,
    /// which follows in the `assistant` line, is not shown twice.
    streamed: bool,
    /// The `result` line: `Some(Ok(text))` or `Some(Err(text))`.
    result: Option<Result<String, String>>,
    /// The CLI denied a tool it would have had to ask about (AC-27).
    denied: bool,
}

impl Stream for Parser {
    fn line(&mut self, line: &str) -> Result<Vec<ChatEvent>, ()> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Vec::new());
        }
        let v: Value = serde_json::from_str(line).map_err(|_| ())?;
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
                            out.push(ChatEvent::ToolStart {
                                id: str_of(block, "id").unwrap_or_default(),
                                name: real_name(
                                    &str_of(block, "name").unwrap_or_default(),
                                    &[PINCEL_PREFIX],
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

    fn finished_ok(&self) -> bool {
        matches!(self.result, Some(Ok(_)))
    }

    fn error_text(&self) -> Option<&str> {
        match &self.result {
            Some(Err(text)) => Some(text),
            _ => None,
        }
    }

    fn denied(&self) -> bool {
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
    let (text, images) = match block.get("content") {
        Some(Value::String(s)) => (s.clone(), Vec::new()),
        Some(Value::Array(parts)) => content_parts(parts),
        _ => (String::new(), Vec::new()),
    };
    ChatEvent::ToolEnd {
        id: block
            .get("tool_use_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        ok: block.get("is_error").and_then(Value::as_bool) != Some(true),
        text,
        images,
    }
}
