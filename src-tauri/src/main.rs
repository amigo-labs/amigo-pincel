// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager};

mod assistant;
mod comfyui;
mod fonts;
#[cfg(target_os = "linux")]
mod linux_install;
mod mcp;
mod menu_labels;
mod native_menu;
mod open_files;
#[cfg(test)]
mod test_support;
mod updater;
mod webview_data;

// Sync helpers split out from the async commands so unit tests can
// exercise the FS round-trip without spinning up a Tokio runtime.
fn read_path(path: &str) -> Result<Vec<u8>, String> {
    fs::read(PathBuf::from(path)).map_err(|e| format!("read {path}: {e}"))
}

fn write_path(path: &str, bytes: &[u8]) -> Result<(), String> {
    fs::write(PathBuf::from(path), bytes).map_err(|e| format!("write {path}: {e}"))
}

/// For each path, whether it names an existing file — the recent files
/// drop entries whose file is gone (docs/specs/file-access.md AC-30,
/// AC-31). A path on a drive that is not connected counts as gone.
fn files_present(paths: &[String]) -> Vec<bool> {
    paths.iter().map(|p| Path::new(p).is_file()).collect()
}

#[tauri::command]
async fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    read_path(&path)
}

#[tauri::command]
async fn files_exist(paths: Vec<String>) -> Vec<bool> {
    files_present(&paths)
}

#[tauri::command]
async fn write_file_bytes(path: String, bytes: Vec<u8>) -> Result<(), String> {
    write_path(&path, &bytes)
}

/// Whether this build installs a native menu bar (spec §11.3).
///
/// macOS only: there the menu bar belongs to the screen rather than the
/// window, and the App menu with Quit / About is a platform requirement.
/// On Windows and Linux it would stack on top of the in-app menu bar,
/// which lists a superset of its entries, so the app ships that one alone.
///
/// A `const` rather than `#[cfg]` on every menu item: CI builds this crate
/// on Linux only, and an attribute would take the whole macOS path out of
/// the type check.
const NATIVE_MENU: bool = cfg!(target_os = "macos");

/// The start screen's recent files (the renderer pushes them with
/// `set_recent_menu`); kept here so a rebuilt menu lists them again.
static RECENTS: Mutex<Vec<native_menu::RecentEntry>> = Mutex::new(Vec::new());

/// The active editor's menus (`set_editor_menu`), or `None` while the
/// start screen shows (docs/specs/macos-native-menu.md).
static EDITOR_MENU: Mutex<Option<Vec<native_menu::Node>>> = Mutex::new(None);

// Rebuild the `Open Recent` submenu from the JS side. The renderer
// owns the IDB-backed recents list; it re-calls this whenever the
// list changes. Empty list yields a disabled "(no recent files)"
// placeholder so the submenu remains visible but inert.
#[tauri::command]
async fn set_recent_menu(
    app: AppHandle,
    items: Vec<native_menu::RecentEntry>,
) -> Result<(), String> {
    if let Ok(mut recents) = RECENTS.lock() {
        *recents = items;
    }
    if !NATIVE_MENU {
        // No native menu to feed; the in-app File menu lists the recents.
        return Ok(());
    }
    rebuild_menu(&app)
}

// The active editor's menus, mirrored into the native menu bar while its
// tab shows; `None` brings back the start-screen menu
// (docs/specs/macos-native-menu.md AC-03, AC-15). A failed build leaves
// the previous menu in place and rejects (AC-14).
#[tauri::command]
async fn set_editor_menu(
    app: AppHandle,
    menus: Option<Vec<native_menu::Node>>,
) -> Result<(), String> {
    if !NATIVE_MENU {
        // Only macOS draws a native menu bar; elsewhere the in-app bar shows.
        return Ok(());
    }
    if let Ok(mut current) = EDITOR_MENU.lock() {
        *current = menus;
    }
    rebuild_menu(&app)
}

