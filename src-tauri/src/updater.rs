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
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Menu id of "Check for Updates…" in the macOS menu; the renderer
/// handles it like the in-app entry (AC-17).
pub const MENU_CHECK: &str = "menu:check-updates";

/// Menu id of "About Pincel" in the macOS menu; the renderer opens the
/// in-app About dialog (AC-17, AC-31).
pub const MENU_ABOUT: &str = "menu:about";

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

/// Updater state shared by the background check, the commands and the
/// exit hook (`app.manage`).
pub struct UpdaterState {
    inner: Mutex<Pending>,
    /// The update choice made in this run; it applies even when storing
    /// it failed (docs/specs/desktop-app.md, Revision).
    session: std::sync::Mutex<Option<Mode>>,
    /// A version a background check found under "notify" that waits for
    /// the notice (AC-26). Outside `inner`, so reading it never waits for
    /// a download.
    announce: std::sync::Mutex<Option<String>>,
    /// A check talks to the server right now.
    checking: AtomicBool,
    /// The renderer released the background check (AC-24).
    released: AtomicBool,
    /// A background check runs, so a second one does not start.
    background: AtomicBool,
    /// When the app started, for the background check's start delay.
    started: Instant,
}

impl Default for UpdaterState {
    fn default() -> Self {
        Self {
            inner: Mutex::default(),
            session: std::sync::Mutex::default(),
            announce: std::sync::Mutex::default(),
            checking: AtomicBool::new(false),
            released: AtomicBool::new(false),
            background: AtomicBool::new(false),
            started: Instant::now(),
        }
    }
}

#[derive(Default)]
struct Pending {
    /// Version downloaded and installed (or held) in this session.
    ready: Option<String>,
    /// Windows: the verified installer, run when the app exits.
    held: Option<(Update, Vec<u8>)>,
    /// The outcome of the last check, for a check that waited for it.
    last: Option<Result<Outcome, String>>,
    /// Restart Now: start Pincel again after installing at exit (AC-32).
    restart: bool,
}

impl Pending {
    /// Mark the downloaded version for a restart; an error when none is
    /// downloaded.
    fn arm_restart(&mut self) -> Result<(), String> {
        if self.ready.is_none() {
            return Err("no update is downloaded".to_string());
        }
        self.restart = true;
        Ok(())
    }
}

/// What a check found, as the renderer reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "version", rename_all = "camelCase")]
pub enum Outcome {
    /// No newer version.
    UpToDate,
    /// A newer version exists and was not downloaded ("notify").
    Available(String),
    /// A newer version is downloaded and verified.
    Ready(String),
}

/// What a check that waited for a running one reports: that one's
/// outcome, unless that one only looked and this one downloads.
fn joined_outcome(
    last: Option<&Result<Outcome, String>>,
    download: bool,
) -> Option<Result<Outcome, String>> {
    match last {
        Some(Ok(Outcome::Available(_))) if download => None,
        other => other.cloned(),
    }
}

/// Event the renderer listens to; it reads [`updater_state`] again.
const CHANGED_EVENT: &str = "updater-changed";

/// Check for one update and, with `download`, download, verify and
/// install (or hold) it. One check runs at a time; a check that finds one
/// running waits for it and reports its outcome (AC-31). A version
/// already downloaded in this run is reported without asking the server.
async fn fetch(app: &AppHandle, download: bool) -> Result<Outcome, String> {
    let state = app.state::<UpdaterState>();
    let joined = state.checking.load(Ordering::SeqCst);
    let mut pending = state.inner.lock().await;
    if let Some(version) = &pending.ready {
        return Ok(Outcome::Ready(version.clone()));
    }
    if joined && let Some(outcome) = joined_outcome(pending.last.as_ref(), download) {
        return outcome;
    }
    state.checking.store(true, Ordering::SeqCst);
    let result = look(app, &mut pending, download).await;
    state.checking.store(false, Ordering::SeqCst);
    if let Ok(Outcome::Ready(version)) = &result {
        // Installed: nothing left to announce (AC-26).
        *state
            .announce
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        update_settings(app, |settings| settings.announced = Some(version.clone()));
    }
    pending.last = Some(result.clone());
    drop(pending);
    let _ = app.emit(CHANGED_EVENT, ());
    result
}

