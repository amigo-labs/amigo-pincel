//! The connection: which server is used, starting and ending the one
//! Pincel owns, and the status the dialog shows
//! (docs/specs/comfyui-connect.md).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::api::{self, ProbeError, ServerInfo, Timeouts};
use super::http::{self, Url};
use super::install::{self, FolderProblem, Installation};
use super::process::{self, Process};
use super::settings::{self, RunRecord, ServerChoice, Settings};

/// Timeouts and ports; tests shorten them.
#[derive(Debug, Clone)]
pub struct Config {
    pub timeouts: Timeouts,
    /// How long a started server may take to answer (AC-07).
    pub start_timeout: Duration,
    /// Interval of the start and exit checks.
    pub poll: Duration,
    /// Ports Automatic tries, in order (AC-03).
    pub auto_ports: Vec<u16>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            timeouts: Timeouts {
                probe: Duration::from_secs(2),
                info: Duration::from_secs(30),
            },
            start_timeout: Duration::from_secs(120),
            poll: Duration::from_millis(250),
            // ComfyUI's own default first, then ComfyUI Desktop's.
            auto_ports: vec![8188, 8000],
        }
    }
}

/// Where the settings and the run record live.
#[derive(Debug, Clone)]
pub struct Paths {
    pub settings: PathBuf,
    pub record: PathBuf,
}

/// What the dialog shows about the connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Status {
    /// Nothing needed a server yet.
    Idle,
    /// Automatic found no server (AC-03).
    NotFound,
    /// The chosen server has no folder or URL yet.
    NotConfigured,
    /// Pincel is starting an installation (AC-06).
    Starting,
    /// A server answers (AC-04, AC-11, AC-12).
    Connected { server: ServerInfo },
    /// A started server ended or never answered (AC-07).
    Stopped { log: Vec<String> },
    /// The installation folder is gone (AC-13).
    InstallationNotFound { path: String },
    /// The installation folder is no longer a startable ComfyUI.
    InstallationInvalid {
        path: String,
        problem: FolderProblem,
    },
    /// AC-08: the custom URL is not one.
    InvalidUrl,
    /// AC-08: nothing answers.
    Unreachable { url: String },
    /// AC-08: something answers, but not ComfyUI.
    NotComfy { url: String },
    /// AC-08: 401, 403 or 407.
    Refused { status: u16 },
    /// The process could not be started at all.
    Failed { message: String },
}

/// Outcome of picking an existing-installation folder (AC-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FolderCheck {
    Accepted { settings: Settings },
    Refused { problem: FolderProblem },
}

/// The server a request can use, or the reason in English (for the
/// `gen.*` tools of comfyui-generate).
pub fn require_usable(status: &Status) -> Result<ServerInfo, String> {
    match status {
        Status::Connected { server } if !server.missing_nodes.is_empty() => Err(format!(
            "this ComfyUI is too old, update it (missing node types: {})",
            server.missing_nodes.join(", ")
        )),
        Status::Connected { server } if server.checkpoints.is_empty() => {
            Err("no model installed".to_string())
        }
        Status::Connected { server } => Ok(server.clone()),
        Status::Idle | Status::NotConfigured => Err("ComfyUI is not set up".to_string()),
        Status::NotFound => Err("no ComfyUI server found".to_string()),
        Status::Starting => Err("ComfyUI is starting".to_string()),
        Status::Stopped { .. } => Err("ComfyUI stopped".to_string()),
        Status::InstallationNotFound { path } => Err(format!("installation not found at {path}")),
        Status::InstallationInvalid { path, .. } => {
            Err(format!("no startable ComfyUI installation at {path}"))
        }
        Status::InvalidUrl => Err("not a valid URL".to_string()),
        Status::Unreachable { .. } => Err("not reachable".to_string()),
        Status::NotComfy { .. } => Err("not a ComfyUI server".to_string()),
        Status::Refused { status } => Err(format!("the server refused access ({status})")),
        Status::Failed { message } => Err(format!("ComfyUI could not be started: {message}")),
    }
}

