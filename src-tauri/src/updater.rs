//! Silent self-updater for the desktop app, after amigo-blinzel's:
//! at most one check a day, 10 s after start, off the start path; the
//! update is downloaded, its signature verified against the public key
//! in `tauri.conf.json` and installed for the next start. Help menu:
//! "Check for Updates…" and "Install Updates Automatically".
//! See docs/RELEASING.md and docs/specs/pincel.md §15, 2026-10-26.
//!
//! macOS and the Linux AppImage replace the installed bundle right away.
//! Windows keeps the verified installer until the app exits and runs it
//! then (passive NSIS / MSI), because it can't replace a running app.
//! Debug builds and Linux packages owned by a package manager (.deb /
//! .rpm) never update themselves.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::current_labels;

/// Menu id of "Check for Updates…".
pub const MENU_CHECK: &str = "updater:check";
/// Menu id of the "Install Updates Automatically" checkbox.
pub const MENU_AUTO: &str = "updater:auto";

const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
const START_DELAY: Duration = Duration::from_secs(10);
const SETTINGS_FILE: &str = "updater.json";

/// What the updater remembers between starts (`<app config>/updater.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Check and install in the background.
    pub auto_update: bool,
    /// Unix seconds of the last background check.
    pub last_check: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_update: true,
            last_check: 0,
        }
    }
}

/// Settings from the file's bytes; a damaged file counts as no file.
fn parse_settings(bytes: &[u8]) -> Settings {
    serde_json::from_slice(bytes).unwrap_or_default()
}

/// Whether a background check is due at `now` (Unix seconds). A clock
/// that went back counts as due, so a wrong date can't stop updates.
fn is_due(settings: &Settings, now: u64) -> bool {
    settings.auto_update
        && (now < settings.last_check || now - settings.last_check >= CHECK_INTERVAL_SECS)
}

/// Whether this copy may update itself: a release build, and on Linux
/// only the AppImage (packages belong to the package manager).
pub fn supported() -> bool {
    if cfg!(debug_assertions) {
        return false;
    }
    if cfg!(target_os = "linux") {
        return std::env::var_os("APPIMAGE").is_some();
    }
    true
}

fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(SETTINGS_FILE))
}

/// The stored settings, or the defaults.
pub fn load_settings(app: &AppHandle) -> Settings {
    settings_path(app)
        .and_then(|path| fs::read(path).ok())
        .map(|bytes| parse_settings(&bytes))
        .unwrap_or_default()
}

fn save_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app).ok_or_else(|| "no app config directory".to_string())?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Updater state shared by the background check, the menu and the exit
/// hook (`app.manage`).
#[derive(Default)]
pub struct UpdaterState {
    inner: Mutex<Pending>,
}

#[derive(Default)]
struct Pending {
    /// Version downloaded and installed (or held) in this session.
    ready: Option<String>,
    /// Windows: the verified installer, run when the app exits.
    held: Option<(Update, Vec<u8>)>,
}

enum Outcome {
    UpToDate,
    Ready(String),
}

/// Check, download, verify and install (or hold) one update. The state
/// lock is held throughout, so a manual check waits for a running
/// background one instead of downloading twice.
async fn fetch(app: &AppHandle) -> Result<Outcome, String> {
    let state = app.state::<UpdaterState>();
    let mut pending = state.inner.lock().await;
    if let Some(version) = &pending.ready {
        return Ok(Outcome::Ready(version.clone()));
    }
    let updater = app
        .updater_builder()
        .restart_after_install(false)
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(Outcome::UpToDate);
    };
    // `download` verifies the signature; nothing unsigned gets further.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    let version = update.version.clone();
    if cfg!(windows) {
        pending.held = Some((update, bytes));
    } else {
        update.install(bytes).map_err(|e| e.to_string())?;
    }
    pending.ready = Some(version.clone());
    Ok(Outcome::Ready(version))
}

fn record_check(app: &AppHandle, now: u64) {
    // Re-read so a menu toggle made meanwhile survives.
    let mut settings = load_settings(app);
    settings.last_check = now;
    if let Err(e) = save_settings(app, &settings) {
        eprintln!("pincel: saving updater settings failed: {e}");
    }
}

/// Start the daily background check, 10 s after launch, on its own
/// thread. A failed check counts as a check, so a broken endpoint is
/// retried tomorrow rather than on every start.
pub fn spawn_background_check(app: &AppHandle) {
    if !supported() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(START_DELAY);
        let now = now_secs();
        if !is_due(&load_settings(&app), now) {
            return;
        }
        let result = tauri::async_runtime::block_on(fetch(&app));
        record_check(&app, now);
        if let Err(e) = result {
            eprintln!("pincel: update check failed: {e}");
        }
    });
}

/// "Check for Updates…": check now and report the result in a dialog.
pub fn check_now(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let result = tauri::async_runtime::block_on(fetch(&app));
        record_check(&app, now_secs());
        let l = current_labels();
        let text = match result {
            Ok(Outcome::UpToDate) => l.update_none.to_string(),
            Ok(Outcome::Ready(version)) => l.update_ready.replace("{version}", &version),
            Err(e) => l.update_failed.replace("{error}", &e),
        };
        app.dialog().message(text).title("Pincel").show(|_| {});
    });
}

/// "Install Updates Automatically": flip and store the setting (the menu
/// checkbox toggles itself).
pub fn toggle_auto(app: &AppHandle) {
    let mut settings = load_settings(app);
    settings.auto_update = !settings.auto_update;
    if let Err(e) = save_settings(app, &settings) {
        eprintln!("pincel: saving updater settings failed: {e}");
    }
}

/// Run an update held for exit (Windows). The installer takes over and
/// the process ends here.
pub fn install_on_exit(app: &AppHandle) {
    let state = app.state::<UpdaterState>();
    // A check still running at exit holds the lock and has nothing held
    // yet; don't wait for it.
    let held = state
        .inner
        .try_lock()
        .ok()
        .and_then(|mut pending| pending.held.take());
    if let Some((update, bytes)) = held
        && let Err(e) = update.install(bytes)
    {
        eprintln!("pincel: installing the update failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_settings_missing_fields_use_defaults() {
        let s = parse_settings(br#"{"last_check": 42}"#);
        assert_eq!(
            s,
            Settings {
                auto_update: true,
                last_check: 42
            }
        );
    }

    #[test]
    fn parse_settings_damaged_file_is_default() {
        assert_eq!(parse_settings(b"{not json"), Settings::default());
    }

    #[test]
    fn parse_settings_round_trip() {
        let s = Settings {
            auto_update: false,
            last_check: 1_700_000_000,
        };
        let bytes = serde_json::to_vec(&s).unwrap();
        assert_eq!(parse_settings(&bytes), s);
    }

    #[test]
    fn is_due_first_start() {
        assert!(is_due(&Settings::default(), 1_700_000_000));
    }

    #[test]
    fn is_due_once_a_day() {
        let s = Settings {
            auto_update: true,
            last_check: 1_000_000,
        };
        assert!(!is_due(&s, 1_000_000 + CHECK_INTERVAL_SECS - 1));
        assert!(is_due(&s, 1_000_000 + CHECK_INTERVAL_SECS));
    }

    #[test]
    fn is_due_clock_went_back() {
        let s = Settings {
            auto_update: true,
            last_check: 2_000_000,
        };
        assert!(is_due(&s, 1_000_000));
    }

    #[test]
    fn is_due_never_when_off() {
        let s = Settings {
            auto_update: false,
            last_check: 0,
        };
        assert!(!is_due(&s, 1_700_000_000));
    }
}
