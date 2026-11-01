//! Removes the service worker an earlier desktop build registered.
//!
//! Up to 1.5.0 the desktop bundle shipped the PWA service worker. WebView2
//! cannot fetch a worker script from Tauri's custom protocol, so that worker
//! never updated and kept serving the bundle it had cached first: an updated
//! app showed an old UI. Desktop builds no longer register a worker; this
//! deletes the frozen registration and its Cache Storage before the webview
//! starts. IndexedDB, where autosave, recents and prefs live, is a sibling
//! directory and stays untouched.

use std::fs;
use std::io;
use std::path::Path;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Manager, Runtime};

/// WebView2's service-worker store (registrations, scripts, Cache Storage)
/// inside the app's local data directory.
const SERVICE_WORKER_DIR: [&str; 3] = ["EBWebView", "Default", "Service Worker"];

/// Deletes the service-worker store under `data_dir`. `Ok(true)` when there
/// was one; `Ok(false)` when there is nothing to remove (macOS, Linux, or a
/// copy that never registered a worker).
pub fn remove_service_workers(data_dir: &Path) -> io::Result<bool> {
    let dir = SERVICE_WORKER_DIR
        .iter()
        .fold(data_dir.to_path_buf(), |path, part| path.join(part));
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

/// Runs the cleanup while the app starts, before the main window creates its
/// webview. Register it after the single-instance plugin: that one exits a
/// second launch first, so the files of a running window are never touched.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("webview-data")
        .setup(|app, _api| {
            if let Ok(dir) = app.path().app_local_data_dir()
                && let Err(err) = remove_service_workers(&dir)
            {
                eprintln!("could not remove the stale service worker: {err}");
            }
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        fs::create_dir_all(path.parent().expect("test path has a parent"))
            .expect("create test dirs");
        fs::write(path, b"x").expect("write test file");
    }

    #[test]
    fn remove_service_workers_deletes_the_store_and_keeps_indexeddb() {
        let data = tempfile::tempdir().expect("tempdir");
        let profile = data.path().join("EBWebView").join("Default");
        touch(
            &profile
                .join("Service Worker")
                .join("Database")
                .join("CURRENT"),
        );
        touch(
            &profile
                .join("Service Worker")
                .join("CacheStorage")
                .join("index"),
        );
        touch(&profile.join("IndexedDB").join("pincel.leveldb"));

        assert!(remove_service_workers(data.path()).expect("remove"));
        assert!(!profile.join("Service Worker").exists());
        assert!(profile.join("IndexedDB").join("pincel.leveldb").exists());
    }

    #[test]
    fn remove_service_workers_without_a_store_is_a_no_op() {
        let data = tempfile::tempdir().expect("tempdir");
        assert!(!remove_service_workers(data.path()).expect("no-op"));
    }
}
