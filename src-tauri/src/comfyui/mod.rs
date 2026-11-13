//! The ComfyUI connection of the desktop app (docs/specs/comfyui-connect.md,
//! Decision Log 2026-11-06 "ComfyUI"): finding a running server, starting
//! an existing installation, a custom URL, the status and the model list.
//! It lives here rather than in the webview, so the CSP stays closed.
//! The managed install (docs/specs/comfyui-install.md) is `managed`.

pub mod api;
mod download;
mod engine;
mod extract;
pub mod generate;
pub mod hardware;
mod http;
pub mod install;
pub mod managed;
pub mod manager;
mod pins;
mod process;
pub mod settings;
#[cfg(test)]
mod testing;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager as _, Runtime};

use api::ServerInfo;
use download::InstallError;
use generate::{GenConfig, GenError, GenEvent, GenerateRequest, Generations, Models, Started};
use managed::{InstallInfo, LocationCheck, ManagedState, Service};
use manager::{Config, FolderCheck, Manager, Paths, Status};
use settings::{ServerChoice, Settings};

/// Event carrying every status change (a start finishing, a process
/// ending) to the renderer.
pub const STATUS_EVENT: &str = "comfyui:status";

/// Event carrying the progress and end of a managed-install job.
pub const INSTALL_EVENT: &str = "comfyui:install";

/// Event carrying a generation's queue position, progress, preview and
/// end (comfyui-generate).
pub const GENERATION_EVENT: &str = "comfyui:generation";

/// Managed state: the app's one connection.
pub struct ComfyState(Arc<Manager>);

/// Managed state: the managed installation.
pub struct InstallState(Arc<Service>);

/// Managed state: the generation jobs (comfyui-generate).
pub struct GenerationState(Generations);

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
    let jobs = app.clone();
    let service = Arc::new(Service::new(
        Arc::clone(&manager),
        dir,
        Box::new(move |event| {
            let _ = jobs.emit(INSTALL_EVENT, event);
        }),
    ));
    app.manage(InstallState(Arc::clone(&service)));
    let generations = app.clone();
    app.manage(GenerationState(Generations::new(
        GenConfig::default(),
        Box::new(move |event: &GenEvent| {
            let _ = generations.emit(GENERATION_EVENT, event);
        }),
        // No package change while a generation runs (comfyui-install AC-14).
        Box::new(move |running| service.set_generating(running)),
    )));
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

/// Runs `f` on the managed installation off the async runtime: the
/// hardware check, folder deletion and server stops block.
async fn installing<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    f: impl FnOnce(&Arc<Service>) -> T + Send + 'static,
) -> Result<T, String> {
    let service = app
        .try_state::<InstallState>()
        .map(|state| Arc::clone(&state.0))
        .ok_or_else(|| "ComfyUI install unavailable".to_string())?;
    tauri::async_runtime::spawn_blocking(move || f(&service))
        .await
        .map_err(|e| e.to_string())
}

/// A job command whose `spawn_blocking` failed.
fn failed(message: String) -> InstallError {
    InstallError::Failed { message }
}

/// Hardware, catalog, sizes, accepted licenses, the managed installation
/// and a running job (comfyui-install AC-01 – AC-05).
#[tauri::command]
pub async fn comfyui_install_info(app: AppHandle) -> Result<InstallInfo, String> {
    installing(&app, |s| s.info()).await
}

/// Free space, writability and an existing install at a picked location
/// (AC-03).
#[tauri::command]
pub async fn comfyui_check_location(app: AppHandle, path: String) -> Result<LocationCheck, String> {
    installing(&app, move |s| s.check_location(&path)).await
}

/// Remembers accepted licenses (AC-05).
#[tauri::command]
pub async fn comfyui_accept_licenses(
    app: AppHandle,
    ids: Vec<String>,
) -> Result<Vec<String>, String> {
    installing(&app, move |s| s.accept(&ids)).await?
}

/// Starts installing into `<location>/Pincel ComfyUI` (AC-06 – AC-10).
#[tauri::command]
pub async fn comfyui_install(
    app: AppHandle,
    location: String,
    packages: Vec<String>,
) -> Result<(), InstallError> {
    installing(&app, move |s| s.install(&location, &packages))
        .await
        .map_err(failed)?
}

/// Continues the recorded install (Retry, Resume; AC-09).
#[tauri::command]
pub async fn comfyui_install_resume(app: AppHandle) -> Result<(), InstallError> {
    installing(&app, |s| s.resume()).await.map_err(failed)?
}