/// Status listener (the Tauri event in the app).
pub type Notify = Box<dyn Fn(&Status) + Send + Sync>;

/// A server this app ends on quit.
enum Owned {
    /// Started in this run.
    Child {
        process: Arc<Process>,
        root: PathBuf,
    },
    /// Started in an earlier run that did not end it (AC-10).
    Adopted { pid: u32, port: u16, root: PathBuf },
}

impl Owned {
    fn port(&self) -> u16 {
        match self {
            Owned::Child { process, .. } => process.port,
            Owned::Adopted { port, .. } => *port,
        }
    }

    fn root(&self) -> &Path {
        match self {
            Owned::Child { root, .. } | Owned::Adopted { root, .. } => root,
        }
    }

    fn end(&self) {
        match self {
            Owned::Child { process, .. } => process.kill(),
            Owned::Adopted { pid, .. } => process::kill_pid(*pid),
        }
    }
}

struct Inner {
    status: Status,
    owned: Option<Owned>,
    /// Bumped whenever `owned` changes, so a stale exit watcher stays quiet.
    generation: u64,
}

struct Shared {
    config: Config,
    paths: Paths,
    inner: Mutex<Inner>,
    notify: Notify,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn set_status(&self, status: Status) {
        let changed = {
            let mut inner = self.lock();
            let changed = inner.status != status;
            inner.status = status.clone();
            changed
        };
        if changed {
            (self.notify)(&status);
        }
    }

    fn remove_record(&self) {
        let _ = std::fs::remove_file(&self.paths.record);
    }

    /// The started process of `generation` ended: report it, once.
    fn process_ended(&self, generation: u64) -> bool {
        let process = {
            let mut inner = self.lock();
            if inner.generation != generation {
                return false;
            }
            let Some(Owned::Child { process, .. }) = inner.owned.take() else {
                return false;
            };
            inner.generation += 1;
            process
        };
        self.remove_record();
        self.set_status(Status::Stopped {
            log: process.final_log(),
        });
        true
    }
}

/// The ComfyUI connection of one app.
pub struct Manager {
    shared: Arc<Shared>,
    /// Held while a server is resolved or started, so requests during a
    /// start wait for it instead of starting a second process (AC-06).
    start_lock: Mutex<()>,
}

impl Manager {
    pub fn new(config: Config, paths: Paths, notify: Notify) -> Self {
        Self {
            shared: Arc::new(Shared {
                config,
                paths,
                inner: Mutex::new(Inner {
                    status: Status::Idle,
                    owned: None,
                    generation: 0,
                }),
                notify,
            }),
            start_lock: Mutex::new(()),
        }
    }

    /// The current status.
    pub fn status(&self) -> Status {
        self.shared.lock().status.clone()
    }

    /// The stored settings (defaults when none are stored, AC-02).
    pub fn settings(&self) -> Settings {
        settings::read_json(&self.shared.paths.settings).unwrap_or_default()
    }

    fn store(&self, settings: &Settings) -> Result<Settings, String> {
        settings::write_json(&self.shared.paths.settings, settings)?;
        Ok(settings.clone())
    }

    /// Changes the stored settings (the managed install records its folder).
    pub fn update_settings(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
        let mut settings = self.settings();
        change(&mut settings);
        self.store(&settings)
    }

    /// Ends the server this app started, before its installation changes
    /// (comfyui-install AC-14), and forgets the status.
    pub fn stop(&self) {
        let _guard = self
            .start_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.end_owned();
        self.shared.set_status(Status::Idle);
    }

    /// Stores the server choice.
    pub fn set_server(&self, server: ServerChoice) -> Result<Settings, String> {
        let mut settings = self.settings();
        settings.server = server;
        self.store(&settings)
    }

