//! The Assistant chat's CLI side (docs/specs/ai-chat-panel.md): find the
//! installed agent CLIs, start one per answer against the live MCP
//! endpoint with the chat's token, stream its output to the webview as
//! `assistant-event`s, stop it, and clean up. The commands are thin
//! wrappers (AGENTS.md §5.5); the UI owns the chat itself. One adapter
//! per supported CLI (docs/specs/ai-chat-clis.md).

mod adapter;
mod claude;
mod codex;
mod find;
mod gemini;
mod opencode;
mod probe;
mod run;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_clis;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::mcp::Endpoint;
pub(crate) use run::Assistant;
use run::{Answer, Sink};

/// Event the answers' progress reaches the webview with.
const EVENT: &str = "assistant-event";

/// One step of an answer, in the order the CLI reports them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatEvent {
    /// The CLI's session id, continued by the next answer (AC-12).
    Session {
        /// The id `--resume` takes.
        id: String,
    },
    /// More of the agent's reply text (AC-05).
    Text {
        /// The new text.
        text: String,
    },
    /// The agent called a tool (AC-06).
    ToolStart {
        /// The call's id.
        id: String,
        /// The tool: a Pincel tool's own name, or the CLI's (`WebSearch`).
        name: String,
        /// The arguments.
        args: Value,
    },
    /// A tool call returned (AC-06).
    ToolEnd {
        /// The call's id.
        id: String,
        /// `false` for an error result.
        ok: bool,
        /// The result's text.
        text: String,
        /// Width and height of each image in the result.
        images: Vec<[u32; 2]>,
    },
    /// The answer ended (AC-08, AC-11, AC-20).
    Done {
        /// How.
        outcome: Outcome,
        /// What went wrong, for a failed answer.
        error: Option<Failure>,
    },
}

/// How an answer ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// The CLI finished.
    Finished,
    /// The user stopped it.
    Stopped,
    /// The CLI failed.
    Failed,
}

/// The cause the UI names for a failure (AC-20, AC-27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Hint {
    /// Not signed in.
    SignIn,
    /// No network.
    Offline,
    /// The model is unknown.
    UnknownModel,
    /// The CLI asked for a permission Pincel does not grant.
    Permission,
    /// A local model's server (Ollama, LM Studio) is not reachable
    /// (ai-chat-clis AC-08).
    LocalServer,
    /// The model cannot call tools (ai-chat-clis, Edge cases).
    NoTools,
    /// Anything else.
    Unexpected,
}

/// A failed answer's error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Failure {
    /// The CLI's own error text.
    pub text: String,
    /// The recognised cause.
    pub hint: Hint,
}

#[derive(Serialize, Clone)]
struct Payload<'a> {
    chat: &'a str,
    event: ChatEvent,
}

/// The webview of the running app.
struct Webview(AppHandle);

impl Sink for Webview {
    fn event(&self, chat: &str, event: ChatEvent) {
        let _ = self.0.emit(EVENT, Payload { chat, event });
    }
}

/// Where the chat folders live: `assistant` in the user's own app data
/// folder (the MCP config in them carries the chat's token, so never the
/// shared temp folder); without one, a folder of this start alone.
fn chats_root(local_data: Option<PathBuf>) -> PathBuf {
    local_data.map_or_else(
        || {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            std::env::temp_dir().join(format!("pincel-assistant-{}-{nanos}", std::process::id()))
        },
        |dir| dir.join("assistant"),
    )
}

/// The app's Assistant; chat folders from a crashed run are removed here
/// (AC-22).
pub(crate) fn create(app: &AppHandle) -> Assistant {
    let assistant = Assistant::new(
        chats_root(app.path().app_local_data_dir().ok()),
        Arc::new(Webview(app.clone())),
    );
    assistant.purge();
    assistant
}

/// App exit: no CLI started by Pincel stays running (AC-22).
pub(crate) fn shutdown(app: &AppHandle) {
    if let Some(assistant) = app.try_state::<Assistant>() {
        assistant.stop_all(Duration::from_secs(2));
        assistant.purge();
    }
}

/// A supported CLI, whether the user's terminal finds it (AC-03) and
/// whether its version is new enough (ai-chat-clis AC-04).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStatus {
    id: &'static str,
    name: &'static str,
    install_url: &'static str,
    installed: bool,
    /// The installed version; `None` when not installed or unreadable.
    version: Option<String>,
    /// The oldest version Pincel works with.
    min_version: String,
    /// Installed with a known version of at least `min_version`.
    supported: bool,
}