// Rebuild the whole native menu in `lang`.
#[tauri::command]
async fn set_menu_language(app: AppHandle, lang: String) -> Result<(), String> {
    menu_labels::set_language(&lang);
    if !NATIVE_MENU {
        // Nothing to relabel: the in-app menu bar follows the locale itself.
        return Ok(());
    }
    rebuild_menu(&app)
}

/// Accelerators the renderer rebound (docs/specs/power-user-shortcuts-keymap.md):
/// menu item id → accelerator, an empty string for none. Items not listed
/// keep their default.
static ACCELERATORS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// The accelerator of menu item `id`: the rebound one, else `default`.
fn accelerator(id: &str, default: &str) -> Option<String> {
    let rebound = ACCELERATORS
        .lock()
        .ok()
        .and_then(|map| map.iter().find(|(k, _)| k == id).map(|(_, v)| v.clone()));
    match rebound {
        Some(a) if a.is_empty() => None,
        Some(a) => Some(a),
        None => Some(default.to_string()),
    }
}

// Push the keymap's rebound native-menu chords (`menu:open` →
// `CmdOrCtrl+U`) and rebuild the menu, so its accelerators and the hints
// it draws follow the user's keymap. A no-op where there is no native menu.
#[tauri::command]
async fn set_menu_accelerators(app: AppHandle, map: HashMap<String, String>) -> Result<(), String> {
    if let Ok(mut current) = ACCELERATORS.lock() {
        *current = map.into_iter().collect();
    }
    if !NATIVE_MENU {
        return Ok(());
    }
    rebuild_menu(&app)
}

/// The native menu bar for the current state: the editor's menus while
/// one was pushed, else the start screen's.
fn menu_nodes() -> Vec<native_menu::Node> {
    let l = menu_labels::current();
    let updatable = updater::supported();
    let editor = EDITOR_MENU.lock().ok().and_then(|m| m.clone());
    match editor {
        Some(menus) => native_menu::editor_menus(
            l,
            updatable,
            accelerator(native_menu::MENU_SETTINGS, native_menu::SETTINGS_DEFAULT),
            &menus,
        ),
        None => {
            let recents = RECENTS.lock().map(|r| r.clone()).unwrap_or_default();
            native_menu::start_menus(l, updatable, &recents, &accelerator)
        }
    }
}

// Rebuild the whole native menu (labels, accelerators, recents, the
// mirrored editor menus). On error the previous menu stays installed.
fn rebuild_menu(app: &AppHandle) -> Result<(), String> {
    let menu = native_menu::materialize(app, &menu_nodes()).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    Ok(())
}

// Pull the first non-binary CLI argument that looks like a sprite path.
// Tauri 2 hands `argv[0]` as the binary; we forward any subsequent path.
// File-association launches pass the file as `argv[1]` on every
// supported platform.
fn first_file_arg(argv: &[String]) -> Option<String> {
    argv.iter().skip(1).find(|a| !a.starts_with('-')).cloned()
}

