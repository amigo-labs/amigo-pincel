// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use tauri::menu::{
    AboutMetadata, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Manager, Wry};

// Sync helpers split out from the async commands so unit tests can
// exercise the FS round-trip without spinning up a Tokio runtime.
fn read_path(path: &str) -> Result<Vec<u8>, String> {
    fs::read(PathBuf::from(path)).map_err(|e| format!("read {path}: {e}"))
}

fn write_path(path: &str, bytes: &[u8]) -> Result<(), String> {
    fs::write(PathBuf::from(path), bytes).map_err(|e| format!("write {path}: {e}"))
}

#[tauri::command]
async fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    read_path(&path)
}

#[tauri::command]
async fn write_file_bytes(path: String, bytes: Vec<u8>) -> Result<(), String> {
    write_path(&path, &bytes)
}

#[derive(Deserialize)]
struct RecentMenuItem {
    id: String,
    name: String,
}

// Rebuild the `Open Recent` submenu from the JS side. The renderer
// owns the IDB-backed recents list; it re-calls this whenever the
// list changes. Empty list yields a disabled "(no recent files)"
// placeholder so the submenu remains visible but inert.
#[tauri::command]
async fn set_recent_menu(app: AppHandle, items: Vec<RecentMenuItem>) -> Result<(), String> {
    let menu = app.menu().ok_or_else(|| "no menu installed".to_string())?;
    let submenu = find_submenu(&menu, RECENT_MENU_ID)
        .ok_or_else(|| "open-recent submenu not found".to_string())?;
    rebuild_recent_submenu(&app, &submenu, &items).map_err(|e| e.to_string())
}

const RECENT_MENU_ID: &str = "open-recent";

/// Native menu labels in one UI language (spec §9.5). The renderer owns
/// the language choice and pushes it through `set_menu_language`.
struct Labels {
    file: &'static str,
    new: &'static str,
    open: &'static str,
    open_recent: &'static str,
    no_recent: &'static str,
    save: &'static str,
    save_as: &'static str,
    export_png: &'static str,
    export_sheet: &'static str,
    quit: &'static str,
    edit: &'static str,
    undo: &'static str,
    redo: &'static str,
    cut: &'static str,
    copy: &'static str,
    paste: &'static str,
    view: &'static str,
    zoom_in: &'static str,
    zoom_out: &'static str,
    reset_zoom: &'static str,
    help: &'static str,
    about: &'static str,
}

const LABELS_EN: Labels = Labels {
    file: "File",
    new: "New",
    open: "Open\u{2026}",
    open_recent: "Open Recent",
    no_recent: "(no recent files)",
    save: "Save",
    save_as: "Save As\u{2026}",
    export_png: "Export as PNG\u{2026}",
    export_sheet: "Export Sprite Sheet\u{2026}",
    quit: "Quit",
    edit: "Edit",
    undo: "Undo",
    redo: "Redo",
    cut: "Cut",
    copy: "Copy",
    paste: "Paste",
    view: "View",
    zoom_in: "Zoom In",
    zoom_out: "Zoom Out",
    reset_zoom: "Reset Zoom",
    help: "Help",
    about: "About Pincel",
};

const LABELS_DE: Labels = Labels {
    file: "Datei",
    new: "Neu",
    open: "\u{d6}ffnen\u{2026}",
    open_recent: "Zuletzt ge\u{f6}ffnet",
    no_recent: "(keine zuletzt ge\u{f6}ffneten Dateien)",
    save: "Speichern",
    save_as: "Speichern unter\u{2026}",
    export_png: "Als PNG exportieren\u{2026}",
    export_sheet: "Sprite-Sheet exportieren\u{2026}",
    quit: "Beenden",
    edit: "Bearbeiten",
    undo: "R\u{fc}ckg\u{e4}ngig",
    redo: "Wiederholen",
    cut: "Ausschneiden",
    copy: "Kopieren",
    paste: "Einf\u{fc}gen",
    view: "Ansicht",
    zoom_in: "Vergr\u{f6}\u{df}ern",
    zoom_out: "Verkleinern",
    reset_zoom: "Zoom zur\u{fc}cksetzen",
    help: "Hilfe",
    about: "\u{dc}ber Pincel",
};

/// True while the native menu shows German labels.
static GERMAN: AtomicBool = AtomicBool::new(false);