/// The supported CLIs, looked up afresh with their versions (AC-03,
/// ai-chat-clis AC-01, AC-04); the CLIs are asked in parallel.
#[tauri::command]
pub(crate) async fn assistant_clis() -> Result<Vec<CliStatus>, String> {
    let path = find::search_path();
    let path_env = (!cfg!(windows)).then_some(path.as_os_str());
    let found: Vec<(&find::CliDef, Option<PathBuf>)> = find::CLIS
        .iter()
        .map(|c| (c, find::locate_in(c.program, &path)))
        .collect();
    let versions: Vec<Option<[u32; 3]>> = std::thread::scope(|s| {
        let asks: Vec<_> = found
            .iter()
            .map(|(_, program)| {
                s.spawn(move || {
                    program
                        .as_deref()
                        .and_then(|p| probe::version(p, path_env, &[]))
                })
            })
            .collect();
        asks.into_iter().map(|h| h.join().ok().flatten()).collect()
    });
    Ok(found
        .iter()
        .zip(versions)
        .map(|((c, program), version)| CliStatus {
            id: c.id,
            name: c.name,
            install_url: c.install_url,
            installed: program.is_some(),
            version: version.map(probe::version_text),
            min_version: probe::version_text(c.min_version),
            supported: version.is_some_and(|v| v >= c.min_version),
        })
        .collect())
}

/// The models the model field lists for `cli` (ai-chat-clis AC-05): for
/// opencode, `opencode models` within 5 s; no list for the others. An
/// error makes the field take free text.
#[tauri::command]
pub(crate) async fn assistant_models(cli: String) -> Result<Vec<probe::Model>, String> {
    let def = find::cli(&cli).ok_or_else(|| format!("unknown CLI `{cli}`"))?;
    if def.kind != adapter::Kind::Opencode {
        return Ok(Vec::new());
    }
    let (program, path) = find::locate(def.program);
    let program = program.ok_or_else(|| format!("{} is not installed", def.name))?;
    let path_env = (!cfg!(windows)).then_some(path.as_os_str());
    probe::opencode_models(&program, path_env, &[], probe::DEADLINE)
}

/// What the UI sends for one answer.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SendRequest {
    chat: String,
    cli: String,
    model: Option<String>,
    /// The user's message with its editor-context block.
    prompt: String,
    system_prompt: String,
    session: Option<String>,
    image_access: bool,
}

/// Start an answer (AC-05): the chat's token on the endpoint, then the
/// CLI. Fails, starting nothing, when either cannot start (AC-28).
#[tauri::command]
pub(crate) async fn assistant_send(
    assistant: State<'_, Assistant>,
    endpoint: State<'_, Endpoint>,
    request: SendRequest,
) -> Result<(), String> {
    let def = find::cli(&request.cli).ok_or_else(|| format!("unknown CLI `{}`", request.cli))?;
    let (program, path) = find::locate(def.program);
    let program = program.ok_or_else(|| format!("{} is not installed", def.name))?;
    let chat = request.chat;
    let access = endpoint.chat_start(&chat, request.image_access)?;
    let answer = Answer {
        chat: chat.clone(),
        kind: def.kind,
        program,
        path_env: (!cfg!(windows)).then_some(path),
        prompt: request.prompt,
        system_prompt: request.system_prompt,
        model: request.model,
        session: request.session,
        mcp_url: access.url,
        mcp_token: access.token,
    };
    // External calls are refused from here on (mcp-live-desktop AC-23);
    // the UI clears it once the answer's last call has run.
    endpoint.chat_answering(&chat, true);
    assistant
        .start(answer)
        .inspect_err(|_| endpoint.chat_answering(&chat, false))
}

/// Stop the answer of `chat` (AC-11).
#[tauri::command]
pub(crate) async fn assistant_stop(
    assistant: State<'_, Assistant>,
    chat: String,
) -> Result<(), String> {
    assistant.stop(&chat);
    Ok(())
}

/// The chat ended (New chat, tab closed, CLI or model changed): its
/// answer stops, its token ends and its folder goes (AC-13, AC-14).
#[tauri::command]
pub(crate) async fn assistant_end(
    assistant: State<'_, Assistant>,
    endpoint: State<'_, Endpoint>,
    chat: String,
) -> Result<(), String> {
    assistant.end_chat(&chat);
    endpoint.chat_end(&chat);
    Ok(())
}
