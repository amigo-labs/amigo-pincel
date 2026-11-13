//! The install steps of the managed ComfyUI (docs/specs/comfyui-install.md,
//! Technical notes "Steps"): uv, Python, ComfyUI, its packages, the model
//! paths, the models, and `.version` last. Each finished step is recorded
//! in `.install-state.json`, so a retry skips it (AC-09).

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::download::{self, InstallError};
use super::extract::{self, Kind};
use super::hardware::Backend;
use super::pins::{Package, Pins};
use super::process::{self, LOG_LINES};
use super::settings;

/// The subfolder of the chosen location (AC-03).
pub const FOLDER: &str = "Pincel ComfyUI";

/// The files and folders of one managed installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub root: PathBuf,
}

impl Layout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn at(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
    fn downloads(&self) -> PathBuf {
        self.at("downloads")
    }
    fn uv_dir(&self) -> PathBuf {
        self.at("uv")
    }
    pub fn venv(&self) -> PathBuf {
        self.at("venv")
    }
    pub fn comfy(&self) -> PathBuf {
        self.at("ComfyUI")
    }
    pub fn models(&self) -> PathBuf {
        self.at("models")
    }
    fn lock_file(&self) -> PathBuf {
        self.at("requirements.lock.txt")
    }
    fn state_file(&self) -> PathBuf {
        self.at(".install-state.json")
    }
    fn version_file(&self) -> PathBuf {
        self.at(".version")
    }
    fn rollback(&self) -> PathBuf {
        self.at("rollback")
    }

    /// The `uv` binary extracted from the pinned archive.
    fn uv(&self, pins: &Pins) -> PathBuf {
        let name = Path::new(&pins.uv_binary)
            .file_name()
            .unwrap_or(OsStr::new("uv"));
        self.uv_dir().join(name)
    }

    /// The venv's interpreter — the one `install::detect` finds first, so
    /// the managed installation starts with the interpreter it set up.
    pub fn python(&self) -> PathBuf {
        if cfg!(windows) {
            self.venv().join("Scripts").join("python.exe")
        } else {
            self.venv().join("bin").join("python3")
        }
    }

    /// Where a model package's file goes.
    pub fn model(&self, package: &Package) -> PathBuf {
        self.models()
            .join(package.kind.folder())
            .join(&package.file)
    }
}

/// The step the wizard names (AC-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    /// "Preparing Python": uv and the Python environment.
    Python,
    /// "Downloading ComfyUI".
    Comfyui,
    /// "Installing PyTorch and dependencies".
    Packages,
    /// "Downloading <package>".
    Model,
}

/// One progress report (AC-06): bytes for Pincel's own downloads, the
/// latest output line for the steps uv runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub step: Step,
    /// The package id of a model download.
    pub package: Option<String>,
    pub done: Option<u64>,
    pub total: Option<u64>,
    pub line: Option<String>,
}

impl Progress {
    fn bytes(step: Step, package: Option<&str>, done: u64, total: u64) -> Self {
        Self {
            step,
            package: package.map(str::to_string),
            done: Some(done),
            total: Some(total),
            line: None,
        }
    }

    fn line(step: Step, line: Option<String>) -> Self {
        Self {
            step,
            package: None,
            done: None,
            total: None,
            line,
        }
    }
}

/// A finished base step, recorded in `.install-state.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Stage {
    Uv,
    Python,
    Comfyui,
    Packages,
    Config,
}

/// What a (possibly unfinished) install has done.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct State {
    pins: String,
    done: Vec<Stage>,
    /// Ids of the verified model packages.
    models: Vec<String>,
}

/// `.version`: the pin set and backend of a complete install, written last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionFile {
    pub pins: String,
    pub backend: Backend,
    pub comfyui: String,
    pub python: String,
    pub uv: String,
}

/// A complete install's `.version`; `None` for an incomplete one.
pub fn read_version(layout: &Layout) -> Option<VersionFile> {
    settings::read_json(&layout.version_file())
}

/// The model packages an install has verified.
pub fn installed_models(layout: &Layout) -> Vec<String> {
    load_state(layout).models
}

fn load_state(layout: &Layout) -> State {
    settings::read_json(&layout.state_file()).unwrap_or_default()
}

