//! The OpenAI Codex CLI adapter (docs/specs/ai-chat-clis.md, Technical
//! notes): its arguments and its `exec --json` output.
//!
//! Flags as of Codex CLI 0.156 (`codex exec --help`, 2026-11): `exec
//! --json` with the prompt on stdin (`-`), `--ignore-user-config` (no MCP
//! server, profile or feature from the user's `config.toml`; sign-in still
//! works) and `--ignore-rules`, `--skip-git-repo-check` (the chat folder is
//! no repository), `--sandbox read-only`, `-c key=value` overrides in TOML
//! for the approval policy, web search, the MCP server, the system prompt
//! (`developer_instructions`) and the features, `-m`, and `exec resume
//! <id>`. The token reaches the MCP server through an environment variable
//! (`bearer_token_env_var`), never as an argument.

use std::collections::HashSet;
use std::path::Path;

use serde_json::{Value, json};

use super::ChatEvent;
use super::adapter::{Launch, Stream, content_parts, real_name, str_of};
use super::run::Answer;

/// The environment variable the chat's token goes to.
pub const TOKEN_ENV: &str = "PINCEL_MCP_TOKEN";

/// Built-in features that give the agent a tool beyond Pincel's and web
/// search (ai-chat-clis AC-03): the shell, reading images from disk, the
/// browser and computer use, image generation, sub-agents, apps, plugins
/// and memories.
pub const DISABLED_FEATURES: &[&str] = &[
    "shell_tool",
    "unified_exec",
    "view_image",
    "apps",
    "plugins",
    "browser_use",
    "browser_use_external",
    "in_app_browser",
    "computer_use",
    "image_generation",
    "multi_agent",
    "memories",
    "code_mode_host",
    "tool_suggest",
    "skill_search",
    "skill_mcp_dependency_install",
    "workspace_dependencies",
];

