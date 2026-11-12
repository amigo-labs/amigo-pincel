//! Self-updater for the desktop app, after amigo-blinzel's: at most one
//! background check a day, 10 s after start, off the start path; the
//! update is downloaded, its signature verified against the public key
//! in `tauri.conf.json` and installed for the next start. The user's
//! update choice (install automatically / notify / never check) decides
//! what the background check does; Help ▸ "Check for Updates…" always
//! checks. See docs/RELEASING.md, docs/specs/desktop-app.md and
//! docs/specs/pincel.md §15, 2026-10-26 and 2026-11-11.
//!
//! macOS and the Linux AppImage replace the installed bundle right away.
//! Windows keeps the verified installer until the app exits and runs it
//! then (passive NSIS / MSI), because it can't replace a running app.
//! Debug builds, Linux packages owned by a package manager (.deb / .rpm)
//! and an AppImage that wasn't installed with `--install` never update
//! themselves.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::PoisonError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::current_labels;

/// Menu id of "Check for Updates…".
pub const MENU_CHECK: &str = "updater:check";

const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
const START_DELAY: Duration = Duration::from_secs(10);
const SETTINGS_FILE: &str = "updater.json";

/// How updates are handled: the update choice (docs/specs/desktop-app.md,
/// Revision 2026-11-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Check, download and install in the background.
    Auto,
    /// Check in the background and announce a new version; install on request.
    Notify,
    /// No background check; only a manual check looks.
    Off,
}

/// What the updater remembers between starts (`<app config>/updater.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The update choice; written only when the user confirms or changes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    /// The flag of versions before the update choice. Read, never written
    /// by this version except as found: a choice replaces it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
    /// Unix seconds of the last check.
    pub last_check: u64,
    /// The last version announced under "notify".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub announced: Option<String>,
}

impl Settings {
    /// The stored choice: `mode`, else the meaning of an earlier version's
    /// flag (on → install automatically, off → never check), else none.
    pub fn stored_mode(&self) -> Option<Mode> {
        self.mode.or(self
            .auto_update
            .map(|on| if on { Mode::Auto } else { Mode::Off }))
    }

    /// Whether a choice is stored, so the first-start question stays away.
    pub fn chosen(&self) -> bool {
        self.stored_mode().is_some()
    }

    /// Store `mode` as the choice; it replaces an earlier version's flag.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = Some(mode);
        self.auto_update = None;
    }
}

/// The update choice that applies: the one made in this run, else the
/// stored one, else "install automatically".
fn applying_mode(session: Option<Mode>, settings: &Settings) -> Mode {
    session.or(settings.stored_mode()).unwrap_or(Mode::Auto)
}

/// Settings from the file's bytes; a damaged file counts as no file.
fn parse_settings(bytes: &[u8]) -> Settings {
    serde_json::from_slice(bytes).unwrap_or_default()
}

/// Whether a background check is due at `now` (Unix seconds) under
/// `mode`. A clock that went back counts as due, so a wrong date can't
/// stop updates.
fn is_due(mode: Mode, last_check: u64, now: u64) -> bool {
    mode != Mode::Off && (now < last_check || now - last_check >= CHECK_INTERVAL_SECS)
}

/// Why a release build cannot update itself, as the About dialog says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Unsupported {
    /// A `.deb` / `.rpm` install: the package manager updates it.
    Package,
    /// An AppImage not installed with `--install`.
    Appimage,
}

/// The reason on Linux, from whether Pincel runs from an AppImage and
/// whether that is the installed copy.
fn linux_reason(appimage: bool, installed: bool) -> Option<Unsupported> {
    match (appimage, installed) {
        (_, true) => None,
        (true, false) => Some(Unsupported::Appimage),
        (false, false) => Some(Unsupported::Package),
    }
}

/// Why this copy cannot update itself; `None` where it can, and for a
/// debug build, whose reason only developers would read.
fn unsupported_reason() -> Option<Unsupported> {
    if cfg!(debug_assertions) || !cfg!(target_os = "linux") {
        return None;
    }
    linux_reason(
        std::env::var_os("APPIMAGE").is_some(),
        running_installed_appimage(),
    )
}

/// Whether the running AppImage is the copy `--install` placed.
#[cfg(target_os = "linux")]
fn running_installed_appimage() -> bool {
    crate::linux_install::running_installed()
}

/// No AppImage outside Linux.
#[cfg(not(target_os = "linux"))]
fn running_installed_appimage() -> bool {
    false
}

