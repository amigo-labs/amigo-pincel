//! The managed installation as the app sees it (docs/specs/comfyui-install.md):
//! the wizard's checks, accepted licenses, install / resume / cancel, adding
//! and removing packages, update and uninstall, each run as one background
//! job that reports progress and its end as events.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread;

use serde::{Deserialize, Serialize};

use super::download::InstallError;
use super::engine::{self, Ctx, FOLDER, Layout, Progress};
use super::hardware::{self, Backend, Hardware};
use super::manager::{Manager, Status};
use super::pins::{self, Package, Pins, SelectionError};
use super::settings::{self, ServerChoice};

/// Which job runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JobKind {
    Install,
    Packages,
    Update,
}

/// What a job reports (the `comfyui:install` event).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum JobEvent {
    Progress {
        job: JobKind,
        progress: Progress,
    },
    /// The job finished; the server's status after starting it (AC-10).
    Done {
        job: JobKind,
        status: Status,
    },
    /// The job stopped (AC-07, AC-08, cancel); Retry resumes it (AC-09).
    Failed {
        job: JobKind,
        error: InstallError,
    },
}

/// A running job, as the dialog shows it after reopening.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub job: JobKind,
    pub progress: Option<Progress>,
}

/// The managed installation's state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ManagedState {
    None,
    /// The folder is gone (AC-15).
    Missing {
        dir: String,
    },
    /// Started but not finished: "Installation incomplete" (edge case).
    Incomplete {
        dir: String,
        packages: Vec<String>,
    },
    Complete {
        dir: String,
        comfyui: String,
        models: Vec<String>,
        /// Pincel pins another ComfyUI (AC-12).
        update_available: bool,
    },
}

/// What is already in a location's "Pincel ComfyUI" folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Existing {
    None,
    Incomplete,
    /// Offer "Use this installation" (edge case).
    Complete,
}

/// The wizard's location check (AC-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationCheck {
    /// `<location>/Pincel ComfyUI`.
    pub target: String,
    /// Free bytes on its volume; `None` when unknown.
    pub free: Option<u64>,
    pub writable: bool,
    pub existing: Existing,
}

/// Everything the wizard shows before installing (AC-01 – AC-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallInfo {
    pub hardware: Hardware,
    pub catalog: Vec<Package>,
    /// Python, ComfyUI and its packages; `None` on unsupported systems.
    pub base_size: Option<u64>,
    pub reserve: u64,
    /// Package ids whose license the user accepted (AC-05).
    pub accepted: Vec<String>,
    pub managed: ManagedState,
    pub job: Option<JobView>,
    /// A generation runs: package changes, update, uninstall wait (AC-14).
    pub generating: bool,
}

/// The managed folder Pincel knows of and the packages chosen for it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Record {
    dir: String,
    packages: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Licenses {
    accepted: Vec<String>,
}

/// Job listener (the Tauri event in the app).
pub type JobNotify = Box<dyn Fn(&JobEvent) + Send + Sync>;
/// The pins for a backend (tests point them at a local server).
pub type PinsFor = Box<dyn Fn(Backend) -> Option<Pins> + Send + Sync>;

struct Running {
    job: JobKind,
    cancel: Arc<AtomicBool>,
    progress: Option<Progress>,
}

/// The managed installation of one app.
pub struct Service {
    manager: Arc<Manager>,
    /// The app config dir: the record and the accepted licenses.
    config: PathBuf,
    notify: JobNotify,
    pins: PinsFor,
    catalog: Vec<Package>,
    hardware: OnceLock<Hardware>,
    detect: Box<dyn Fn() -> Hardware + Send + Sync>,
    running: Mutex<Option<Running>>,
    generating: AtomicBool,
}

fn failed(message: impl Into<String>) -> InstallError {
    InstallError::Failed {
        message: message.into(),
    }
}

impl Service {
    pub fn new(manager: Arc<Manager>, config: PathBuf, notify: JobNotify) -> Self {
        Self::with(
            manager,
            config,
            notify,
            Box::new(pins::current),
            pins::catalog(),
            Box::new(hardware::detect),
        )
    }