async fn look(app: &AppHandle, pending: &mut Pending, download: bool) -> Result<Outcome, String> {
    // Whether the installer starts Pincel again is decided at exit
    // (Restart Now).
    let updater = app
        .updater_builder()
        .restart_after_install(false)
        .build()
        .map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        return Ok(Outcome::UpToDate);
    };
    let version = update.version.clone();
    if !download {
        return Ok(Outcome::Available(version));
    }
    // `download` verifies the signature; nothing unsigned gets further.
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
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

/// What a background check does.
#[derive(Debug, PartialEq, Eq)]
enum Background {
    Skip,
    /// "notify": look and announce, download nothing.
    Look,
    /// "install automatically".
    Download,
}

fn background_action(mode: Mode, last_check: u64, now: u64) -> Background {
    if !is_due(mode, last_check, now) {
        return Background::Skip;
    }
    match mode {
        Mode::Auto => Background::Download,
        Mode::Notify => Background::Look,
        Mode::Off => Background::Skip,
    }
}

/// Whether `found` is announced: every version once (AC-26).
fn should_announce(found: &str, announced: Option<&str>) -> bool {
    announced != Some(found)
}

/// How long a background check released `elapsed` after start still
/// waits: nothing once the start delay has passed.
fn delay_left(elapsed: Duration) -> Duration {
    START_DELAY.saturating_sub(elapsed)
}

/// Run the background check if it is due under the choice that applies.
/// A failed check counts as a check, so a broken endpoint is retried
/// tomorrow rather than on every start.
fn run_background_check(app: &AppHandle) {
    let state = app.state::<UpdaterState>();
    if state.background.swap(true, Ordering::SeqCst) {
        return;
    }
    let now = now_secs();
    let settings = load_settings(app);
    let mode = applying_mode(session_mode(app), &settings);
    let action = background_action(mode, settings.last_check, now);
    if action != Background::Skip {
        let result = tauri::async_runtime::block_on(fetch(app, action == Background::Download));
        record_check(app, now);
        match result {
            Ok(Outcome::Available(version)) => {
                if should_announce(&version, load_settings(app).announced.as_deref()) {
                    *state
                        .announce
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner) = Some(version);
                    let _ = app.emit(CHANGED_EVENT, ());
                }
            }
            Ok(_) => {}
            Err(e) => eprintln!("pincel: update check failed: {e}"),
        }
    }
    state.background.store(false, Ordering::SeqCst);
}

fn spawn_background_check(app: &AppHandle, delay: Duration) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        run_background_check(&app);
    });
}

/// Release the background check once the first-start question is closed
/// or never shows (AC-24): it runs 10 s after start, at once if that has
/// passed. Later calls (a page reload) change nothing.
#[tauri::command]
pub fn updater_release(app: AppHandle) {
    if !supported() {
        return;
    }
    let state = app.state::<UpdaterState>();
    if state.released.swap(true, Ordering::SeqCst) {
        return;
    }
    spawn_background_check(&app, delay_left(state.started.elapsed()));
}

/// Run an update held for exit (Windows) — after Restart Now the
/// installer starts Pincel again (NSIS `/R`) — or, where the bundle is
/// already replaced, start the new version (AC-33).
pub fn install_on_exit(app: &AppHandle) {
    let state = app.state::<UpdaterState>();
    // A check still running at exit holds the lock and has nothing held
    // yet; don't wait for it.
    let Ok(mut pending) = state.inner.try_lock() else {
        return;
    };
    let restart = pending.restart;
    if let Some((update, bytes)) = pending.held.take() {
        if let Err(e) = update.restart_after_install(restart).install(bytes) {
            eprintln!("pincel: installing the update failed: {e}");
        }
        return;
    }
    if restart && pending.ready.is_some() {
        drop(pending);
        tauri::process::restart(&app.env());
    }
}

