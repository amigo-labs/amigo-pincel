//! The ComfyUI connection of the desktop app (docs/specs/comfyui-connect.md,
//! Decision Log 2026-11-06 "ComfyUI"): finding a running server, starting
//! an existing installation, a custom URL, the status and the model list.
//! It lives here rather than in the webview, so the CSP stays closed.

pub mod api;
mod download;
mod engine;
mod extract;
pub mod hardware;
mod pins;
mod http;
pub mod install;
pub mod manager;
mod process;
pub mod settings;
#[cfg(test)]
mod testing;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager as _, Runtime};

use api::ServerInfo;
use manager::{Config, FolderCheck, Manager, Paths, Status};
use settings::{ServerChoice, Settings};

/// Event carrying every status change (a start finishing, a process
/// ending) to the renderer.
pub const STATUS_EVENT: &str = "comfyui:status";

/// Managed state: the app's one connection.
pub struct ComfyState(Arc<Manager>);

/// Creates the connection and, off the start path, takes over a server an
/// earlier run left running (AC-10).
pub fn setup<R: Runtime>(app: &AppHandle<R>) {
    http::install_crypto_provider();
    let Ok(dir) = app.path().app_config_dir() else {
        eprintln!("pincel: no app config directory, ComfyUI connection disabled");
        return;
    };
    let emitter = app.clone();
    let manager = Arc::new(Manager::new(
        Config::default(),
        Paths {
            settings: dir.join("comfyui.json"),
            record: dir.join("comfyui-run.json"),
        },
        Box::new(move |status| {
            let _ = emitter.emit(STATUS_EVENT, status);
        }),
    ));
    app.manage(ComfyState(Arc::clone(&manager)));
    std::thread::spawn(move || manager.adopt_on_start());
}

/// App quit: ends the server the app started (AC-09).
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    if let Some(state) = app.try_state::<ComfyState>() {
        state.0.shutdown();
    }
}

/// Runs `f` on the connection off the async runtime: probes and starts
/// block for up to the start timeout.
async fn blocking<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    f: impl FnOnce(&Manager) -> T + Send + 'static,
) -> Result<T, String> {
    let manager = app
        .try_state::<ComfyState>()
        .map(|state| Arc::clone(&state.0))
        .ok_or_else(|| "ComfyUI connection unavailable".to_string())?;
    tauri::async_runtime::spawn_blocking(move || f(&manager))
        .await
        .map_err(|e| e.to_string())
}

/// The stored settings (AC-02).
#[tauri::command]
pub async fn comfyui_settings(app: AppHandle) -> Result<Settings, String> {
    blocking(&app, |m| m.settings()).await
}

/// Stores the server choice.
#[tauri::command]
pub async fn comfyui_set_server(app: AppHandle, server: ServerChoice) -> Result<Settings, String> {
    blocking(&app, move |m| m.set_server(server)).await?
}

/// Checks a picked folder and stores it if it is an installation (AC-05).
#[tauri::command]
pub async fn comfyui_choose_folder(app: AppHandle, path: String) -> Result<FolderCheck, String> {
    blocking(&app, move |m| m.choose_folder(&path)).await?
}

/// Stores a custom URL and tests it (AC-08).
#[tauri::command]
pub async fn comfyui_test_url(app: AppHandle, url: String) -> Result<Status, String> {
    blocking(&app, move |m| m.test_url(&url)).await?
}

/// A server is needed (the dialog opened): resolve or start it (AC-03, AC-06).
#[tauri::command]
pub async fn comfyui_connect(app: AppHandle) -> Result<Status, String> {
    blocking(&app, |m| m.ensure()).await
}

/// The current status, without probing.
#[tauri::command]
pub async fn comfyui_status(app: AppHandle) -> Result<Status, String> {
    blocking(&app, |m| m.status()).await
}

/// The server a generation can use, or why there is none (AC-12; the
/// Generate panel and `gen.*` tools of comfyui-generate call it).
#[tauri::command]
pub async fn comfyui_usable(app: AppHandle) -> Result<ServerInfo, String> {
    blocking(&app, |m| m.ensure_usable()).await?
}