    /// A service with its own pins, catalog and hardware check (tests).
    pub fn with(
        manager: Arc<Manager>,
        config: PathBuf,
        notify: JobNotify,
        pins: PinsFor,
        catalog: Vec<Package>,
        detect: Box<dyn Fn() -> Hardware + Send + Sync>,
    ) -> Self {
        Self {
            manager,
            config,
            notify,
            pins,
            catalog,
            hardware: OnceLock::new(),
            detect,
            running: Mutex::new(None),
            generating: AtomicBool::new(false),
        }
    }

    fn record_path(&self) -> PathBuf {
        self.config.join("comfyui-managed.json")
    }

    fn licenses_path(&self) -> PathBuf {
        self.config.join("comfyui-licenses.json")
    }

    fn record(&self) -> Option<Record> {
        settings::read_json(&self.record_path())
    }

    fn running(&self) -> std::sync::MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The hardware check, run once per app start (AC-01).
    pub fn hardware(&self) -> Hardware {
        self.hardware.get_or_init(|| (self.detect)()).clone()
    }

    fn pins(&self) -> Result<Pins, InstallError> {
        self.hardware()
            .backend()
            .and_then(|backend| (self.pins)(backend))
            .ok_or_else(|| failed("this system is not supported"))
    }

    /// Marks a generation as running or ended; comfyui-generate calls it
    /// (AC-14).
    pub fn set_generating(&self, running: bool) {
        self.generating.store(running, Ordering::SeqCst);
    }

    /// What the wizard needs (AC-01 – AC-05).
    pub fn info(&self) -> InstallInfo {
        let hardware = self.hardware();
        let base_size = hardware
            .backend()
            .and_then(|b| (self.pins)(b))
            .map(|p| p.base_size);
        InstallInfo {
            hardware,
            catalog: self.catalog.clone(),
            base_size,
            reserve: pins::RESERVE,
            accepted: self.accepted(),
            managed: self.state(),
            job: self.running().as_ref().map(|r| JobView {
                job: r.job,
                progress: r.progress.clone(),
            }),
            generating: self.generating.load(Ordering::SeqCst),
        }
    }

    /// The accepted licenses (AC-05).
    pub fn accepted(&self) -> Vec<String> {
        settings::read_json::<Licenses>(&self.licenses_path())
            .unwrap_or_default()
            .accepted
    }

    /// Remembers that the user accepted the licenses of `ids` (AC-05).
    pub fn accept(&self, ids: &[String]) -> Result<Vec<String>, String> {
        let mut licenses: Licenses = settings::read_json(&self.licenses_path()).unwrap_or_default();
        for id in ids {
            if self.catalog.iter().any(|p| &p.id == id) && !licenses.accepted.contains(id) {
                licenses.accepted.push(id.clone());
            }
        }
        settings::write_json(&self.licenses_path(), &licenses)?;
        Ok(licenses.accepted)
    }

    /// The managed installation's state.
    pub fn state(&self) -> ManagedState {
        let Some(record) = self.record() else {
            return ManagedState::None;
        };
        let layout = Layout::new(&record.dir);
        if !layout.root.is_dir() {
            return ManagedState::Missing { dir: record.dir };
        }
        match engine::read_version(&layout) {
            Some(version) => ManagedState::Complete {
                update_available: (self.pins)(version.backend)
                    .is_some_and(|pins| pins.id != version.pins),
                comfyui: version.comfyui,
                models: engine::installed_models(&layout),
                dir: record.dir,
            },
            None => ManagedState::Incomplete {
                dir: record.dir,
                packages: record.packages,
            },
        }
    }

    /// The wizard's check of a picked location (AC-03).
    pub fn check_location(&self, location: &str) -> LocationCheck {
        let location = Path::new(location);
        let target = location.join(FOLDER);
        let layout = Layout::new(&target);
        let existing = if engine::read_version(&layout).is_some() {
            Existing::Complete
        } else if target.is_dir() {
            Existing::Incomplete
        } else {
            Existing::None
        };
        LocationCheck {
            target: target.to_string_lossy().into_owned(),
            free: hardware::free_space(&target),
            writable: hardware::writable(location),
            existing,
        }
    }