fn save_state(layout: &Layout, state: &State) -> Result<(), InstallError> {
    settings::write_json(&layout.state_file(), state)
        .map_err(|message| InstallError::Failed { message })
}

/// One run of the engine: where, which pins, cancel and progress.
pub struct Ctx<'a> {
    pub layout: &'a Layout,
    pub pins: &'a Pins,
    pub cancel: &'a AtomicBool,
    pub progress: &'a (dyn Fn(Progress) + Sync),
}

impl Ctx<'_> {
    fn check_cancel(&self) -> Result<(), InstallError> {
        if self.cancel.load(Ordering::SeqCst) {
            Err(InstallError::Cancelled)
        } else {
            Ok(())
        }
    }
}

fn io_err(e: std::io::Error) -> InstallError {
    InstallError::io(&e)
}

/// Removes a file or folder if it is there.
fn remove(path: &Path) -> Result<(), InstallError> {
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else if path.exists() {
        fs::remove_file(path)
    } else {
        Ok(())
    };
    result.map_err(io_err)
}

/// Installs everything that is not done yet, then the `models`, then
/// writes `.version` (AC-06 – AC-09).
pub fn install(ctx: &Ctx, models: &[Package]) -> Result<(), InstallError> {
    fs::create_dir_all(&ctx.layout.root).map_err(io_err)?;
    base(ctx)?;
    add_models(ctx, models)?;
    write_version(ctx)
}

fn base(ctx: &Ctx) -> Result<(), InstallError> {
    let layout = ctx.layout;
    let mut state = load_state(layout);
    if state.pins != ctx.pins.id {
        // Another pin set: every base step runs again; models are pinned
        // by their own hashes and stay.
        state.pins = ctx.pins.id.clone();
        state.done.clear();
        save_state(layout, &state)?;
    }
    for stage in [
        Stage::Uv,
        Stage::Python,
        Stage::Comfyui,
        Stage::Packages,
        Stage::Config,
    ] {
        if state.done.contains(&stage) {
            continue;
        }
        ctx.check_cancel()?;
        match stage {
            Stage::Uv => step_uv(ctx)?,
            Stage::Python => step_python(ctx)?,
            Stage::Comfyui => step_comfyui(ctx)?,
            Stage::Packages => step_packages(ctx)?,
            Stage::Config => step_config(ctx)?,
        }
        state.done.push(stage);
        save_state(layout, &state)?;
    }
    Ok(())
}

/// Fetches a pinned archive into `downloads/` and unpacks the part below
/// `prefix` into `dest` (through a temporary folder, so a half-extracted
/// archive is never taken for a finished one).
fn fetch_and_extract(
    ctx: &Ctx,
    pin: &download::Pinned,
    name: &str,
    step: Step,
    prefix: &str,
    dest: &Path,
) -> Result<(), InstallError> {
    let kind = Kind::of(&pin.url).ok_or_else(|| InstallError::Failed {
        message: format!("unknown archive type: {}", pin.url),
    })?;
    let file = pin.url.rsplit('/').next().unwrap_or(name);
    let archive = ctx.layout.downloads().join(file);
    download::fetch(pin, &archive, name, ctx.cancel, &|done, total| {
        (ctx.progress)(Progress::bytes(step, None, done, total))
    })?;
    ctx.check_cancel()?;
    let mut tmp = dest.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    remove(&tmp)?;
    extract::extract(&archive, kind, prefix, &tmp)?;
    remove(dest)?;
    fs::rename(&tmp, dest).map_err(io_err)?;
    // The verified archive is no longer needed.
    remove(&archive)
}

fn step_uv(ctx: &Ctx) -> Result<(), InstallError> {
    let prefix = Path::new(&ctx.pins.uv_binary)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    fetch_and_extract(
        ctx,
        &ctx.pins.uv,
        "uv",
        Step::Python,
        &prefix,
        &ctx.layout.uv_dir(),
    )?;
    if ctx.layout.uv(ctx.pins).is_file() {
        Ok(())
    } else {
        Err(InstallError::Failed {
            message: format!("{} missing from the uv archive", ctx.pins.uv_binary),
        })
    }
}