/// Labels for a renderer language code; anything but `de` is English.
fn labels_for(lang: &str) -> &'static Labels {
    if lang == "de" { &LABELS_DE } else { &LABELS_EN }
}

fn current_labels() -> &'static Labels {
    if GERMAN.load(Ordering::Relaxed) {
        &LABELS_DE
    } else {
        &LABELS_EN
    }
}

// Rebuild the whole native menu in `lang`, keeping the Open Recent
// entries the renderer pushed earlier.
#[tauri::command]
async fn set_menu_language(app: AppHandle, lang: String) -> Result<(), String> {
    GERMAN.store(lang == "de", Ordering::Relaxed);
    let recents: Vec<RecentMenuItem> = app
        .menu()
        .and_then(|menu| find_submenu(&menu, RECENT_MENU_ID))
        .and_then(|sub| sub.items().ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let entry = item.as_menuitem()?;
            let id = entry.id().0.strip_prefix("recent:")?.to_string();
            if id == "_empty" {
                return None;
            }
            Some(RecentMenuItem {
                id,
                name: entry.text().ok()?,
            })
        })
        .collect();
    let menu = build_menu(&app).map_err(|e| e.to_string())?;
    app.set_menu(menu.clone()).map_err(|e| e.to_string())?;
    if !recents.is_empty() {
        let submenu = find_submenu(&menu, RECENT_MENU_ID)
            .ok_or_else(|| "open-recent submenu not found".to_string())?;
        rebuild_recent_submenu(&app, &submenu, &recents).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Depth-first search for the submenu with `id`. Non-submenu entries are
/// *skipped*, not fatal — the previous split implementation used `?` on
/// `as_submenu()`, which aborted the scan at the first plain item, so
/// "Open Recent" (which sits after "New" / "Open…" in the File menu) was
/// never found and the recents submenu never synced.
fn find_submenu(menu: &Menu<Wry>, id: &str) -> Option<Submenu<Wry>> {
    fn walk(items: Vec<tauri::menu::MenuItemKind<Wry>>, id: &str) -> Option<Submenu<Wry>> {
        for item in items {
            let Some(sub) = item.as_submenu() else {
                continue;
            };
            if sub.id().0 == id {
                return Some(sub.clone());
            }
            if let Some(nested) = walk(sub.items().unwrap_or_default(), id) {
                return Some(nested);
            }
        }
        None
    }
    walk(menu.items().ok()?, id)
}

fn rebuild_recent_submenu(
    app: &AppHandle,
    submenu: &Submenu<Wry>,
    items: &[RecentMenuItem],
) -> tauri::Result<()> {
    // Clear by removing every existing item.
    if let Ok(existing) = submenu.items() {
        for item in existing {
            let _ = submenu.remove(&item);
        }
    }
    if items.is_empty() {
        let placeholder = MenuItemBuilder::with_id("recent:_empty", current_labels().no_recent)
            .enabled(false)
            .build(app)?;
        submenu.append(&placeholder)?;
        return Ok(());
    }
    for it in items {
        let id = format!("recent:{}", it.id);
        let entry = MenuItemBuilder::with_id(id, &it.name).build(app)?;
        submenu.append(&entry)?;
    }
    Ok(())
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let l = current_labels();
    // File items
    let new_item = MenuItemBuilder::with_id("menu:new", l.new)
        .accelerator("CmdOrCtrl+N")
        .build(app)?;
    let open_item = MenuItemBuilder::with_id("menu:open", l.open)
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let recent_placeholder = MenuItemBuilder::with_id("recent:_empty", l.no_recent)
        .enabled(false)
        .build(app)?;
    let recent_submenu = SubmenuBuilder::with_id(app, RECENT_MENU_ID, l.open_recent)
        .item(&recent_placeholder)
        .build()?;
    let save_item = MenuItemBuilder::with_id("menu:save", l.save)
        .accelerator("CmdOrCtrl+S")
        .build(app)?;
    let save_as_item = MenuItemBuilder::with_id("menu:saveAs", l.save_as)
        .accelerator("CmdOrCtrl+Shift+S")
        .build(app)?;

    // Export items: the mounted editor serves them (Pixel mode both,
    // Image mode the PNG export); unserved ids are ignored by the shell.
    let export_png_item = MenuItemBuilder::with_id("menu:exportPng", l.export_png)
        .accelerator("CmdOrCtrl+E")
        .build(app)?;
    let export_sheet_item = MenuItemBuilder::with_id("menu:exportSheet", l.export_sheet)
        .accelerator("CmdOrCtrl+Shift+E")
        .build(app)?;

    let file = SubmenuBuilder::new(app, l.file)
        .item(&new_item)
        .item(&open_item)
        .item(&recent_submenu)
        .separator()
        .item(&save_item)
        .item(&save_as_item)
        .separator()
        .item(&export_png_item)
        .item(&export_sheet_item)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some(l.quit))?)
        .build()?;

    // Edit items
    let undo_item = MenuItemBuilder::with_id("menu:undo", l.undo)
        .accelerator("CmdOrCtrl+Z")
        .build(app)?;
    let redo_item = MenuItemBuilder::with_id("menu:redo", l.redo)
        .accelerator("CmdOrCtrl+Shift+Z")
        .build(app)?;
    let edit = SubmenuBuilder::new(app, l.edit)
        .item(&undo_item)
        .item(&redo_item)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some(l.cut))?)
        .item(&PredefinedMenuItem::copy(app, Some(l.copy))?)
        .item(&PredefinedMenuItem::paste(app, Some(l.paste))?)
        .build()?;

    // View items
    let zoom_in = MenuItemBuilder::with_id("menu:zoomIn", l.zoom_in)
        .accelerator("CmdOrCtrl+Plus")
        .build(app)?;
    let zoom_out = MenuItemBuilder::with_id("menu:zoomOut", l.zoom_out)
        .accelerator("CmdOrCtrl+-")
        .build(app)?;
    let reset_zoom = MenuItemBuilder::with_id("menu:resetZoom", l.reset_zoom)
        .accelerator("CmdOrCtrl+0")
        .build(app)?;
    let view = SubmenuBuilder::new(app, l.view)
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&reset_zoom)
        .build()?;

    let help = SubmenuBuilder::new(app, l.help)
        .item(&PredefinedMenuItem::about(
            app,
            Some(l.about),
            Some(AboutMetadata {
                name: Some("Pincel".to_string()),
                ..Default::default()
            }),
        )?)
        .build()?;

    Menu::with_items(app, &[&file, &edit, &view, &help])
}

