//! The managed installation end to end: the engine against the local file
//! server and fake uv of `engine::tests`, the started ComfyUI a fake
//! `main.py` (docs/specs/comfyui-install.md, Technical notes "Tests").

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::*;
use crate::comfyui::api::Timeouts;
use crate::comfyui::engine::tests::Fixture;
use crate::comfyui::hardware::{Device, Os};
use crate::comfyui::manager::{Config, Paths};
use crate::comfyui::pins::ModelKind;
use crate::comfyui::testing::FakeComfy;

struct Env {
    fx: Fixture,
    events: Arc<Mutex<Vec<JobEvent>>>,
    pins: Arc<Mutex<Pins>>,
    manager: Arc<Manager>,
    service: Arc<Service>,
}

fn cuda() -> Hardware {
    Hardware {
        os: Os::Windows,
        device: Some(Device {
            name: "NVIDIA GeForce RTX 4070".into(),
            memory: 12 << 30,
            backend: Backend::Cuda,
        }),
        unsupported: None,
        slow: false,
    }
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

impl Env {
    fn new() -> Self {
        Self::with(Fixture::new())
    }

    fn with(fx: Fixture) -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let pins = Arc::new(Mutex::new(fx.pins("0.3.1")));
        let (manager, service) = Self::app(&fx, &events, &pins);
        Self {
            fx,
            events,
            pins,
            manager,
            service,
        }
    }

    /// One app run on the fixture's config dir.
    fn app(
        fx: &Fixture,
        events: &Arc<Mutex<Vec<JobEvent>>>,
        pins: &Arc<Mutex<Pins>>,
    ) -> (Arc<Manager>, Arc<Service>) {
        let config = fx.tmp.path().join("config");
        let manager = Arc::new(Manager::new(
            Config {
                timeouts: Timeouts {
                    probe: Duration::from_secs(2),
                    info: Duration::from_secs(5),
                },
                start_timeout: Duration::from_secs(30),
                poll: Duration::from_millis(50),
                auto_ports: vec![],
            },
            Paths {
                settings: config.join("comfyui.json"),
                record: config.join("comfyui-run.json"),
            },
            Box::new(|_| {}),
        ));
        let sink = Arc::clone(events);
        let current = Arc::clone(pins);
        let catalog = vec![
            fx.sd15(),
            fx.package("sdxl", ModelKind::Checkpoint, 5),
            Package {
                requires: Some("sdxl".into()),
                ..fx.pixel()
            },
        ];
        let service = Arc::new(Service::with(
            Arc::clone(&manager),
            config,
            Box::new(move |e| sink.lock().unwrap().push(e.clone())),
            Box::new(move |_| Some(current.lock().unwrap().clone())),
            catalog,
            Box::new(cuda),
        ));
        (manager, service)
    }

    fn restart(&self) -> (Arc<Manager>, Arc<Service>) {
        Self::app(&self.fx, &self.events, &self.pins)
    }

    fn location(&self) -> String {
        self.fx
            .layout
            .root
            .parent()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    fn dir(&self) -> String {
        self.fx.layout.root.to_string_lossy().into_owned()
    }

    /// Waits for the next job end and returns it.
    fn finished(&self) -> JobEvent {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            {
                let mut events = self.events.lock().unwrap();
                if let Some(i) = events
                    .iter()
                    .position(|e| !matches!(e, JobEvent::Progress { .. }))
                {
                    let end = events.remove(i);
                    events.drain(..i);
                    return end;
                }
            }
            assert!(Instant::now() < deadline, "the job did not end");
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn installed(&self, packages: &[&str]) {
        self.service.accept(&ids(packages)).unwrap();
        self.service
            .install(&self.location(), &ids(packages))
            .unwrap();
        let end = self.finished();
        assert!(matches!(end, JobEvent::Done { .. }), "{end:?}");
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        self.manager.shutdown();
    }
}

fn connected(status: &Status) -> &crate::comfyui::api::ServerInfo {
    match status {
        Status::Connected { server } => server,
        other => panic!("not connected: {other:?}"),
    }
}

// AC-04, AC-05

#[test]
fn install_refuses_unaccepted_licenses_and_selections_without_a_checkpoint() {
    let env = Env::new();
    let location = env.location();
    assert!(matches!(
        env.service.install(&location, &ids(&["sd15"])),
        Err(InstallError::Failed { message }) if message.contains("license")
    ));
    env.service.accept(&ids(&["pixel"])).unwrap();
    assert!(matches!(
        env.service.install(&location, &ids(&["pixel"])),
        Err(InstallError::Failed { .. })
    ));
    // Nothing was recorded or downloaded.
    assert_eq!(env.service.state(), ManagedState::None);
    assert!(!env.fx.layout.root.exists());
}

#[test]
fn accepted_licenses_are_remembered_across_restarts() {
    let env = Env::new();
    assert!(env.service.info().accepted.is_empty());
    env.service.accept(&ids(&["sd15", "unknown"])).unwrap();
    env.service.accept(&ids(&["pixel", "sd15"])).unwrap();
    let (_, next) = env.restart();
    assert_eq!(next.info().accepted, ["sd15", "pixel"]);
}

// AC-01, AC-03

#[test]
fn info_and_check_location_give_the_wizard_its_numbers() {
    let env = Env::new();
    let info = env.service.info();
    assert_eq!(info.hardware, cuda());
    assert_eq!(info.base_size, Some(1));
    assert_eq!(info.reserve, 5 << 30);
    assert_eq!(info.catalog.len(), 3);
    let location = env.location();
    std::fs::create_dir_all(&location).unwrap();
    let check = env.service.check_location(&location);
    assert_eq!(check.target, env.dir());
    assert!(check.writable);
    assert!(check.free.is_some_and(|f| f > 0));
    assert_eq!(check.existing, Existing::None);
    let missing = env.service.check_location(&format!("{location}-gone"));
    assert!(!missing.writable);
}

// AC-10, edge case "Use this installation"

#[test]
fn a_finished_install_selects_managed_and_starts_the_server() {
    let env = Env::new();
    env.installed(&["sd15"]);
    let settings = env.manager.settings();
    assert_eq!(settings.server, ServerChoice::Managed);
    assert_eq!(settings.managed_dir.as_deref(), Some(env.dir().as_str()));
    let server = connected(&env.manager.status()).clone();
    assert!(server.started);
    assert_eq!(server.checkpoints, ["sd15.safetensors"]);
    assert_eq!(
        env.service.state(),
        ManagedState::Complete {
            dir: env.dir(),
            comfyui: "0.3.1".into(),
            models: ids(&["sd15"]),
            update_available: false,
        }
    );
    // Another app finds the folder complete and can use it as it is.
    env.manager.shutdown();
    let check = env.service.check_location(&env.location());
    assert_eq!(check.existing, Existing::Complete);
    std::fs::remove_file(
        env.fx
            .tmp
            .path()
            .join("config")
            .join("comfyui-managed.json"),
    )
    .unwrap();
    env.manager
        .update_settings(|s| {
            s.server = ServerChoice::Automatic;
            s.managed_dir = None;
        })
        .unwrap();
    let status = env.service.adopt(&env.location()).unwrap();
    connected(&status);
    assert_eq!(env.manager.settings().server, ServerChoice::Managed);
}

#[test]
fn a_finished_install_whose_server_fails_stays_complete_and_reports_stopped() {
    let env = Env::with(Fixture::with_comfy(FakeComfy {
        exit: Some(1),
        ..FakeComfy::default()
    }));
    env.installed(&["sd15"]);
    let end_status = env.manager.status();
    assert!(
        matches!(&end_status, Status::Stopped { log } if log.iter().any(|l| l.contains("fake comfy line"))),
        "{end_status:?}"
    );
    assert!(matches!(env.service.state(), ManagedState::Complete { .. }));
    assert_eq!(env.manager.settings().server, ServerChoice::Managed);
}

// AC-09, edge case "Pincel restarted after an interrupted install"

#[test]
fn an_interrupted_install_is_incomplete_after_a_restart_and_resume_finishes_it() {
    let env = Env::new();
    env.fx.configure(r#"{"sync_fail_times": 1}"#);
    env.service.accept(&ids(&["sd15"])).unwrap();
    env.service
        .install(&env.location(), &ids(&["sd15"]))
        .unwrap();
    let end = env.finished();
    assert!(
        matches!(
            end,
            JobEvent::Failed {
                job: JobKind::Install,
                error: InstallError::SetupFailed { .. }
            }
        ),
        "{end:?}"
    );
    // The server choice stays as it was until the install finishes.
    assert_eq!(env.manager.settings().server, ServerChoice::Automatic);
    let (_, next) = env.restart();
    assert_eq!(
        next.state(),
        ManagedState::Incomplete {
            dir: env.dir(),
            packages: ids(&["sd15"])
        }
    );
    assert_eq!(
        next.check_location(&env.location()).existing,
        Existing::Incomplete
    );
    env.service.resume().unwrap();
    assert!(matches!(env.finished(), JobEvent::Done { .. }));
    assert_eq!(
        env.fx.uv_calls().iter().filter(|c| c[0] == "venv").count(),
        1
    );
}

#[test]
fn delete_of_an_incomplete_install_removes_the_folder() {
    let env = Env::new();
    env.fx.configure(r#"{"venv_fail": true}"#);
    env.service.accept(&ids(&["sd15"])).unwrap();
    env.service
        .install(&env.location(), &ids(&["sd15"]))
        .unwrap();
    assert!(matches!(env.finished(), JobEvent::Failed { .. }));
    env.service.uninstall().unwrap();
    assert!(!env.fx.layout.root.exists());
    assert_eq!(env.service.state(), ManagedState::None);
    assert_eq!(env.manager.settings().server, ServerChoice::Automatic);
}

// AC-11

#[test]
fn package_changes_touch_only_those_models_and_keep_a_checkpoint() {
    let env = Env::new();
    env.installed(&["sd15"]);
    // The last checkpoint cannot go; the LoRA needs SDXL.
    assert!(env.service.change_packages(&[], &ids(&["sd15"])).is_err());
    env.service.accept(&ids(&["pixel"])).unwrap();
    assert!(env.service.change_packages(&ids(&["pixel"]), &[]).is_err());
    // Unaccepted license.
    assert!(env.service.change_packages(&ids(&["sdxl"]), &[]).is_err());

    env.service.accept(&ids(&["sdxl"])).unwrap();
    env.service
        .change_packages(&ids(&["sdxl", "pixel"]), &ids(&["sd15"]))
        .unwrap();
    let end = env.finished();
    let JobEvent::Done { job, status } = end else {
        panic!("{end:?}");
    };
    assert_eq!(job, JobKind::Packages);
    connected(&status);
    assert_eq!(env.fx.requests_for("/sd15.safetensors"), 1);
    assert_eq!(env.fx.requests_for("/sdxl.safetensors"), 1);
    assert!(!env.fx.layout.model(&env.fx.sd15()).exists());
    let ManagedState::Complete { models, .. } = env.service.state() else {
        panic!("not complete");
    };
    assert_eq!(models, ["sdxl", "pixel"]);
    // Removing SDXL alone would strand the LoRA (the UI confirms and
    // removes both).
    assert!(env.service.change_packages(&[], &ids(&["sdxl"])).is_err());
}

// AC-12

#[test]
fn newer_pins_offer_an_update_that_keeps_the_models() {
    let env = Env::new();
    env.installed(&["sd15"]);
    *env.pins.lock().unwrap() = env.fx.pins("0.3.2");
    assert!(matches!(
        env.service.state(),
        ManagedState::Complete {
            update_available: true,
            ..
        }
    ));
    env.service.update().unwrap();
    let end = env.finished();
    assert!(
        matches!(
            end,
            JobEvent::Done {
                job: JobKind::Update,
                ..
            }
        ),
        "{end:?}"
    );
    connected(&env.manager.status());
    assert_eq!(
        env.service.state(),
        ManagedState::Complete {
            dir: env.dir(),
            comfyui: "0.3.2".into(),
            models: ids(&["sd15"]),
            update_available: false,
        }
    );
}

#[test]
fn a_failed_update_restores_and_starts_the_previous_installation() {
    let env = Env::new();
    env.installed(&["sd15"]);
    *env.pins.lock().unwrap() = env.fx.pins("0.3.2");
    env.fx.configure(r#"{"sync_fail_times": 2}"#);
    env.service.update().unwrap();
    let end = env.finished();
    assert!(
        matches!(
            end,
            JobEvent::Failed {
                job: JobKind::Update,
                error: InstallError::SetupFailed { .. }
            }
        ),
        "{end:?}"
    );
    connected(&env.manager.status());
    assert!(matches!(
        env.service.state(),
        ManagedState::Complete { comfyui, update_available: true, .. } if comfyui == "0.3.1"
    ));
}

// AC-13

#[test]
fn uninstall_deletes_the_folder_and_falls_back_to_automatic() {
    let env = Env::new();
    env.installed(&["sd15"]);
    connected(&env.manager.status());
    env.service.uninstall().unwrap();
    assert!(!env.fx.layout.root.exists());
    assert!(
        env.fx.layout.root.parent().unwrap().is_dir(),
        "the picked location stays"
    );
    let settings = env.manager.settings();
    assert_eq!(settings.server, ServerChoice::Automatic);
    assert_eq!(settings.managed_dir, None);
    assert_eq!(env.service.state(), ManagedState::None);
    assert_eq!(env.manager.status(), Status::Idle);
}

// AC-14

#[test]
fn a_running_generation_blocks_package_changes_update_and_uninstall() {
    let env = Env::new();
    env.installed(&["sd15"]);
    env.service.set_generating(true);
    assert!(env.service.info().generating);
    env.service.accept(&ids(&["sdxl"])).unwrap();
    assert_eq!(
        env.service.change_packages(&ids(&["sdxl"]), &[]),
        Err(InstallError::GenerationRunning)
    );
    assert_eq!(env.service.update(), Err(InstallError::GenerationRunning));
    assert_eq!(
        env.service.uninstall(),
        Err(InstallError::GenerationRunning)
    );
    assert!(env.fx.layout.root.is_dir());
    connected(&env.manager.status());
    env.service.set_generating(false);
    env.service.uninstall().unwrap();
}

// AC-15

#[test]
fn a_vanished_managed_folder_is_reported_with_its_path() {
    let env = Env::new();
    env.installed(&["sd15"]);
    env.manager.stop();
    std::fs::remove_dir_all(&env.fx.layout.root).unwrap();
    assert_eq!(
        env.service.state(),
        ManagedState::Missing { dir: env.dir() }
    );
    assert_eq!(
        env.manager.ensure(),
        Status::InstallationNotFound { path: env.dir() }
    );
}