    fn select(&self, ids: &[String]) -> Result<Vec<Package>, InstallError> {
        pins::select(&self.catalog, ids).map_err(|e| match e {
            SelectionError::Unknown(id) => failed(format!("unknown package {id}")),
            SelectionError::NoCheckpoint => failed("select at least one checkpoint"),
            SelectionError::Missing { package, requires } => {
                failed(format!("{package} needs {requires}"))
            }
        })
    }

    fn require_accepted(&self, packages: &[Package]) -> Result<(), InstallError> {
        let accepted = self.accepted();
        match packages.iter().find(|p| !accepted.contains(&p.id)) {
            Some(p) => Err(failed(format!("the license of {} is not accepted", p.name))),
            None => Ok(()),
        }
    }

    fn idle(&self) -> Result<(), InstallError> {
        if self.generating.load(Ordering::SeqCst) {
            return Err(InstallError::GenerationRunning);
        }
        if self.running().is_some() {
            return Err(InstallError::Busy);
        }
        Ok(())
    }

    /// Installs into `<location>/Pincel ComfyUI` (AC-05 – AC-10). Returns
    /// once the job runs.
    pub fn install(
        self: &Arc<Self>,
        location: &str,
        packages: &[String],
    ) -> Result<(), InstallError> {
        let chosen = self.select(packages)?;
        self.require_accepted(&chosen)?;
        self.pins()?;
        self.idle()?;
        let target = Path::new(location).join(FOLDER);
        let record = Record {
            dir: target.to_string_lossy().into_owned(),
            packages: chosen.iter().map(|p| p.id.clone()).collect(),
        };
        settings::write_json(&self.record_path(), &record).map_err(failed)?;
        self.spawn(JobKind::Install, move |service, ctx| {
            engine::install(ctx, &chosen)?;
            Ok(service.finish(&record.dir))
        })
    }

    /// Continues the recorded install (Retry, "Resume", AC-09).
    pub fn resume(self: &Arc<Self>) -> Result<(), InstallError> {
        let record = self
            .record()
            .ok_or_else(|| failed("no installation to resume"))?;
        let location = Path::new(&record.dir)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.install(&location, &record.packages)
    }

    /// Uses a complete "Pincel ComfyUI" in `location` (edge case).
    pub fn adopt(&self, location: &str) -> Result<Status, InstallError> {
        self.idle()?;
        let target = Path::new(location).join(FOLDER);
        if engine::read_version(&Layout::new(&target)).is_none() {
            return Err(failed("no complete installation in this folder"));
        }
        let dir = target.to_string_lossy().into_owned();
        let record = Record {
            packages: engine::installed_models(&Layout::new(&target)),
            dir: dir.clone(),
        };
        settings::write_json(&self.record_path(), &record).map_err(failed)?;
        Ok(self.finish(&dir))
    }

    /// AC-10: the server choice becomes Managed and the server starts.
    fn finish(&self, dir: &str) -> Status {
        if let Err(e) = self.manager.update_settings(|s| {
            s.server = ServerChoice::Managed;
            s.managed_dir = Some(dir.to_string());
        }) {
            return Status::Failed { message: e };
        }
        self.manager.ensure()
    }

    /// Cancels the running job.
    pub fn cancel(&self) {
        if let Some(running) = self.running().as_ref() {
            running.cancel.store(true, Ordering::SeqCst);
        }
    }

    fn complete_layout(&self) -> Result<(Record, Layout), InstallError> {
        let record = self
            .record()
            .ok_or_else(|| failed("no managed installation"))?;
        let layout = Layout::new(&record.dir);
        if engine::read_version(&layout).is_none() {
            return Err(failed("the installation is not complete"));
        }
        Ok((record, layout))
    }