// Pull the first non-binary CLI argument that looks like a sprite path.
// Tauri 2 hands `argv[0]` as the binary; we forward any subsequent path.
// File-association launches pass the file as `argv[1]` on every
// supported platform.
fn first_file_arg(argv: &[String]) -> Option<String> {
    argv.iter().skip(1).find(|a| !a.starts_with('-')).cloned()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // Second-instance: foreground the existing window and
            // forward any opened file to the renderer. Errors are
            // logged but non-fatal.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
            if let Some(path) = first_file_arg(&argv) {
                let _ = app.emit("open-file", path);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            read_file_bytes,
            write_file_bytes,
            set_recent_menu,
            set_menu_language
        ])
        .setup(|app| {
            let handle = app.handle();
            let menu = build_menu(handle)?;
            app.set_menu(menu)?;
            // First-launch CLI arg (file-association on Windows /
            // Linux, also macOS when launched from a shell).
            let argv: Vec<String> = std::env::args().collect();
            if let Some(path) = first_file_arg(&argv) {
                let app_handle = app.handle().clone();
                // Defer until the renderer is listening. A short
                // setTimeout-equivalent on the Rust side: spawn a task
                // that waits for the webview to fire `tauri://load`
                // before we emit. For simplicity we emit immediately
                // and rely on the renderer's `listen` arming before
                // first paint.
                let _ = app_handle.emit("open-file", path);
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
// on macOS; everywhere else the handler is a no-op.
#[allow(unused_variables)]
fn handle_run_event(app: &AppHandle, event: tauri::RunEvent) {
    #[cfg(target_os = "macos")]
    if let tauri::RunEvent::Opened { urls } = event {
        for url in urls {
            if let Ok(path) = url.to_file_path() {
                if let Some(s) = path.to_str() {
                    let _ = app.emit("open-file", s.to_string());
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

    #[test]
    fn labels_for_german_and_fallback() {
        assert_eq!(labels_for("de").file, "Datei");
        assert_eq!(labels_for("en").file, "File");
        assert_eq!(labels_for("fr").file, "File");
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