/// The main window starts hidden (`visible: false`) and shows once the
/// page has loaded, so it never flashes empty; after this long it shows
/// regardless (amigo-blinzel's 400 ms).
const SHOW_DEADLINE: Duration = Duration::from_millis(400);

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    if let Some(code) = linux_install::run_cli(&std::env::args().collect::<Vec<_>>()) {
        std::process::exit(code);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // Second-instance: foreground the existing window and
            // forward any opened file to the renderer. Errors are
            // logged but non-fatal.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            if let Some(path) = first_file_arg(&argv) {
                open_files::deliver(app, path);
            }
        }))
        // After single-instance, so a second launch exits before it runs.
        .plugin(webview_data::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updater::UpdaterState::default())
        .manage(open_files::OpenFiles::default())
        .manage(fonts::SystemFonts::default())
        .invoke_handler(tauri::generate_handler![
            read_file_bytes,
            files_exist,
            write_file_bytes,
            set_recent_menu,
            set_editor_menu,
            set_menu_language,
            set_menu_accelerators,
            open_files::take_open_files,
            fonts::system_font_faces,
            fonts::system_font_file,
            updater::updater_state,
            updater::updater_check,
            updater::updater_set_mode,
            updater::updater_check_on_open,
            updater::updater_mark_announced,
            updater::updater_arm_restart,
            updater::updater_release,
            comfyui::comfyui_settings,
            comfyui::comfyui_set_server,
            comfyui::comfyui_choose_folder,
            comfyui::comfyui_test_url,
            comfyui::comfyui_connect,
            comfyui::comfyui_status,
            comfyui::comfyui_usable,
            comfyui::comfyui_install_info,
            comfyui::comfyui_check_location,
            comfyui::comfyui_accept_licenses,
            comfyui::comfyui_install,
            comfyui::comfyui_install_resume,
            comfyui::comfyui_install_cancel,
            comfyui::comfyui_managed,
            comfyui::comfyui_use_installation,
            comfyui::comfyui_change_packages,
            comfyui::comfyui_update,
            comfyui::comfyui_uninstall,
            comfyui::comfyui_set_generating,
            comfyui::comfyui_generation_models,
            comfyui::comfyui_generate,
            comfyui::comfyui_generation_cancel,
            comfyui::comfyui_variant,
            comfyui::comfyui_generation_release,
            mcp::mcp_status,
            mcp::mcp_enable,
            mcp::mcp_disable,
            mcp::mcp_regenerate_token,
            mcp::mcp_disconnect,
            mcp::mcp_call_result,
            mcp::mcp_call_waiting,
            mcp::mcp_chat_start,
            mcp::mcp_chat_end,
            mcp::mcp_chat_answering,
            mcp::mcp_chat_image_access,
            assistant::assistant_clis,
            assistant::assistant_models,
            assistant::assistant_send,
            assistant::assistant_stop,
            assistant::assistant_end
        ])
        .on_page_load(|webview, payload| match payload.event() {
            PageLoadEvent::Started => open_files::reset(webview.app_handle()),
            PageLoadEvent::Finished => {
                let _ = webview.window().show();
            }
        })
        .setup(|app| {
            let handle = app.handle();
            // Stopped until the user allows AI agents or a chat starts it
            // (mcp-live-desktop AC-01).
            app.manage(mcp::create(handle));
            // The Assistant chat's CLIs (ai-chat-panel); clears chat
            // folders a crashed run left behind (AC-22).
            app.manage(assistant::create(handle));
            if NATIVE_MENU {
                let menu = native_menu::materialize(handle, &menu_nodes())?;
                app.set_menu(menu)?;
            }
            comfyui::setup(handle);
            let deadline = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(SHOW_DEADLINE);
                show_main_window(&deadline);
            });
            // First-launch CLI arg (file-association on Windows /
            // Linux, also macOS when launched from a shell). The page is
            // not loaded yet: the path waits until the renderer asks.
            let argv: Vec<String> = std::env::args().collect();
            if let Some(path) = first_file_arg(&argv) {
                open_files::deliver(handle, path);
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            // The id field carries our `menu:<action>` / `recent:<docId>`
            // string. The renderer parses the prefix to dispatch.
            let _ = app.emit("menu", event.id().0.clone());
        })
        .build(tauri::generate_context!())
        .expect("error while building Pincel")
        .run(handle_run_event);
}