    /// Adds and removes model packages (AC-11): the server stops first,
    /// only these files change, and it starts again.
    pub fn change_packages(
        self: &Arc<Self>,
        add: &[String],
        remove: &[String],
    ) -> Result<(), InstallError> {
        self.idle()?;
        let (record, layout) = self.complete_layout()?;
        let mut wanted: Vec<String> = engine::installed_models(&layout)
            .into_iter()
            .filter(|id| !remove.contains(id))
            .collect();
        for id in add {
            if !wanted.contains(id) {
                wanted.push(id.clone());
            }
        }
        // At least one checkpoint stays; the LoRA never outlives SDXL.
        let kept = self.select(&wanted)?;
        let added: Vec<Package> = kept
            .iter()
            .filter(|p| add.contains(&p.id))
            .cloned()
            .collect();
        self.require_accepted(&added)?;
        let removed: Vec<Package> = self
            .catalog
            .iter()
            .filter(|p| remove.contains(&p.id))
            .cloned()
            .collect();
        self.pins()?;
        self.spawn(JobKind::Packages, move |service, ctx| {
            service.manager.stop();
            engine::remove_models(&layout, &removed)?;
            let result = engine::add_models(ctx, &added);
            let record = Record {
                packages: engine::installed_models(&layout),
                ..record
            };
            settings::write_json(&service.record_path(), &record).map_err(failed)?;
            result?;
            Ok(service.manager.ensure())
        })
    }

    /// Replaces ComfyUI and its packages with the current pins (AC-12); on
    /// failure the previous installation is back and starts.
    pub fn update(self: &Arc<Self>) -> Result<(), InstallError> {
        self.idle()?;
        self.complete_layout()?;
        self.pins()?;
        self.spawn(JobKind::Update, move |service, ctx| {
            service.manager.stop();
            let start = || match service.manager.ensure() {
                Status::Connected { .. } => Ok(()),
                Status::Stopped { log } => Err(log),
                other => Err(vec![format!("{other:?}")]),
            };
            match engine::update(ctx, &start) {
                Ok(()) => Ok(service.manager.status()),
                Err(e) => {
                    service.manager.stop();
                    service.manager.ensure();
                    Err(e)
                }
            }
        })
    }

    /// Deletes the managed folder with all models; the server choice falls
    /// back to Automatic (AC-13). Also "Delete" for an incomplete install.
    pub fn uninstall(&self) -> Result<(), InstallError> {
        self.idle()?;
        let record = self
            .record()
            .ok_or_else(|| failed("no managed installation"))?;
        let root = PathBuf::from(&record.dir);
        // Only ever a folder Pincel made (spec Decisions: the subfolder).
        if root.file_name().is_some_and(|n| n == FOLDER) && root.is_dir() {
            self.manager.stop();
            std::fs::remove_dir_all(&root).map_err(|e| InstallError::io(&e))?;
        }
        self.manager
            .update_settings(|s| {
                if s.server == ServerChoice::Managed {
                    s.server = ServerChoice::Automatic;
                }
                s.managed_dir = None;
            })
            .map_err(failed)?;
        let _ = std::fs::remove_file(self.record_path());
        Ok(())
    }

    /// Runs `work` as the one job, reporting progress and its end.
    fn spawn(
        self: &Arc<Self>,
        job: JobKind,
        work: impl FnOnce(&Service, &Ctx) -> Result<Status, InstallError> + Send + 'static,
    ) -> Result<(), InstallError> {
        let pins = self.pins()?;
        let layout = self
            .record()
            .map(|r| Layout::new(r.dir))
            .ok_or_else(|| failed("no managed installation"))?;
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut running = self.running();
            if running.is_some() {
                return Err(InstallError::Busy);
            }
            *running = Some(Running {
                job,
                cancel: Arc::clone(&cancel),
                progress: None,
            });
        }
        let service = Arc::clone(self);
        thread::spawn(move || {
            let progress = |p: Progress| {
                if let Some(running) = service.running().as_mut() {
                    running.progress = Some(p.clone());
                }
                (service.notify)(&JobEvent::Progress { job, progress: p });
            };
            let ctx = Ctx {
                layout: &layout,
                pins: &pins,
                cancel: &cancel,
                progress: &progress,
            };
            let result = work(&service, &ctx);
            *service.running() = None;
            (service.notify)(&match result {
                Ok(status) => JobEvent::Done { job, status },
                Err(error) => JobEvent::Failed { job, error },
            });
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
