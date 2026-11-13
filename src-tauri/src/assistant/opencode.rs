//! The opencode adapter (docs/specs/ai-chat-clis.md, Technical notes): its
//! arguments, its config file and its `run --format json` output.
//!
//! As of opencode 1.19 (`packages/opencode/src/cli/cmd/run.ts` and the V1
//! config schema, 2026-11): `run --format json` with the prompt on stdin,
//! `-m <provider/model>`, `--session <id>`. Its JSON lines (`step_start`,
//! `text`, `tool_use`, `step_finish`, `error`) all carry the `sessionID`;
//! a tool shows up once it has finished. The MCP server, the permissions
//! and the system prompt go into a config file named by `OPENCODE_CONFIG`,
//! which overrides the user's global config; project configs are off.
//! opencode 2 (`opencode2`, the V2 config) is a separate program.

use std::path::Path;

use serde_json::{Value, json};

use super::ChatEvent;
use super::adapter::{Launch, Stream, in_dir, real_name, str_of};
use super::run::Answer;

/// The config file in the chat folder (it carries the chat's token).
pub const CONFIG: &str = "opencode-config.json";
/// The file the system prompt goes to.
pub const SYSTEM_PROMPT: &str = "system-prompt.md";
/// The environment variable naming the config file.
pub const CONFIG_ENV: &str = "OPENCODE_CONFIG";
/// The environment variable that keeps project configs out.
pub const NO_PROJECT_ENV: &str = "OPENCODE_DISABLE_PROJECT_CONFIG";

/// How long one Pincel tool call may take (opencode's default is 5 s).
const TOOL_TIMEOUT_MS: u32 = 600_000;

/// The config: the chat's Pincel session as MCP server `pincel`, every
/// tool denied but Pincel's and web search (ai-chat-clis AC-03), and the
/// system prompt as instructions. opencode matches permissions in key
/// order and the last match wins: `*` comes first (serde_json keeps keys
/// sorted, and `*` sorts before letters).
pub fn config(answer: &Answer, dir: &Path) -> String {
    json!({
        "$schema": "https://opencode.ai/config.json",
        "mcp": { "pincel": {
            "type": "remote",
            "url": answer.mcp_url,
            "headers": { "Authorization": format!("Bearer {}", answer.mcp_token) },
            "oauth": false,
            "enabled": true,
            "timeout": TOOL_TIMEOUT_MS,
        } },
        "permission": { "*": "deny", "pincel_*": "allow", "websearch": "allow" },
        "instructions": [in_dir(dir, SYSTEM_PROMPT)],
    })
    .to_string()
}

/// The launch of one answer, run in the chat folder `dir`.
pub fn launch(answer: &Answer, dir: &Path) -> Launch {
    let mut args: Vec<String> = ["run", "--format", "json"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    if let Some(model) = answer.model.as_deref().filter(|m| !m.trim().is_empty()) {
        args.extend(["-m".to_owned(), model.trim().to_owned()]);
    }
    if let Some(session) = &answer.session {
        args.extend(["--session".to_owned(), session.clone()]);
    }
    Launch {
        args,
        env: vec![
            (CONFIG_ENV.to_owned(), in_dir(dir, CONFIG)),
            (NO_PROJECT_ENV.to_owned(), "1".to_owned()),
        ],
        files: vec![
            (CONFIG, config(answer, dir)),
            (SYSTEM_PROMPT, answer.system_prompt.clone()),
        ],
        transient: vec![CONFIG],
    }
}

/// Reads `opencode run --format json` lines into chat events.
#[derive(Default)]
pub struct Parser {
    session: bool,
    texted: bool,
    errors: Vec<String>,
}

impl Stream for Parser {
    fn line(&mut self, line: &str) -> Result<Vec<ChatEvent>, ()> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(Vec::new());
        }
        let v: Value = serde_json::from_str(line).map_err(|_| ())?;
        let mut out = Vec::new();
        if !self.session
            && let Some(id) = str_of(&v, "sessionID")
        {
            self.session = true;
            out.push(ChatEvent::Session { id });
        }
        let part = v.get("part").unwrap_or(&Value::Null);
        match v.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = str_of(part, "text").filter(|t| !t.trim().is_empty()) {
                    let text = if self.texted {
                        format!("\n\n{text}")
                    } else {
                        text
                    };
                    self.texted = true;
                    out.push(ChatEvent::Text { text });
                }
            }
            Some("tool_use") => {
                self.texted = false;
                let id = str_of(part, "callID")
                    .or_else(|| str_of(part, "id"))
                    .unwrap_or_default();
                let state = part.get("state").unwrap_or(&Value::Null);
                out.push(ChatEvent::ToolStart {
                    id: id.clone(),
                    name: real_name(&str_of(part, "tool").unwrap_or_default(), &["pincel_"]),
                    args: state.get("input").cloned().unwrap_or(Value::Null),
                });
                let ok = str_of(state, "status").as_deref() == Some("completed");
                let text = if ok {
                    str_of(state, "output")
                } else {
                    str_of(state, "error").or_else(|| str_of(state, "output"))
                };
                out.push(ChatEvent::ToolEnd {
                    id,
                    ok,
                    text: text.unwrap_or_default(),
                    images: Vec::new(),
                });
            }
            Some("error") => {
                let error = v.get("error").unwrap_or(&Value::Null);
                let text = error
                    .pointer("/data/message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .or_else(|| str_of(error, "name"))
                    .unwrap_or_else(|| "error".to_owned());
                self.errors.push(text);
            }
            _ => {}
        }
        Ok(out)
    }

    /// opencode has no final result line: an answer without an error line
    /// finished (with the process's exit code).
    fn finished_ok(&self) -> bool {
        self.errors.is_empty()
    }

    fn error_text(&self) -> Option<&str> {
        self.errors.last().map(String::as_str)
    }
}

/// The models `opencode models` lists (`provider/model` per line), in its
/// order; other lines (warnings, blank) are skipped (ai-chat-clis AC-05).
pub fn parse_models(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.contains(char::is_whitespace)
                && l.split_once('/')
                    .is_some_and(|(p, m)| !p.is_empty() && !m.is_empty())
        })
        .map(str::to_owned)
        .collect()
}