/// Cancels the running job.
#[tauri::command]
pub async fn comfyui_install_cancel(app: AppHandle) -> Result<(), String> {
    installing(&app, |s| s.cancel()).await
}

/// The managed installation's state.
#[tauri::command]
pub async fn comfyui_managed(app: AppHandle) -> Result<ManagedState, String> {
    installing(&app, |s| s.state()).await
}

/// Uses the complete installation at `location` (edge case).
#[tauri::command]
pub async fn comfyui_use_installation(
    app: AppHandle,
    location: String,
) -> Result<Status, InstallError> {
    installing(&app, move |s| s.adopt(&location))
        .await
        .map_err(failed)?
}

/// Adds and removes model packages (AC-11).
#[tauri::command]
pub async fn comfyui_change_packages(
    app: AppHandle,
    add: Vec<String>,
    remove: Vec<String>,
) -> Result<(), InstallError> {
    installing(&app, move |s| s.change_packages(&add, &remove))
        .await
        .map_err(failed)?
}

/// Updates ComfyUI to the current pins (AC-12).
#[tauri::command]
pub async fn comfyui_update(app: AppHandle) -> Result<(), InstallError> {
    installing(&app, |s| s.update()).await.map_err(failed)?
}

/// Deletes the managed installation (AC-13) or an incomplete one.
#[tauri::command]
pub async fn comfyui_uninstall(app: AppHandle) -> Result<(), InstallError> {
    installing(&app, |s| s.uninstall()).await.map_err(failed)?
}

/// Marks a generation as running or ended, so the installation is not
/// changed under it (AC-14). The generation jobs of comfyui-generate set
/// the flag in the backend themselves (`GenerationState`).
#[tauri::command]
pub async fn comfyui_set_generating(app: AppHandle, running: bool) -> Result<(), String> {
    installing(&app, move |s| s.set_generating(running)).await
}

fn generations<R: Runtime>(app: &AppHandle<R>) -> Result<Generations, String> {
    app.try_state::<GenerationState>()
        .map(|state| state.0.clone())
        .ok_or_else(|| "ComfyUI generation unavailable".to_string())
}

/// The server's checkpoints (with the catalog's model family) and LoRAs
/// for the Generate panel and `gen.models`; the reason when no server is
/// usable (comfyui-generate AC-02, AC-03, AC-18).
#[tauri::command]
pub async fn comfyui_generation_models(app: AppHandle) -> Result<Models, String> {
    let server = blocking(&app, |m| m.ensure_usable()).await??;
    Ok(generate::models(&server))
}

/// Starts a generation on the usable server (comfyui-generate AC-05 –
/// AC-07); its progress arrives as `comfyui:generation` events.
#[tauri::command]
pub async fn comfyui_generate(
    app: AppHandle,
    request: GenerateRequest,
) -> Result<Started, GenError> {
    let gens = generations(&app).map_err(|message| GenError::NotUsable { message })?;
    let server = blocking(&app, |m| m.ensure_usable())
        .await
        .map_err(|message| GenError::NotUsable { message })?
        .map_err(|message| GenError::NotUsable { message })?;
    tauri::async_runtime::spawn_blocking(move || gens.start(&server, &request))
        .await
        .map_err(|e| GenError::Failed {
            message: e.to_string(),
        })?
}

/// Cancels a generation: off the queue, or stopped if it runs (AC-09).
#[tauri::command]
pub async fn comfyui_generation_cancel(app: AppHandle, job: u64) -> Result<(), String> {
    generations(&app)?.cancel(job);
    Ok(())
}

/// Variant `index` of a finished generation, PNG bytes (AC-10).
#[tauri::command]
pub async fn comfyui_variant(
    app: AppHandle,
    job: u64,
    index: usize,
) -> Result<tauri::ipc::Response, String> {
    generations(&app)?
        .variant(job, index)
        .map(tauri::ipc::Response::new)
        .ok_or_else(|| "no such variant".to_string())
}

/// The editor dropped a generation's variants (Apply, Discard, a new
/// Generate, the tab closed); a running one is cancelled (AC-13, AC-21).
#[tauri::command]
pub async fn comfyui_generation_release(app: AppHandle, job: u64) -> Result<(), String> {
    generations(&app)?.release(job);
    Ok(())
}
