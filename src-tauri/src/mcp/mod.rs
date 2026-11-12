//! Live control of the desktop app over MCP (docs/specs/mcp-live-desktop.md).
//!
//! The endpoint ([`endpoint`]) runs in this process; the documents live in
//! the webview's wasm (pincel.md §9.3). A tool call therefore goes to the
//! webview as an `mcp-call` event, the mounted editor runs it through
//! `pincel-agent`, and the answer comes back through `mcp_call_result`.
//! The commands here are thin wrappers around [`Endpoint`] (AGENTS.md §5.5).

mod endpoint;
mod host_tools;
mod http;
mod secret;
#[cfg(test)]
mod tests;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

pub(crate) use endpoint::Endpoint;
use endpoint::{Bridge, ChatAccess, DEFAULT_PORT, ForwardedCall, Status, Timing};

/// Event a tool call reaches the webview with (payload [`ForwardedCall`]).
const CALL_EVENT: &str = "mcp-call";
/// Event with the number of open external sessions (status-bar indicator).
const SESSIONS_EVENT: &str = "mcp-sessions";
/// Event with the id of a call that timed out (payload `u64`).
const EXPIRED_EVENT: &str = "mcp-call-expired";

/// The webview of the running app.
struct Webview(AppHandle);

impl Bridge for Webview {
    fn forward(&self, call: &ForwardedCall) {
        if self.0.emit(CALL_EVENT, call).is_err() {
            // No webview to run it: the call times out (AC-19).
            eprintln!("pincel: cannot forward MCP call {}", call.id);
        }
    }

    fn sessions_changed(&self, external: usize) {
        let _ = self.0.emit(SESSIONS_EVENT, external);
    }

    fn expired(&self, id: u64) {
        let _ = self.0.emit(EXPIRED_EVENT, id);
    }
}

/// The app's endpoint, stopped until the setting or a chat starts it
/// (AC-01). Its config lives in the app config dir as `mcp.json`.
pub(crate) fn create(app: &AppHandle) -> Endpoint {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("pincel"));
    Endpoint::new(
        Box::new(Webview(app.clone())),
        dir.join("mcp.json"),
        DEFAULT_PORT,
        Timing::default(),
    )
}

/// The AI agents dialog's state.
#[tauri::command]
pub(crate) async fn mcp_status(endpoint: State<'_, Endpoint>) -> Result<Status, String> {
    Ok(endpoint.status())
}

/// Turn "Allow AI agents to control Pincel" on; `fresh` when the user
/// switched it on (new token), `false` at app start (stored token).
#[tauri::command]
pub(crate) async fn mcp_enable(
    endpoint: State<'_, Endpoint>,
    fresh: bool,
) -> Result<Status, String> {
    endpoint.enable(fresh)
}

/// Turn the setting off.
#[tauri::command]
pub(crate) async fn mcp_disable(endpoint: State<'_, Endpoint>) -> Result<Status, String> {
    Ok(endpoint.disable())
}

/// "Regenerate token".
#[tauri::command]
pub(crate) async fn mcp_regenerate_token(endpoint: State<'_, Endpoint>) -> Result<Status, String> {
    endpoint.regenerate()
}

/// The indicator's disconnect button.
#[tauri::command]
pub(crate) async fn mcp_disconnect(endpoint: State<'_, Endpoint>) -> Result<Status, String> {
    Ok(endpoint.disconnect())
}

/// The editor's answer to a forwarded call.
#[tauri::command]
pub(crate) async fn mcp_call_result(
    endpoint: State<'_, Endpoint>,
    id: u64,
    reply: Value,
) -> Result<(), String> {
    endpoint.deliver(id, reply);
    Ok(())
}

/// A dialog the call opened waits for the user, or closed.
#[tauri::command]
pub(crate) async fn mcp_call_waiting(
    endpoint: State<'_, Endpoint>,
    id: u64,
    waiting: bool,
) -> Result<(), String> {
    endpoint.set_waiting(id, waiting);
    Ok(())
}

/// A chat's first message: its URL and token (AC-22).
#[tauri::command]
pub(crate) async fn mcp_chat_start(
    endpoint: State<'_, Endpoint>,
    chat: String,
    image_access: bool,
) -> Result<ChatAccess, String> {
    endpoint.chat_start(&chat, image_access)
}

/// The chat ended; its token stops working.
#[tauri::command]
pub(crate) async fn mcp_chat_end(
    endpoint: State<'_, Endpoint>,
    chat: String,
) -> Result<(), String> {
    endpoint.chat_end(&chat);
    Ok(())
}

/// The chat's answer started or ended (AC-23).
#[tauri::command]
pub(crate) async fn mcp_chat_answering(
    endpoint: State<'_, Endpoint>,
    chat: String,
    running: bool,
) -> Result<(), String> {
    endpoint.chat_answering(&chat, running);
    Ok(())
}

/// The chat's "Agent may see the image" switch.
#[tauri::command]
pub(crate) async fn mcp_chat_image_access(
    endpoint: State<'_, Endpoint>,
    chat: String,
    on: bool,
) -> Result<(), String> {
    endpoint.chat_image_access(&chat, on);
    Ok(())
}