/// Whether this copy may update itself: a release build, and on Linux
/// only the AppImage installed with `--install` (packages belong to the
/// package manager, a downloaded AppImage stays as it is).
pub fn supported() -> bool {
    if cfg!(debug_assertions) {
        return false;
    }
    #[cfg(target_os = "linux")]
    return crate::linux_install::running_installed();
    #[cfg(not(target_os = "linux"))]
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
        .map(|path| load_settings_at(&path))
        .unwrap_or_default()
}

fn load_settings_at(path: &Path) -> Settings {
    fs::read(path)
        .map(|bytes| parse_settings(&bytes))
        .unwrap_or_default()
}

/// Write the whole file or nothing: a temporary file renamed over the
/// old one, so a reader never sees half a file (which would read as the
/// defaults and turn automatic updates back on).
fn save_settings_at(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    let partial = path.with_extension("json.tmp");
    fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    fs::rename(&partial, path).map_err(|e| e.to_string())
}

/// Serializes every read-modify-write of the settings file, so the
/// background check's `last_check` can't undo a menu toggle.
static SETTINGS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Read, change and write the settings. A choice made in this run
/// (`session`) is written along, so a later successful write stores a
/// choice whose own write failed.
fn update_settings_at(
    path: &Path,
    session: Option<Mode>,
    change: impl FnOnce(&mut Settings),
) -> Result<(), String> {
    // The lock guards no data of its own, so a poisoned one is still good.
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut settings = load_settings_at(path);
    if let Some(mode) = session {
        settings.set_mode(mode);
    }
    change(&mut settings);
    save_settings_at(path, &settings)
}

fn try_update_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let session = session_mode(app);
    settings_path(app)
        .ok_or_else(|| "no app config directory".to_string())
        .and_then(|path| update_settings_at(&path, session, change))
}

fn update_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) {
    if let Err(e) = try_update_settings(app, change) {
        eprintln!("pincel: saving updater settings failed: {e}");
    }
}

/// The update choice made in this run, if any.
fn session_mode(app: &AppHandle) -> Option<Mode> {
    *app.state::<UpdaterState>()
        .session
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// The update choice that applies now.
fn current_mode(app: &AppHandle) -> Mode {
    applying_mode(session_mode(app), &load_settings(app))
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
    /// The update choice made in this run; it applies even when storing
    /// it failed (docs/specs/desktop-app.md, Revision).
    session: std::sync::Mutex<Option<Mode>>,
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
    update_settings(app, |settings| settings.last_check = now);
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
        let mode = current_mode(&app);
        // "notify" announces instead of installing; until that lands, its
        // background check does nothing rather than install unasked.
        if mode != Mode::Auto || !is_due(mode, load_settings(&app).last_check, now) {
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

/// What the Help menu, the About dialog and the first-start question
/// draw from (docs/specs/desktop-app.md AC-22, AC-28). The native macOS
/// menu reads `supported()` directly instead, at build time.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterInfo {
    /// This copy may replace itself — see [`supported`]. False hides the
    /// update entries, so a packaged or debug build offers nothing it
    /// cannot deliver.
    pub supported: bool,
    /// The update choice that applies.
    pub mode: Mode,
    /// Whether a choice is stored or was made in this run; false asks
    /// the first-start question.
    pub chosen: bool,
    /// Version already downloaded in this session, if any.
    pub ready: Option<String>,
    /// Why this copy cannot update itself, where the About dialog names it.
    pub unsupported: Option<Unsupported>,
}

/// Read the updater's state.
#[tauri::command]
pub async fn updater_state(app: AppHandle) -> UpdaterInfo {
    // `try_lock`: a running background check holds the lock for the whole
    // download, and a menu must not wait on the network to open.
    let ready = {
        let state = app.state::<UpdaterState>();
        let pending = state.inner.try_lock();
        pending.ok().and_then(|pending| pending.ready.clone())
    };
    let settings = load_settings(&app);
    let session = session_mode(&app);
    UpdaterInfo {
        supported: supported(),
        mode: applying_mode(session, &settings),
        chosen: session.is_some() || settings.chosen(),
        ready,
        unsupported: unsupported_reason(),
    }
}

/// The answer to [`updater_set_mode`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeSaved {
    /// The update choice that now applies.
    pub mode: Mode,
    /// Whether it was stored; false: it applies for this run only.
    pub saved: bool,
}

/// Make `mode` the update choice: it applies at once for this run, and
/// is stored if the settings file can be written (AC-23, AC-28).
#[tauri::command]
pub async fn updater_set_mode(app: AppHandle, mode: Mode) -> ModeSaved {
    *app.state::<UpdaterState>()
        .session
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(mode);
    let saved = match try_update_settings(&app, |_| {}) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("pincel: saving the update choice failed: {e}");
            false
        }
    };
    ModeSaved { mode, saved }
}