    /// Accepts `path` as the existing installation if it is one (AC-05).
    pub fn choose_folder(&self, path: &str) -> Result<FolderCheck, String> {
        match install::detect(Path::new(path)) {
            Ok(_) => {
                let mut settings = self.settings();
                settings.server = ServerChoice::Existing;
                settings.existing_dir = Some(path.to_string());
                Ok(FolderCheck::Accepted {
                    settings: self.store(&settings)?,
                })
            }
            Err(problem) => Ok(FolderCheck::Refused { problem }),
        }
    }

    /// Stores `url` as the custom server and connects to it (AC-08).
    pub fn test_url(&self, url: &str) -> Result<Status, String> {
        let mut settings = self.settings();
        settings.server = ServerChoice::Custom;
        settings.custom_url = url.trim().to_string();
        self.store(&settings)?;
        Ok(self.ensure())
    }

    /// A server is needed: resolve the chosen one, starting an
    /// installation if it must (AC-03, AC-06).
    pub fn ensure(&self) -> Status {
        let _guard = self
            .start_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let settings = self.settings();
        let status = match settings.server {
            ServerChoice::Automatic => match settings
                .managed_dir
                .as_deref()
                .map(Path::new)
                .and_then(|dir| install::detect(dir).ok())
            {
                Some(inst) => self.ensure_installation(&inst),
                None => self.discover(),
            },
            ServerChoice::Existing => self.ensure_dir(settings.existing_dir.as_deref()),
            ServerChoice::Managed => self.ensure_dir(settings.managed_dir.as_deref()),
            ServerChoice::Custom => self.probe_custom(&settings.custom_url),
        };
        self.shared.set_status(status.clone());
        status
    }

    /// [`ensure`](Self::ensure), then [`require_usable`].
    pub fn ensure_usable(&self) -> Result<ServerInfo, String> {
        require_usable(&self.ensure())
    }

    fn discover(&self) -> Status {
        for &port in &self.shared.config.auto_ports {
            if let Ok(server) = api::fetch_info(&Url::local(port), self.shared.config.timeouts) {
                return Status::Connected { server };
            }
        }
        Status::NotFound
    }

    fn probe_custom(&self, text: &str) -> Status {
        if text.trim().is_empty() {
            return Status::NotConfigured;
        }
        let Some(url) = http::parse_url(text) else {
            return Status::InvalidUrl;
        };
        match api::fetch_info(&url, self.shared.config.timeouts) {
            Ok(server) => Status::Connected { server },
            Err(ProbeError::Unreachable) => Status::Unreachable { url: url.display() },
            Err(ProbeError::NotComfy) => Status::NotComfy { url: url.display() },
            Err(ProbeError::Refused(status)) => Status::Refused { status },
        }
    }

    fn ensure_dir(&self, dir: Option<&str>) -> Status {
        let Some(dir) = dir else {
            return Status::NotConfigured;
        };
        let path = Path::new(dir);
        if !path.is_dir() {
            return Status::InstallationNotFound {
                path: dir.to_string(),
            };
        }
        match install::detect(path) {
            Ok(inst) => self.ensure_installation(&inst),
            Err(problem) => Status::InstallationInvalid {
                path: dir.to_string(),
                problem,
            },
        }
    }

    fn connected_on(&self, port: u16) -> Option<Status> {
        api::fetch_info(&Url::local(port), self.shared.config.timeouts)
            .ok()
            .map(|mut server| {
                server.started = true;
                Status::Connected { server }
            })
    }

    /// Ends the owned server and forgets it.
    fn end_owned(&self) {
        let owned = {
            let mut inner = self.shared.lock();
            inner.generation += 1;
            inner.owned.take()
        };
        if let Some(owned) = owned {
            owned.end();
            self.shared.remove_record();
        }
    }

