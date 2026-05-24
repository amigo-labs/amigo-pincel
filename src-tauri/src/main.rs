// Prevents an additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;

// Sync helpers split out from the async commands so unit tests can
// exercise the FS round-trip without spinning up a Tokio runtime. The
// commands themselves stay `async fn` so Tauri schedules them off the
// main IPC thread.
fn read_path(path: &str) -> Result<Vec<u8>, String> {
    fs::read(PathBuf::from(path)).map_err(|e| format!("read {path}: {e}"))
}

fn write_path(path: &str, bytes: &[u8]) -> Result<(), String> {
    fs::write(PathBuf::from(path), bytes).map_err(|e| format!("write {path}: {e}"))
}

// Read a file by absolute path and return its bytes to the JS side.
#[tauri::command]
async fn read_file_bytes(path: String) -> Result<Vec<u8>, String> {
    read_path(&path)
}

// Write `bytes` to `path`, replacing the file if it exists.
#[tauri::command]
async fn write_file_bytes(path: String, bytes: Vec<u8>) -> Result<(), String> {
    write_path(&path, &bytes)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![read_file_bytes, write_file_bytes])
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
