//! The Gemini CLI adapter (docs/specs/ai-chat-clis.md, Technical notes):
//! its arguments, its settings file and its `stream-json` output.
//!
//! As of Gemini CLI 0.68 (docs and `packages/core/src/output/types.ts`,
//! 2026-11): headless mode with the prompt on stdin, `--output-format
//! stream-json` (`init`, `message`, `tool_use`, `tool_result`, `error`,
//! `result` lines), `--allowed-mcp-server-names`, `--extensions none`, `-m`,
//! `--resume <session id>`. The MCP server and the tool limits go into a
//! settings file that `GEMINI_CLI_SYSTEM_SETTINGS_PATH` names: system
//! settings override the user's and the workspace's. The system prompt is
//! the chat folder's `GEMINI.md`, which Gemini CLI loads as context from
//! its working directory.

use std::path::Path;

use serde_json::{Value, json};

use super::ChatEvent;
use super::adapter::{Launch, Stream, in_dir, real_name, str_of};
use super::run::Answer;

/// The settings file in the chat folder (it carries the chat's token).
pub const SETTINGS: &str = "gemini-settings.json";
/// The context file the system prompt goes to.
pub const CONTEXT: &str = "GEMINI.md";
/// The environment variable naming the system settings file.
pub const SETTINGS_ENV: &str = "GEMINI_CLI_SYSTEM_SETTINGS_PATH";

/// Gemini CLI's built-in web search, the one built-in tool it keeps.
pub const WEB_SEARCH: &str = "google_web_search";

/// Built-in tools excluded besides the allowlist (ai-chat-clis AC-03):
/// shell, files, fetch, memory and sub-agents.
pub const EXCLUDED: &[&str] = &[
    "run_shell_command",
    "read_file",
    "read_many_files",
    "write_file",
    "replace",
    "edit",
    "glob",
    "search_file_content",
    "grep_search",
    "list_directory",
    "web_fetch",
    "save_memory",
    "write_todos",
    "codebase_investigator",
    "delegate_to_agent",
    "activate_skill",
];

/// Prefixes Gemini CLI may give the tools of the MCP server `pincel`.
const PREFIXES: &[&str] = &["mcp_pincel_", "pincel__", "mcp__pincel__"];

/// The settings: the chat's Pincel session as the only MCP server, trusted
/// (no confirmation prompt), and web search as the only built-in tool.
pub fn settings(answer: &Answer) -> String {
    json!({
        "mcpServers": { "pincel": {
            "httpUrl": answer.mcp_url,
            "headers": { "Authorization": format!("Bearer {}", answer.mcp_token) },
            "trust": true,
        } },
        "mcp": { "allowed": ["pincel"] },
        "tools": { "core": [WEB_SEARCH], "exclude": EXCLUDED },
        "security": { "folderTrust": { "enabled": false } },
    })
    .to_string()
}

/// The launch of one answer, run in the chat folder `dir`.
pub fn launch(answer: &Answer, dir: &Path) -> Launch {
    let mut args: Vec<String> = [
        "--output-format",
        "stream-json",
        "--allowed-mcp-server-names",
        "pincel",
        "--extensions",
        "none",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    if let Some(model) = answer.model.as_deref().filter(|m| !m.trim().is_empty()) {
        args.extend(["-m".to_owned(), model.trim().to_owned()]);
    }
    if let Some(session) = &answer.session {
        args.extend(["--resume".to_owned(), session.clone()]);
    }
    Launch {
        args,
        env: vec![(SETTINGS_ENV.to_owned(), in_dir(dir, SETTINGS))],
        files: vec![
            (SETTINGS, settings(answer)),
            (CONTEXT, answer.system_prompt.clone()),
        ],
        secret: vec![SETTINGS],
    }
}

/// Reads Gemini CLI's `stream-json` lines into chat events.
#[derive(Default)]
pub struct Parser {
    /// The `result` line: `Some(Ok(()))` or `Some(Err(text))`.
    result: Option<Result<(), String>>,
    /// The last `error` line of severity `error`.
    error: Option<String>,
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
            Some("init") => {
                if let Some(id) = str_of(&v, "session_id") {
                    out.push(ChatEvent::Session { id });
                }
            }
            Some("message") if str_of(&v, "role").as_deref() == Some("assistant") => {
                if let Some(text) = str_of(&v, "content").filter(|t| !t.is_empty()) {
                    out.push(ChatEvent::Text { text });
                }
            }
            Some("tool_use") => out.push(ChatEvent::ToolStart {
                id: str_of(&v, "tool_id").unwrap_or_default(),
                name: real_name(&str_of(&v, "tool_name").unwrap_or_default(), PREFIXES),
                args: v.get("parameters").cloned().unwrap_or(Value::Null),
            }),
            Some("tool_result") => {
                let ok = str_of(&v, "status").as_deref() == Some("success");
                let text = str_of(&v, "output")
                    .or_else(|| {
                        v.pointer("/error/message")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
                    .unwrap_or_default();
                out.push(ChatEvent::ToolEnd {
                    id: str_of(&v, "tool_id").unwrap_or_default(),
                    ok,
                    text,
                    images: Vec::new(),
                });
            }
            Some("error") if str_of(&v, "severity").as_deref() != Some("warning") => {
                self.error = str_of(&v, "message");
            }
            Some("result") => {
                self.result = Some(if str_of(&v, "status").as_deref() == Some("success") {
                    Ok(())
                } else {
                    Err(v
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .or_else(|| self.error.clone())
                        .unwrap_or_default())
                });
            }
            _ => {}
        }
        Ok(out)
    }

    fn finished_ok(&self) -> bool {
        matches!(self.result, Some(Ok(())))
    }

    fn error_text(&self) -> Option<&str> {
        match &self.result {
            Some(Err(text)) if !text.is_empty() => Some(text),
            _ => self.error.as_deref(),
        }
    }
}