/// What the Help menu, the About dialog, the notice and the first-start
/// question draw from (docs/specs/desktop-app.md AC-22, AC-26, AC-28).
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
    /// A version the notice announces, if any.
    pub announce: Option<String>,
    /// A check talks to the server right now.
    pub checking: bool,
    /// Why this copy cannot update itself, where the About dialog names it.
    pub unsupported: Option<Unsupported>,
}

/// Read the updater's state.
#[tauri::command]
pub async fn updater_state(app: AppHandle) -> UpdaterInfo {
    let state = app.state::<UpdaterState>();
    // `try_lock`: a running check holds the lock for the whole download,
    // and a menu must not wait on the network to open. While it runs
    // nothing is downloaded yet, or it would have returned at once.
    let ready = state
        .inner
        .try_lock()
        .ok()
        .and_then(|pending| pending.ready.clone());
    let announce = state
        .announce
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let settings = load_settings(&app);
    let session = session_mode(&app);
    UpdaterInfo {
        supported: supported(),
        mode: applying_mode(session, &settings),
        chosen: session.is_some() || settings.chosen(),
        ready,
        announce,
        checking: state.checking.load(Ordering::SeqCst),
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
/// is stored if the settings file can be written (AC-23, AC-28). Once the
/// background check is released, a check that is due starts at once.
#[tauri::command]
pub async fn updater_set_mode(app: AppHandle, mode: Mode) -> ModeSaved {
    let state = app.state::<UpdaterState>();
    *state.session.lock().unwrap_or_else(PoisonError::into_inner) = Some(mode);
    let saved = match try_update_settings(&app, |_| {}) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("pincel: saving the update choice failed: {e}");
            false
        }
    };
    if supported() && state.released.load(Ordering::SeqCst) {
        spawn_background_check(&app, delay_left(state.started.elapsed()));
    }
    ModeSaved { mode, saved }
}

/// "Check for Updates…" and Install (the notice, the About dialog):
/// check, download, verify and install whatever the update choice
/// (AC-16, AC-26, AC-27).
#[tauri::command]
pub async fn updater_check(app: AppHandle) -> Result<Outcome, String> {
    if !supported() {
        return Ok(Outcome::UpToDate);
    }
    let result = fetch(&app, true).await;
    // A failed check still counts as one, as in the background path.
    record_check(&app, now_secs());
    result
}

/// The check the About dialog starts when it opens (AC-31): under
/// "install automatically" it downloads, under "notify" it only looks,
/// under "never check" it does not run (`None`).
#[tauri::command]
pub async fn updater_check_on_open(app: AppHandle) -> Result<Option<Outcome>, String> {
    let mode = current_mode(&app);
    if !supported() || mode == Mode::Off {
        return Ok(None);
    }
    let result = fetch(&app, mode == Mode::Auto).await;
    record_check(&app, now_secs());
    result.map(Some)
}

/// Later on the notice (AC-26): `version` is not announced again.
#[tauri::command]
pub async fn updater_mark_announced(app: AppHandle, version: String) {
    {
        let state = app.state::<UpdaterState>();
        let mut announce = state
            .announce
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if announce.as_deref() == Some(version.as_str()) {
            *announce = None;
        }
    }
    update_settings(&app, |settings| settings.announced = Some(version));
}