/// "Check for Updates…" from the in-app Help menu. `Ok(None)` is up to
/// date, `Ok(Some(version))` was downloaded and runs from the next start.
///
/// Unlike [`check_now`] — the macOS menu's entry, which reports through a
/// native dialog — this reports back to the renderer, and needs no thread
/// of its own because the command is already async.
#[tauri::command]
pub async fn updater_check(app: AppHandle) -> Result<Option<String>, String> {
    if !supported() {
        return Ok(None);
    }
    let result = fetch(&app).await;
    // A failed check still counts as one, as in the background path.
    record_check(&app, now_secs());
    match result? {
        Outcome::UpToDate => Ok(None),
        Outcome::Ready(version) => Ok(Some(version)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The renderer reads these three keys by name (ui/src/lib/help/).
    // A rename here is a silent break there, so pin the shape.
    #[test]
    fn updater_info_serializes_in_camel_case() {
        let json = serde_json::to_value(UpdaterInfo {
            supported: true,
            mode: Mode::Notify,
            chosen: true,
            ready: Some("1.2.3".to_string()),
            unsupported: None,
        })
        .expect("UpdaterInfo serializes");
        assert_eq!(json["supported"], serde_json::json!(true));
        assert_eq!(json["mode"], serde_json::json!("notify"));
        assert_eq!(json["chosen"], serde_json::json!(true));
        assert_eq!(json["ready"], serde_json::json!("1.2.3"));
        assert!(json["unsupported"].is_null());
    }

    #[test]
    fn updater_info_reports_no_pending_version_as_null() {
        let json = serde_json::to_value(UpdaterInfo {
            supported: false,
            mode: Mode::Auto,
            chosen: false,
            ready: None,
            unsupported: Some(Unsupported::Package),
        })
        .expect("UpdaterInfo serializes");
        assert!(json["ready"].is_null());
        assert_eq!(json["unsupported"], serde_json::json!("package"));
    }

    #[test]
    fn mode_saved_serializes_in_camel_case() {
        let json = serde_json::to_value(ModeSaved {
            mode: Mode::Off,
            saved: false,
        })
        .unwrap();
        assert_eq!(json["mode"], serde_json::json!("off"));
        assert_eq!(json["saved"], serde_json::json!(false));
    }

    // AC-12: nothing chosen reads as "install automatically".
    #[test]
    fn parse_settings_missing_fields_use_defaults() {
        let s = parse_settings(br#"{"last_check": 42}"#);
        assert_eq!(s.last_check, 42);
        assert_eq!(s.stored_mode(), None);
        assert_eq!(applying_mode(None, &s), Mode::Auto);
    }

    #[test]
    fn parse_settings_damaged_file_is_default() {
        assert_eq!(parse_settings(b"{not json"), Settings::default());
        assert_eq!(applying_mode(None, &Settings::default()), Mode::Auto);
    }

    #[test]
    fn parse_settings_round_trip() {
        let mut s = Settings {
            last_check: 1_700_000_000,
            announced: Some("1.2.3".to_string()),
            ..Settings::default()
        };
        s.set_mode(Mode::Notify);
        let bytes = serde_json::to_vec(&s).unwrap();
        assert_eq!(parse_settings(&bytes), s);
    }

    // AC-12: a file from a version before the update choice keeps its meaning.
    #[test]
    fn parse_settings_legacy_auto_on_reads_as_install_automatically() {
        let s = parse_settings(br#"{"auto_update": true, "last_check": 7}"#);
        assert_eq!(s.stored_mode(), Some(Mode::Auto));
    }

    #[test]
    fn parse_settings_legacy_auto_off_reads_as_never_check() {
        let s = parse_settings(br#"{"auto_update": false, "last_check": 7}"#);
        assert_eq!(s.stored_mode(), Some(Mode::Off));
    }

    #[test]
    fn parse_settings_mode_names_are_lowercase() {
        for (name, mode) in [
            ("auto", Mode::Auto),
            ("notify", Mode::Notify),
            ("off", Mode::Off),
        ] {
            let s = parse_settings(format!(r#"{{"mode": "{name}"}}"#).as_bytes());
            assert_eq!(s.stored_mode(), Some(mode));
        }
    }

    // The new version never writes `auto_update`: a choice replaces it.
    #[test]
    fn set_mode_drops_the_legacy_field() {
        let mut s = parse_settings(br#"{"auto_update": false}"#);
        s.set_mode(Mode::Notify);
        let json = serde_json::to_value(&s).unwrap();
        assert!(json.get("auto_update").is_none());
        assert_eq!(json["mode"], serde_json::json!("notify"));
    }

    #[test]
    fn default_settings_write_no_choice() {
        let json = serde_json::to_value(Settings::default()).unwrap();
        assert!(json.get("mode").is_none());
        assert!(json.get("auto_update").is_none());
    }

    // AC-22, AC-25: the question is for copies with no stored choice only.
    #[test]
    fn chosen_false_without_mode_or_legacy_field() {
        assert!(!parse_settings(br#"{"last_check": 9}"#).chosen());
        assert!(!Settings::default().chosen());
    }

    #[test]
    fn chosen_true_for_mode_or_legacy_field() {
        assert!(parse_settings(br#"{"mode": "off"}"#).chosen());
        assert!(parse_settings(br#"{"auto_update": true}"#).chosen());
    }

    // The choice made in this run wins over the stored one (Revision).
    #[test]
    fn applying_mode_prefers_session_then_stored_then_auto() {
        let stored = parse_settings(br#"{"mode": "off"}"#);
        assert_eq!(applying_mode(Some(Mode::Notify), &stored), Mode::Notify);
        assert_eq!(applying_mode(None, &stored), Mode::Off);
        assert_eq!(applying_mode(None, &Settings::default()), Mode::Auto);
    }

    // AC-13: the background check's time and a change of the choice,
    // concurrently, both land.
    #[test]
    fn update_settings_keeps_concurrent_mode_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SETTINGS_FILE);
        let chooser = {
            let path = path.clone();
            std::thread::spawn(move || {
                for i in 0..101 {
                    let mode = if i % 2 == 0 { Mode::Off } else { Mode::Notify };
                    update_settings_at(&path, None, |s| s.set_mode(mode)).unwrap();
                }
            })
        };
        for now in 1..=100 {
            update_settings_at(&path, None, |s| {
                s.last_check = now;
                s.announced = Some(format!("1.0.{now}"));
            })
            .unwrap();
            // Never a half-written file.
            serde_json::from_slice::<Settings>(&fs::read(&path).unwrap()).unwrap();
        }
        chooser.join().unwrap();
        let settings = load_settings_at(&path);
        // The last of 101 choices (i = 100) was "off": none was lost.
        assert_eq!(settings.stored_mode(), Some(Mode::Off));
        assert_eq!(settings.last_check, 100);
        assert_eq!(settings.announced.as_deref(), Some("1.0.100"));
    }

    // A later successful write stores a choice whose own write failed.
    #[test]
    fn update_settings_stores_the_session_choice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(SETTINGS_FILE);
        update_settings_at(&path, Some(Mode::Notify), |s| s.last_check = 5).unwrap();
        let settings = load_settings_at(&path);
        assert_eq!(settings.stored_mode(), Some(Mode::Notify));
        assert_eq!(settings.last_check, 5);
    }

    // AC-11
    #[test]
    fn is_due_first_start() {
        assert!(is_due(Mode::Auto, 0, 1_700_000_000));
        assert!(is_due(Mode::Notify, 0, 1_700_000_000));
    }

    #[test]
    fn is_due_once_a_day() {
        for mode in [Mode::Auto, Mode::Notify] {
            assert!(!is_due(
                mode,
                1_000_000,
                1_000_000 + CHECK_INTERVAL_SECS - 1
            ));
            assert!(is_due(mode, 1_000_000, 1_000_000 + CHECK_INTERVAL_SECS));
        }
    }

    #[test]
    fn is_due_clock_went_back() {
        assert!(is_due(Mode::Auto, 2_000_000, 1_000_000));
    }

    #[test]
    fn is_due_never_when_off() {
        assert!(!is_due(Mode::Off, 0, 1_700_000_000));
    }

    // AC-28: why a copy cannot update itself.
    #[test]
    fn linux_reason_names_package_and_uninstalled_appimage() {
        assert_eq!(linux_reason(false, false), Some(Unsupported::Package));
        assert_eq!(linux_reason(true, false), Some(Unsupported::Appimage));
        assert_eq!(linux_reason(true, true), None);
    }

    #[test]
    fn unsupported_serializes_in_lowercase() {
        assert_eq!(
            serde_json::to_value(Unsupported::Appimage).unwrap(),
            serde_json::json!("appimage")
        );
    }
}
