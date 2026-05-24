// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use tauri::menu::{
    AboutMetadata, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Wry};

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

fn find_submenu(menu: &Menu<Wry>, id: &str) -> Option<Submenu<Wry>> {
    for item in menu.items().ok()? {
        let sub = item.as_submenu()?;
        if sub.id().0 == id {
            return Some(sub.clone());
        }
        if let Some(nested) = find_submenu_in(sub, id) {
            return Some(nested);
        }
    }
    None
}

fn find_submenu_in(sub: &Submenu<Wry>, id: &str) -> Option<Submenu<Wry>> {
    for item in sub.items().ok()? {
        let inner = item.as_submenu()?;
        if inner.id().0 == id {
            return Some(inner.clone());
        }
        if let Some(nested) = find_submenu_in(inner, id) {
            return Some(nested);
        }
    }
    None
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
        let placeholder = MenuItemBuilder::with_id("recent:_empty", "(no recent files)")
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
    // File items
    let new_item = MenuItemBuilder::with_id("menu:new", "New")
        .accelerator("CmdOrCtrl+N")
        .build(app)?;
    let open_item = MenuItemBuilder::with_id("menu:open", "Open\u{2026}")
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let recent_placeholder = MenuItemBuilder::with_id("recent:_empty", "(no recent files)")
        .enabled(false)
        .build(app)?;
    let recent_submenu = SubmenuBuilder::with_id(app, RECENT_MENU_ID, "Open Recent")
        .item(&recent_placeholder)
        .build()?;
    let save_item = MenuItemBuilder::with_id("menu:save", "Save")
        .accelerator("CmdOrCtrl+S")
        .build(app)?;
    let save_as_item = MenuItemBuilder::with_id("menu:saveAs", "Save As\u{2026}")
        .accelerator("CmdOrCtrl+Shift+S")
        .build(app)?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&new_item)
        .item(&open_item)
        .item(&recent_submenu)
        .separator()
        .item(&save_item)
        .item(&save_as_item)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("Quit"))?)
        .build()?;

    // Edit items
    let undo_item = MenuItemBuilder::with_id("menu:undo", "Undo")
        .accelerator("CmdOrCtrl+Z")
        .build(app)?;
    let redo_item = MenuItemBuilder::with_id("menu:redo", "Redo")
        .accelerator("CmdOrCtrl+Shift+Z")
        .build(app)?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&undo_item)
        .item(&redo_item)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some("Cut"))?)
        .item(&PredefinedMenuItem::copy(app, Some("Copy"))?)
        .item(&PredefinedMenuItem::paste(app, Some("Paste"))?)
        .build()?;

    // View items
    let zoom_in = MenuItemBuilder::with_id("menu:zoomIn", "Zoom In")
        .accelerator("CmdOrCtrl+Plus")
        .build(app)?;
    let zoom_out = MenuItemBuilder::with_id("menu:zoomOut", "Zoom Out")
        .accelerator("CmdOrCtrl+-")
        .build(app)?;
    let reset_zoom = MenuItemBuilder::with_id("menu:resetZoom", "Reset Zoom")
        .accelerator("CmdOrCtrl+0")
        .build(app)?;
    let view = SubmenuBuilder::new(app, "View")
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&reset_zoom)
        .build()?;

    let help = SubmenuBuilder::new(app, "Help")
        .item(&PredefinedMenuItem::about(
            app,
            Some("About Pincel"),
            Some(AboutMetadata {
                name: Some("Pincel".to_string()),
                ..Default::default()
            }),
        )?)
        .build()?;

    Menu::with_items(app, &[&file, &edit, &view, &help])
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            read_file_bytes,
            write_file_bytes,
            set_recent_menu
        ])
        .setup(|app| {
            let handle = app.handle();
            let menu = build_menu(handle)?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| {
            // The id field carries our `menu:<action>` / `recent:<docId>`
            // string. The renderer parses the prefix to dispatch.
            let _ = app.emit("menu", event.id().0.clone());
        })
        .run(tauri::generate_context!())
        .expect("error while running Pincel");
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