/// `s` as a TOML basic string: no raw line break reaches the argument, so
/// a Windows `.cmd` shim can carry it.
pub fn toml_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The launch of one answer (the chat folder holds nothing of Codex's).
pub fn launch(answer: &Answer, _dir: &Path) -> Launch {
    let mut args: Vec<String> = [
        "exec",
        "--json",
        "--color",
        "never",
        "--skip-git-repo-check",
        "--ignore-user-config",
        "--ignore-rules",
        "--sandbox",
        "read-only",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let mut set = |key: &str, value: String| {
        args.push("-c".to_owned());
        args.push(format!("{key}={value}"));
    };
    set("approval_policy", toml_str("never"));
    set("web_search", toml_str("live"));
    set("mcp_servers.pincel.url", toml_str(&answer.mcp_url));
    set(
        "mcp_servers.pincel.bearer_token_env_var",
        toml_str(TOKEN_ENV),
    );
    // A Pincel call may wait for a dialog; Codex's default is 60 s.
    set("mcp_servers.pincel.tool_timeout_sec", "600".to_owned());
    set("include_apply_patch_tool", "false".to_owned());
    for feature in DISABLED_FEATURES {
        set(&format!("features.{feature}"), "false".to_owned());
    }
    set("developer_instructions", toml_str(&answer.system_prompt));
    if let Some(model) = answer.model.as_deref().filter(|m| !m.trim().is_empty()) {
        args.extend(["-m".to_owned(), model.trim().to_owned()]);
    }
    if let Some(session) = &answer.session {
        args.extend(["resume".to_owned(), session.clone()]);
    }
    args.push("-".to_owned());
    Launch {
        args,
        env: vec![(TOKEN_ENV.to_owned(), answer.mcp_token.clone())],
        files: Vec::new(),
        transient: Vec::new(),
    }
}

/// Reads `codex exec --json` lines (`thread.started`, `item.*`,
/// `turn.completed`, `turn.failed`, `error`) into chat events.
#[derive(Default)]
pub struct Parser {
    /// Tool items whose start was shown.
    started: HashSet<String>,
    /// Agent text was shown since the last tool: the next message starts
    /// a new paragraph.
    texted: bool,
    completed: bool,
    error: Option<String>,
}

impl Parser {
    /// A tool item's start: its row's name and arguments.
    fn tool_start(&mut self, item: &Value) -> Option<ChatEvent> {
        let id = str_of(item, "id").unwrap_or_default();
        let (name, args) = match item.get("type").and_then(Value::as_str)? {
            "mcp_tool_call" => {
                let tool = str_of(item, "tool").unwrap_or_default();
                let name = if str_of(item, "server").as_deref() == Some("pincel") {
                    real_name(&tool, &["mcp__pincel__"])
                } else {
                    tool
                };
                (name, item.get("arguments").cloned().unwrap_or(Value::Null))
            }
            "web_search" => (
                "web_search".to_owned(),
                json!({ "query": item.get("query").cloned().unwrap_or(Value::Null) }),
            ),
            "command_execution" => (
                "shell".to_owned(),
                json!({ "command": item.get("command").cloned().unwrap_or(Value::Null) }),
            ),
            _ => return None,
        };
        if !self.started.insert(id.clone()) {
            return None;
        }
        self.texted = false;
        Some(ChatEvent::ToolStart { id, name, args })
    }

    /// A finished tool item as its row's end.
    fn tool_end(item: &Value) -> Option<ChatEvent> {
        let id = str_of(item, "id").unwrap_or_default();
        let failed = str_of(item, "status").as_deref() == Some("failed");
        let (ok, text, images) = match item.get("type").and_then(Value::as_str)? {
            "mcp_tool_call" => {
                let error = item.pointer("/error/message").and_then(Value::as_str);
                let result = item.get("result").unwrap_or(&Value::Null);
                let (text, images) = result
                    .get("content")
                    .and_then(Value::as_array)
                    .map_or_else(|| (String::new(), Vec::new()), |p| content_parts(p));
                let is_error = result.get("is_error").and_then(Value::as_bool) == Some(true);
                match error {
                    Some(e) => (false, e.to_owned(), images),
                    None => (!failed && !is_error, text, images),
                }
            }
            "web_search" => (!failed, String::new(), Vec::new()),
            "command_execution" => (
                !failed && item.get("exit_code").and_then(Value::as_i64) == Some(0),
                str_of(item, "aggregated_output").unwrap_or_default(),
                Vec::new(),
            ),
            _ => return None,
        };
        Some(ChatEvent::ToolEnd {
            id,
            ok,
            text,
            images,
        })
    }
}

impl Stream for Parser {
    fn line(&mut self, line: &str) -> Result<Vec<ChatEvent>, ()> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Vec::new());
        }
        let v: Value = serde_json::from_str(line).map_err(|_| ())?;
        let mut out = Vec::new();
        let item = v.get("item").unwrap_or(&Value::Null);
        match v.get("type").and_then(Value::as_str) {
            Some("thread.started") => {
                if let Some(id) = str_of(&v, "thread_id") {
                    out.push(ChatEvent::Session { id });
                }
            }
            Some("item.started") => out.extend(self.tool_start(item)),
            Some("item.completed") => {
                if item.get("type").and_then(Value::as_str) == Some("agent_message") {
                    if let Some(text) = str_of(item, "text").filter(|t| !t.is_empty()) {
                        let text = if self.texted {
                            format!("\n\n{text}")
                        } else {
                            text
                        };
                        self.texted = true;
                        out.push(ChatEvent::Text { text });
                    }
                } else {
                    // A tool that ran without a start line still gets its row.
                    out.extend(self.tool_start(item));
                    out.extend(Self::tool_end(item));
                }
            }
            Some("turn.completed") => self.completed = true,
            Some("turn.failed") => {
                self.error = item_message(v.get("error")).or(self.error.take());
            }
            Some("error") => {
                if let Some(message) = str_of(&v, "message") {
                    self.error = Some(message);
                }
            }
            _ => {}
        }
        Ok(out)
    }

    fn finished_ok(&self) -> bool {
        self.completed && self.error.is_none()
    }

    fn error_text(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

fn item_message(error: Option<&Value>) -> Option<String> {
    error.and_then(|e| str_of(e, "message"))
}