/// Restart Now (AC-32): mark the downloaded version for a restart when
/// Pincel exits. Waits for a running check; an error when nothing is
/// downloaded.
#[tauri::command]
pub async fn updater_arm_restart(app: AppHandle) -> Result<(), String> {
    let state = app.state::<UpdaterState>();
    let mut pending = state.inner.lock().await;
    pending.arm_restart()
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
            announce: Some("1.2.4".to_string()),
            checking: true,
            unsupported: None,
        })
        .expect("UpdaterInfo serializes");
        assert_eq!(json["supported"], serde_json::json!(true));
        assert_eq!(json["mode"], serde_json::json!("notify"));
        assert_eq!(json["chosen"], serde_json::json!(true));
        assert_eq!(json["ready"], serde_json::json!("1.2.3"));
        assert_eq!(json["announce"], serde_json::json!("1.2.4"));
        assert_eq!(json["checking"], serde_json::json!(true));
        assert!(json["unsupported"].is_null());
    }

    #[test]
    fn updater_info_reports_no_pending_version_as_null() {
        let json = serde_json::to_value(UpdaterInfo {
            supported: false,
            mode: Mode::Auto,
            chosen: false,
            ready: None,
            announce: None,
            checking: false,
            unsupported: Some(Unsupported::Package),
        })
        .expect("UpdaterInfo serializes");
        assert!(json["ready"].is_null());
        assert!(json["announce"].is_null());
        assert_eq!(json["unsupported"], serde_json::json!("package"));
    }

    // The renderer switches on `state`; pin the tagged shape.
    #[test]
    fn outcome_serializes_tagged() {
        let json = |o: Outcome| serde_json::to_value(o).expect("Outcome serializes");
        assert_eq!(
            json(Outcome::UpToDate),
            serde_json::json!({ "state": "upToDate" })
        );
        assert_eq!(
            json(Outcome::Available("2.0.0".into())),
            serde_json::json!({ "state": "available", "version": "2.0.0" })
        );
        assert_eq!(
            json(Outcome::Ready("2.0.0".into())),
            serde_json::json!({ "state": "ready", "version": "2.0.0" })
        );
    }

    // AC-11, AC-26, AC-27: what a due background check does per choice.
    #[test]
    fn background_action_follows_the_choice() {
        assert_eq!(
            background_action(Mode::Auto, 0, CHECK_INTERVAL_SECS),
            Background::Download
        );
        assert_eq!(
            background_action(Mode::Notify, 0, CHECK_INTERVAL_SECS),
            Background::Look
        );
        assert_eq!(
            background_action(Mode::Off, 0, CHECK_INTERVAL_SECS),
            Background::Skip
        );
    }

    #[test]
    fn background_action_not_due_skips() {
        let last = 1_000_000;
        assert_eq!(
            background_action(Mode::Auto, last, last + 60),
            Background::Skip
        );
        assert_eq!(
            background_action(Mode::Notify, last, last + 60),
            Background::Skip
        );
    }

    // AC-26: a version is announced once; a newer one is announced again.
    #[test]
    fn should_announce_only_versions_not_announced_before() {
        assert!(should_announce("2.0.0", None));
        assert!(!should_announce("2.0.0", Some("2.0.0")));
        assert!(should_announce("2.0.1", Some("2.0.0")));
    }

    // AC-32: Restart Now needs a downloaded version.
    #[test]
    fn arm_restart_without_download_is_err() {
        let mut pending = Pending::default();
        assert!(pending.arm_restart().is_err());
        assert!(!pending.restart);
    }

    #[test]
    fn arm_restart_with_download_marks_restart() {
        let mut pending = Pending {
            ready: Some("2.0.0".into()),
            ..Pending::default()
        };
        assert!(pending.arm_restart().is_ok());
        assert!(pending.restart);
    }

    // AC-24: a check released late starts at once, an early one waits out
    // the start delay.
    #[test]
    fn delay_left_counts_down_to_zero() {
        assert_eq!(delay_left(Duration::ZERO), START_DELAY);
        assert_eq!(
            delay_left(Duration::from_secs(4)),
            START_DELAY - Duration::from_secs(4)
        );
        assert_eq!(delay_left(START_DELAY * 3), Duration::ZERO);
    }

    // AC-31: a check that waited for a running one reports that one's
    // outcome, unless that one only looked and this one downloads.
    #[test]
    fn joined_outcome_reports_the_running_check() {
        let up: Result<Outcome, String> = Ok(Outcome::UpToDate);
        assert_eq!(joined_outcome(Some(&up), true), Some(up.clone()));
        let failed: Result<Outcome, String> = Err("offline".into());
        assert_eq!(joined_outcome(Some(&failed), false), Some(failed.clone()));
        let found: Result<Outcome, String> = Ok(Outcome::Available("2.0.0".into()));
        assert_eq!(joined_outcome(Some(&found), false), Some(found.clone()));
        assert_eq!(joined_outcome(Some(&found), true), None);
        assert_eq!(joined_outcome(None, true), None);
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