fn step_python(ctx: &Ctx) -> Result<(), InstallError> {
    let venv = ctx.layout.venv();
    remove(&venv)?;
    run_uv(
        ctx,
        Step::Python,
        &[
            "venv".into(),
            "--no-config".into(),
            "--python-preference".into(),
            "only-managed".into(),
            "--python".into(),
            ctx.pins.python.as_str().into(),
            venv.into_os_string(),
        ],
    )
}

fn step_comfyui(ctx: &Ctx) -> Result<(), InstallError> {
    fetch_and_extract(
        ctx,
        &ctx.pins.comfyui,
        "ComfyUI",
        Step::Comfyui,
        &ctx.pins.comfyui_dir,
        &ctx.layout.comfy(),
    )
}

fn step_packages(ctx: &Ctx) -> Result<(), InstallError> {
    let lock = ctx.layout.lock_file();
    fs::write(&lock, &ctx.pins.lock).map_err(io_err)?;
    run_uv(
        ctx,
        Step::Packages,
        &[
            "pip".into(),
            "sync".into(),
            "--no-config".into(),
            "--require-hashes".into(),
            "--python".into(),
            ctx.layout.python().into_os_string(),
            lock.into_os_string(),
        ],
    )
}

/// `extra_model_paths.yaml`: ComfyUI reads the models from
/// `<install>/models`, which an update of ComfyUI leaves alone (AC-12).
fn step_config(ctx: &Ctx) -> Result<(), InstallError> {
    let models = ctx.layout.models();
    for kind in ["checkpoints", "loras"] {
        fs::create_dir_all(models.join(kind)).map_err(io_err)?;
    }
    let base = models.to_string_lossy().replace('\'', "''");
    let yaml = format!(
        "# Written by Pincel: the managed install's models.\npincel:\n  base_path: '{base}'\n  checkpoints: checkpoints\n  loras: loras\n"
    );
    fs::write(ctx.layout.comfy().join("extra_model_paths.yaml"), yaml).map_err(io_err)
}

/// Downloads the `models` that are not verified yet (AC-06, AC-11).
pub fn add_models(ctx: &Ctx, models: &[Package]) -> Result<(), InstallError> {
    let layout = ctx.layout;
    for package in models {
        let mut state = load_state(layout);
        if state.models.contains(&package.id) && layout.model(package).is_file() {
            continue;
        }
        ctx.check_cancel()?;
        let dest = layout.model(package);
        download::fetch(
            &package.pin,
            &dest,
            &package.name,
            ctx.cancel,
            &|done, total| {
                (ctx.progress)(Progress::bytes(Step::Model, Some(&package.id), done, total))
            },
        )?;
        if !state.models.contains(&package.id) {
            state.models.push(package.id.clone());
        }
        save_state(layout, &state)?;
    }
    Ok(())
}

/// Deletes the files of `models` and forgets them (AC-11).
pub fn remove_models(layout: &Layout, models: &[Package]) -> Result<(), InstallError> {
    let mut state = load_state(layout);
    for package in models {
        let file = layout.model(package);
        remove(&file)?;
        remove(&download::part_path(&file))?;
        state.models.retain(|id| id != &package.id);
    }
    save_state(layout, &state)
}

fn write_version(ctx: &Ctx) -> Result<(), InstallError> {
    let pins = ctx.pins;
    settings::write_json(
        &ctx.layout.version_file(),
        &VersionFile {
            pins: pins.id.clone(),
            backend: pins.backend,
            comfyui: pins.comfyui_version.clone(),
            python: pins.python.clone(),
            uv: pins.uv_version.clone(),
        },
    )
    .map_err(|message| InstallError::Failed { message })
}

/// What an update moves aside and restores on failure (AC-12); the
/// models and uv's Python builds stay.
const REPLACED: [&str; 6] = [
    "uv",
    "venv",
    "ComfyUI",
    "requirements.lock.txt",
    ".install-state.json",
    ".version",
];