// macOS file-open events (Finder double-click, `open -a`, Dock drop)
// arrive here as `RunEvent::Opened { urls }`. The variant only exists
// on macOS. On exit, a ComfyUI the app started is ended and an update
// held for it (Windows) is installed.
fn handle_run_event(app: &AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::Exit = event {
        assistant::shutdown(app);
        comfyui::shutdown(app);
        if let Some(endpoint) = app.try_state::<mcp::Endpoint>() {
            endpoint.shutdown();
        }
        updater::install_on_exit(app);
    }
    #[cfg(target_os = "macos")]
    if let tauri::RunEvent::Opened { urls } = event {
        for url in urls {
            if let Ok(path) = url.to_file_path() {
                if let Some(s) = path.to_str() {
                    open_files::deliver(app, s.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_write_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blob.bin");
        let path_str = path.to_string_lossy().into_owned();
        let payload = vec![1u8, 2, 3, 4, 5];
        write_path(&path_str, &payload).unwrap();
        let got = read_path(&path_str).unwrap();
        assert_eq!(got, payload);
    }

    #[test]
    fn read_missing_path_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.bin");
        let err = read_path(&path.to_string_lossy()).unwrap_err();
        assert!(err.starts_with("read "));
    }

    #[test]
    fn files_present_tells_existing_files_from_missing_paths_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("there.png");
        write_path(&file.to_string_lossy(), &[1, 2, 3]).unwrap();
        let paths = vec![
            file.to_string_lossy().into_owned(),
            dir.path().join("gone.png").to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            String::new(),
        ];
        assert_eq!(files_present(&paths), vec![true, false, false, false]);
    }

    #[test]
    fn first_file_arg_skips_binary() {
        let argv = vec![
            "/usr/bin/pincel".to_string(),
            "/tmp/foo.aseprite".to_string(),
        ];
        assert_eq!(first_file_arg(&argv), Some("/tmp/foo.aseprite".to_string()));
    }

    #[test]
    fn first_file_arg_skips_flags() {
        let argv = vec![
            "/usr/bin/pincel".to_string(),
            "--debug".to_string(),
            "/tmp/foo.aseprite".to_string(),
        ];
        assert_eq!(first_file_arg(&argv), Some("/tmp/foo.aseprite".to_string()));
    }

    #[test]
    fn first_file_arg_none_when_only_binary() {
        let argv = vec!["/usr/bin/pincel".to_string()];
        assert_eq!(first_file_arg(&argv), None);
    }

    /// IHDR colour type of a PNG, `None` if `bytes` is no PNG.
    fn png_color_type(bytes: &[u8]) -> Option<u8> {
        bytes.starts_with(b"\x89PNG\r\n\x1a\n").then(|| bytes[25])
    }

    // Tauri's codegen decodes the bundle icons and rejects anything but
    // RGBA PNGs (colour type 6); an indexed 256 px entry in icon.ico broke
    // the first Windows release build.
    #[test]
    fn bundle_icons_are_rgba_pngs() {
        let ico: &[u8] = include_bytes!("../icons/icon.ico");
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        assert!(count > 0, "icon.ico has no entries");
        for i in 0..count {
            let entry = &ico[6 + 16 * i..22 + 16 * i];
            let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
            let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
            if let Some(color_type) = png_color_type(&ico[offset..offset + size]) {
                assert_eq!(color_type, 6, "icon.ico entry {i} is not an RGBA PNG");
            }
        }
        for (name, png) in [
            ("32x32.png", include_bytes!("../icons/32x32.png").as_slice()),
            (
                "128x128.png",
                include_bytes!("../icons/128x128.png").as_slice(),
            ),
            (
                "128x128@2x.png",
                include_bytes!("../icons/128x128@2x.png").as_slice(),
            ),
            ("icon.png", include_bytes!("../icons/icon.png").as_slice()),
        ] {
            assert_eq!(png_color_type(png), Some(6), "{name} is not an RGBA PNG");
        }
    }

    // The NSIS hooks register every file association of tauri.conf.json
    // under its ProgID (the association's `name`).
    #[test]
    fn nsis_hooks_cover_every_file_association() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let hooks = include_str!("../windows/hooks.nsh");
        for association in conf["bundle"]["fileAssociations"].as_array().unwrap() {
            let progid = association["name"].as_str().unwrap();
            for ext in association["ext"].as_array().unwrap() {
                let line = format!(
                    "!insertmacro ${{OP}} \"{}\" \"{progid}\"",
                    ext.as_str().unwrap()
                );
                assert!(hooks.contains(&line), "missing in hooks.nsh: {line}");
            }
        }
    }

    #[test]
    fn write_then_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blob.bin");
        let path_str = path.to_string_lossy().into_owned();
        write_path(&path_str, b"first").unwrap();
        write_path(&path_str, b"second").unwrap();
        let got = read_path(&path_str).unwrap();
        assert_eq!(got, b"second");
    }
}