    fn ensure_installation(&self, inst: &Installation) -> Status {
        // The server this app owns, if it is this installation's.
        let owned = self
            .shared
            .lock()
            .owned
            .as_ref()
            .map(|o| (o.port(), o.root().to_path_buf()));
        if let Some((port, root)) = owned {
            if root == inst.root
                && let Some(status) = self.connected_on(port)
            {
                return status;
            }
            // Gone, hung, or another installation: never two at once.
            self.end_owned();
        }
        // One started in an earlier run that still answers (AC-10).
        if let Some(record) = settings::read_json::<RunRecord>(&self.shared.paths.record)
            && Path::new(&record.dir) == inst.root
            && let Some(status) = self.connected_on(record.port)
        {
            let mut inner = self.shared.lock();
            inner.generation += 1;
            inner.owned = Some(Owned::Adopted {
                pid: record.pid,
                port: record.port,
                root: inst.root.clone(),
            });
            return status;
        }
        self.start(inst)
    }

    fn start(&self, inst: &Installation) -> Status {
        let config = &self.shared.config;
        let port = match process::free_port() {
            Ok(port) => port,
            Err(e) => {
                return Status::Failed {
                    message: e.to_string(),
                };
            }
        };
        self.shared.set_status(Status::Starting);
        let process = match process::spawn(inst, port) {
            Ok(process) => Arc::new(process),
            Err(e) => {
                return Status::Failed {
                    message: e.to_string(),
                };
            }
        };
        let record = RunRecord {
            port,
            pid: process.pid,
            dir: inst.root.to_string_lossy().into_owned(),
        };
        if let Err(e) = settings::write_json(&self.shared.paths.record, &record) {
            eprintln!("pincel: recording the ComfyUI process failed: {e}");
        }
        let generation = {
            let mut inner = self.shared.lock();
            inner.generation += 1;
            inner.owned = Some(Owned::Child {
                process: Arc::clone(&process),
                root: inst.root.clone(),
            });
            inner.generation
        };
        watch_exit(Arc::clone(&self.shared), Arc::clone(&process), generation);

        // The exit watcher may notice the end first and still be collecting
        // the output when this loop does; the stopped status is built here
        // from the same process, so it never reads "Starting".
        let stopped = |process: &Process| Status::Stopped {
            log: process.final_log(),
        };
        let deadline = Instant::now() + config.start_timeout;
        loop {
            if process.exited() {
                self.shared.process_ended(generation);
                return stopped(&process);
            }
            if api::identify(&Url::local(port), config.timeouts.probe).is_ok()
                && let Some(status) = self.connected_on(port)
            {
                return status;
            }
            if Instant::now() >= deadline {
                process.kill();
                self.shared.process_ended(generation);
                return stopped(&process);
            }
            thread::sleep(config.poll);
        }
    }

    /// App start: take over a server an earlier run started and did not
    /// end, if it still answers (AC-10); forget the record otherwise.
    pub fn adopt_on_start(&self) {
        let _guard = self
            .start_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(record) = settings::read_json::<RunRecord>(&self.shared.paths.record) else {
            return;
        };
        if api::identify(&Url::local(record.port), self.shared.config.timeouts.probe).is_ok() {
            let mut inner = self.shared.lock();
            inner.generation += 1;
            inner.owned = Some(Owned::Adopted {
                pid: record.pid,
                port: record.port,
                root: PathBuf::from(record.dir),
            });
        } else {
            self.shared.remove_record();
        }
    }

    /// App quit: end the server this app started; others keep running (AC-09).
    pub fn shutdown(&self) {
        self.end_owned();
    }
}

/// Reports a started process that ends on its own, while starting or
/// later (AC-07).
fn watch_exit(shared: Arc<Shared>, process: Arc<Process>, generation: u64) {
    thread::spawn(move || {
        loop {
            thread::sleep(shared.config.poll);
            if shared.lock().generation != generation {
                return;
            }
            if process.exited() {
                shared.process_ended(generation);
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests;