/// Replaces ComfyUI and its packages with `ctx.pins`, keeping the models
/// (AC-12). `start` starts the updated server and returns its last output
/// lines when it does not answer. On any failure (cancel included) the
/// previous installation is put back.
pub fn update(ctx: &Ctx, start: &dyn Fn() -> Result<(), Vec<String>>) -> Result<(), InstallError> {
    let layout = ctx.layout;
    let rollback = layout.rollback();
    // A rollback folder left by an interrupted update already holds the
    // previous installation.
    if !rollback.is_dir() {
        let models = load_state(layout).models;
        fs::create_dir_all(&rollback).map_err(io_err)?;
        for name in REPLACED {
            let from = layout.at(name);
            if from.exists() {
                fs::rename(&from, rollback.join(name)).map_err(io_err)?;
            }
        }
        save_state(
            layout,
            &State {
                pins: ctx.pins.id.clone(),
                done: Vec::new(),
                models,
            },
        )?;
    }
    let result = base(ctx)
        .and_then(|()| write_version(ctx))
        .and_then(|()| start().map_err(|log| InstallError::UpdateNotStarted { log }));
    match result {
        Ok(()) => remove(&rollback),
        Err(e) => {
            restore(layout)?;
            Err(e)
        }
    }
}

/// Puts the installation from `rollback/` back.
fn restore(layout: &Layout) -> Result<(), InstallError> {
    let rollback = layout.rollback();
    if !rollback.is_dir() {
        return Ok(());
    }
    for name in REPLACED {
        let saved = rollback.join(name);
        if saved.exists() {
            remove(&layout.at(name))?;
            fs::rename(&saved, layout.at(name)).map_err(io_err)?;
        }
    }
    remove(&rollback)
}

/// Runs the pinned `uv` with `args`, its output as progress lines; fails
/// with its last [`LOG_LINES`] lines (AC-08).
fn run_uv(ctx: &Ctx, step: Step, args: &[OsString]) -> Result<(), InstallError> {
    let layout = ctx.layout;
    let mut command = Command::new(layout.uv(ctx.pins));
    command
        .args(args)
        .current_dir(&layout.root)
        // Python builds and the package cache stay inside the install, so
        // Uninstall removes them too.
        .env("UV_PYTHON_INSTALL_DIR", layout.at("python"))
        .env("UV_CACHE_DIR", layout.at("cache"))
        .env("UV_NO_PROGRESS", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(process::CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|e| InstallError::Failed {
        message: format!("uv could not be started: {e}"),
    })?;
    let (tx, rx) = mpsc::channel::<String>();
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|stream| {
        let tx = tx.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut buf = Vec::new();
            while matches!(reader.read_until(b'\n', &mut buf), Ok(n) if n > 0) {
                let line = String::from_utf8_lossy(&buf)
                    .trim_end_matches(['\r', '\n'])
                    .to_string();
                buf.clear();
                if tx.send(line).is_err() {
                    break;
                }
            }
        })
    })
    .collect();
    drop(tx);
    let mut log: Vec<String> = Vec::new();
    let mut keep = |line: String| {
        (ctx.progress)(Progress::line(step, Some(line.clone())));
        if log.len() == LOG_LINES {
            log.remove(0);
        }
        log.push(line);
    };
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => keep(line),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            // Both pipes closed: the process is ending.
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break child.wait().map_err(io_err)?;
            }
        }
        if ctx.cancel.load(Ordering::SeqCst) {
            process::kill_pid(child.id());
            let _ = child.kill();
            let _ = child.wait();
            return Err(InstallError::Cancelled);
        }
        if let Some(status) = child.try_wait().map_err(io_err)? {
            // Collect what the readers still hold (a grandchild keeping a
            // pipe open must not block).
            while let Ok(line) = rx.recv_timeout(Duration::from_millis(200)) {
                keep(line);
            }
            break status;
        }
    };
    drop(readers);
    if status.success() {
        return Ok(());
    }
    if log.iter().any(|l| mentions_full_disk(l)) {
        return Err(InstallError::DiskFull);
    }
    Err(InstallError::SetupFailed { log })
}

/// uv's and pip's wording for a full disk.
fn mentions_full_disk(line: &str) -> bool {
    let line = line.to_ascii_lowercase();
    [
        "no space left on device",
        "not enough space on the disk",
        "os error 112",
        "os error 28)",
    ]
    .iter()
    .any(|needle| line.contains(needle))
}

#[cfg(test)]
pub(crate) mod tests;
