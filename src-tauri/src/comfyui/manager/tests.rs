//! Connection tests against fake HTTP servers and fake installations
//! (docs/specs/comfyui-connect.md, Technical notes: Tests).

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use super::*;
use crate::comfyui::testing::{
    FakeComfy, FakeHttp, Route, dead_port, fake_installation, info_json, starts, stats_json,
};

struct Env {
    tmp: TempDir,
    events: Arc<Mutex<Vec<Status>>>,
    manager: Manager,
}

impl Env {
    fn new(auto_ports: Vec<u16>) -> Self {
        Self::with(auto_ports, Duration::from_secs(30))
    }

    fn with(auto_ports: Vec<u16>, start_timeout: Duration) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let manager = Self::manager_in(&tmp, auto_ports, start_timeout, &events);
        Self {
            tmp,
            events,
            manager,
        }
    }

    fn manager_in(
        tmp: &TempDir,
        auto_ports: Vec<u16>,
        start_timeout: Duration,
        events: &Arc<Mutex<Vec<Status>>>,
    ) -> Manager {
        let sink = Arc::clone(events);
        Manager::new(
            Config {
                timeouts: Timeouts {
                    probe: Duration::from_secs(2),
                    info: Duration::from_secs(5),
                },
                start_timeout,
                poll: Duration::from_millis(50),
                auto_ports,
            },
            Paths {
                settings: tmp.path().join("config").join("comfyui.json"),
                record: tmp.path().join("config").join("comfyui-run.json"),
            },
            Box::new(move |s| sink.lock().unwrap().push(s.clone())),
        )
    }

    /// A second app run on the same config dir.
    fn restart(&self) -> Manager {
        Self::manager_in(&self.tmp, vec![], Duration::from_secs(30), &self.events)
    }

    fn store(&self, settings: Settings) {
        settings::write_json(&self.manager.shared.paths.settings, &settings).unwrap();
    }

    /// A fake installation in `<tmp>/<name>`, chosen as `server`.
    fn installation(&self, name: &str, fake: &FakeComfy, server: ServerChoice) -> PathBuf {
        let root = self.tmp.path().join(name);
        fake_installation(&root, fake);
        let dir = Some(root.to_string_lossy().into_owned());
        self.store(Settings {
            server,
            existing_dir: dir.clone().filter(|_| server == ServerChoice::Existing),
            managed_dir: dir.filter(|_| server != ServerChoice::Existing),
            ..Settings::default()
        });
        root
    }

    fn events(&self) -> Vec<Status> {
        self.events.lock().unwrap().clone()
    }
}

fn server(status: &Status) -> &ServerInfo {
    match status {
        Status::Connected { server } => server,
        other => panic!("not connected: {other:?}"),
    }
}

fn comfy() -> FakeHttp {
    FakeHttp::comfy(
        "",
        &stats_json("0.3.40"),
        &info_json(&["sd15.safetensors"], &[], &[]),
    )
}

fn port_of(url: &str) -> u16 {
    url.rsplit(':').next().unwrap().parse().unwrap()
}

/// Waits until `cond` holds, up to `secs` seconds.
fn wait_for(secs: u64, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    cond()
}

fn answers(port: u16) -> bool {
    api::identify(&Url::local(port), Duration::from_millis(500)).is_ok()
}

// AC-02

#[test]
fn settings_default_to_automatic_and_persist_across_restarts() {
    let env = Env::new(vec![]);
    assert_eq!(env.manager.settings().server, ServerChoice::Automatic);
    env.manager.set_server(ServerChoice::Custom).unwrap();
    env.manager.test_url("http://127.0.0.1:1/comfy").unwrap();
    let folder = env.tmp.path().join("portable");
    std::fs::create_dir_all(folder.join("python_embeded")).unwrap();
    std::fs::write(folder.join("python_embeded").join("python.exe"), b"").unwrap();
    std::fs::create_dir_all(folder.join("ComfyUI")).unwrap();
    std::fs::write(folder.join("ComfyUI").join("main.py"), b"").unwrap();
    env.manager
        .choose_folder(&folder.to_string_lossy())
        .unwrap();
    env.manager.set_server(ServerChoice::Custom).unwrap();

    let next = env.restart();
    let settings = next.settings();
    assert_eq!(settings.server, ServerChoice::Custom);
    assert_eq!(settings.custom_url, "http://127.0.0.1:1/comfy");
    assert_eq!(
        settings.existing_dir,
        Some(folder.to_string_lossy().into_owned())
    );
}

// AC-03

#[test]
fn ensure_automatic_prefers_the_first_port() {
    let (a, b) = (comfy(), comfy());
    let env = Env::new(vec![a.port, b.port]);
    let status = env.manager.ensure();
    assert_eq!(port_of(&server(&status).url), a.port);
    assert!(!server(&status).started);
}

#[test]
fn ensure_automatic_skips_a_server_that_is_not_comfyui() {
    let other = FakeHttp::start(vec![Route::json(
        "/system_stats",
        200,
        "{\"status\":\"ok\"}",
    )]);
    let b = comfy();
    let env = Env::new(vec![other.port, b.port]);
    assert_eq!(port_of(&server(&env.manager.ensure()).url), b.port);
}

#[test]
fn ensure_automatic_without_a_server_is_not_found() {
    let env = Env::new(vec![dead_port(), dead_port()]);
    assert_eq!(env.manager.ensure(), Status::NotFound);
    assert_eq!(env.manager.status(), Status::NotFound);
    assert_eq!(env.events().last(), Some(&Status::NotFound));
}

#[test]
fn ensure_automatic_uses_the_managed_installation_first() {
    let running = comfy();
    let env = Env::new(vec![running.port]);
    let root = env.installation("managed", &FakeComfy::default(), ServerChoice::Automatic);
    let status = env.manager.ensure();
    assert!(server(&status).started);
    assert_ne!(port_of(&server(&status).url), running.port);
    assert_eq!(starts(&root).len(), 1);
    env.manager.shutdown();
}

// AC-04

#[test]
fn ensure_reports_version_gpu_checkpoints_and_loras() {
    let a = FakeHttp::comfy(
        "",
        &stats_json("0.3.41"),
        &info_json(&["sdxl.safetensors"], &["pixel.safetensors"], &[]),
    );
    let env = Env::new(vec![a.port]);
    let status = env.manager.ensure();
    let info = server(&status);
    assert_eq!(info.version, "0.3.41");
    assert_eq!(info.gpu.as_ref().unwrap().vram_total, 12_878_086_144);
    assert_eq!(info.checkpoints, ["sdxl.safetensors"]);
    assert_eq!(info.loras, ["pixel.safetensors"]);
    assert_eq!(env.events().last(), Some(&status));
}

// AC-05

#[test]
fn choose_folder_accepts_an_installation_and_refuses_others() {
    let env = Env::new(vec![]);
    let root = env.tmp.path().join("ok");
    fake_installation(&root, &FakeComfy::default());
    let check = env.manager.choose_folder(&root.to_string_lossy()).unwrap();
    let FolderCheck::Accepted { settings } = check else {
        panic!("refused: {check:?}");
    };
    assert_eq!(settings.server, ServerChoice::Existing);

    let desktop = env.tmp.path().join("desktop");
    std::fs::create_dir_all(desktop.join("resources")).unwrap();
    std::fs::write(desktop.join("resources").join("app.asar"), b"").unwrap();
    assert_eq!(
        env.manager
            .choose_folder(&desktop.to_string_lossy())
            .unwrap(),
        FolderCheck::Refused {
            problem: FolderProblem::Desktop
        }
    );
    let empty = env.tmp.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert_eq!(
        env.manager.choose_folder(&empty.to_string_lossy()).unwrap(),
        FolderCheck::Refused {
            problem: FolderProblem::NoComfyUi
        }
    );
    // A refused folder leaves the stored choice alone.
    assert_eq!(
        env.manager.settings().existing_dir,
        Some(root.to_string_lossy().into_owned())
    );
}

// AC-06

#[test]
fn ensure_starts_the_existing_installation_bound_to_loopback() {
    let env = Env::new(vec![]);
    let root = env.installation("existing", &FakeComfy::default(), ServerChoice::Existing);
    let status = env.manager.ensure();
    let info = server(&status);
    assert!(info.started);
    let port = port_of(&info.url);
    assert_eq!(starts(&root), [format!("--listen 127.0.0.1 --port {port}")]);
    let events = env.events();
    assert_eq!(events.first(), Some(&Status::Starting));
    assert_eq!(events.last(), Some(&status));
    // Needed again: the running server is used, not a second one.
    server(&env.manager.ensure());
    assert_eq!(starts(&root).len(), 1);
    env.manager.shutdown();
}

#[test]
fn requests_during_a_start_wait_for_the_same_start() {
    let env = Arc::new(Env::new(vec![]));
    let fake = FakeComfy {
        delay: Some(1.0),
        ..FakeComfy::default()
    };
    let root = env.installation("slow", &fake, ServerChoice::Existing);
    let first = {
        let env = Arc::clone(&env);
        thread::spawn(move || env.manager.ensure())
    };
    assert!(wait_for(10, || env.manager.status() == Status::Starting));
    let second = env.manager.ensure();
    let first = first.join().unwrap();
    assert_eq!(server(&first).url, server(&second).url);
    assert_eq!(starts(&root).len(), 1);
    env.manager.shutdown();
}

#[test]
fn ensure_starts_again_after_the_server_stopped() {
    let env = Env::new(vec![]);
    let root = env.installation("again", &FakeComfy::default(), ServerChoice::Existing);
    let first = env.manager.ensure();
    process::kill_pid(pid_of(&env));
    assert!(wait_for(10, || matches!(
        env.manager.status(),
        Status::Stopped { .. }
    )));
    let second = env.manager.ensure();
    assert_ne!(server(&first).url, server(&second).url);
    assert_eq!(starts(&root).len(), 2);
    env.manager.shutdown();
}

fn pid_of(env: &Env) -> u32 {
    settings::read_json::<RunRecord>(&env.manager.shared.paths.record)
        .expect("run record")
        .pid
}

// AC-07

#[test]
fn a_start_that_never_answers_stops_with_the_last_50_lines() {
    let env = Env::with(vec![], Duration::from_millis(1500));
    let fake = FakeComfy {
        lines: 60,
        delay: None,
        ..FakeComfy::default()
    };
    env.installation("hangs", &fake, ServerChoice::Existing);
    let Status::Stopped { log } = env.manager.ensure() else {
        panic!("not stopped: {:?}", env.manager.status());
    };
    assert_eq!(log.len(), 50);
    assert_eq!(log.last().map(String::as_str), Some("fake comfy line 59"));
    assert_eq!(log.first().map(String::as_str), Some("fake comfy line 10"));
    // Nothing is left to end, and no record points at a dead process.
    assert!(!env.manager.shared.paths.record.exists());
}

#[test]
fn a_process_that_exits_while_starting_stops_with_its_output() {
    let env = Env::new(vec![]);
    let fake = FakeComfy {
        lines: 3,
        exit: Some(1),
        ..FakeComfy::default()
    };
    env.installation("crashes", &fake, ServerChoice::Existing);
    let started = Instant::now();
    let status = env.manager.ensure();
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_eq!(
        status,
        Status::Stopped {
            log: (0..3).map(|i| format!("fake comfy line {i}")).collect()
        }
    );
}

#[test]
fn a_started_server_that_exits_later_reports_stopped() {
    let env = Env::new(vec![]);
    env.installation("later", &FakeComfy::default(), ServerChoice::Existing);
    server(&env.manager.ensure());
    process::kill_pid(pid_of(&env));
    assert!(wait_for(10, || matches!(
        env.manager.status(),
        Status::Stopped { .. }
    )));
    let Some(Status::Stopped { log }) = env.events().last().cloned() else {
        panic!("no stopped event: {:?}", env.events());
    };
    assert!(log.iter().any(|l| l == "fake comfy line 2"), "{log:?}");
    assert!(require_usable(&env.manager.status()).is_err());
}

// AC-08

#[test]
fn test_url_classifies_every_outcome() {
    let env = Env::new(vec![]);
    assert_eq!(
        env.manager.test_url("not a url").unwrap(),
        Status::InvalidUrl
    );
    let dead = dead_port();
    assert_eq!(
        env.manager
            .test_url(&format!("http://127.0.0.1:{dead}"))
            .unwrap(),
        Status::Unreachable {
            url: format!("http://127.0.0.1:{dead}")
        }
    );
    let proxy = FakeHttp::start(vec![Route::json("/comfy/system_stats", 401, "{}")]);
    assert_eq!(
        env.manager
            .test_url(&format!("http://127.0.0.1:{}/comfy", proxy.port))
            .unwrap(),
        Status::Refused { status: 401 }
    );
    let other = FakeHttp::start(vec![Route::json("/system_stats", 200, "{}")]);
    assert!(matches!(
        env.manager
            .test_url(&format!("http://localhost:{}", other.port))
            .unwrap(),
        Status::NotComfy { .. }
    ));
    let behind_proxy = FakeHttp::comfy(
        "/comfy",
        &stats_json("0.3.40"),
        &info_json(&["a.safetensors"], &[], &[]),
    );
    let status = env
        .manager
        .test_url(&format!("http://127.0.0.1:{}/comfy/", behind_proxy.port))
        .unwrap();
    assert_eq!(
        server(&status).url,
        format!("http://127.0.0.1:{}/comfy", behind_proxy.port)
    );
    assert_eq!(env.manager.settings().server, ServerChoice::Custom);
}

#[test]
fn test_url_over_https_reaches_the_server_and_reports_failures() {
    let env = Env::new(vec![]);
    let dead = dead_port();
    assert_eq!(
        env.manager
            .test_url(&format!("https://127.0.0.1:{dead}/comfy"))
            .unwrap(),
        Status::Unreachable {
            url: format!("https://127.0.0.1:{dead}/comfy")
        }
    );
    // A server without TLS: the handshake fails, nothing is mistaken for ComfyUI.
    let plain = comfy();
    assert_eq!(
        env.manager
            .test_url(&format!("https://localhost:{}", plain.port))
            .unwrap(),
        Status::Unreachable {
            url: format!("https://localhost:{}", plain.port)
        }
    );
}

// AC-09

#[test]
fn shutdown_ends_the_started_server_and_no_other() {
    let foreign = comfy();
    let env = Env::new(vec![foreign.port]);
    assert!(!server(&env.manager.ensure()).started);
    env.manager.shutdown();
    assert!(answers(foreign.port));

    env.installation("own", &FakeComfy::default(), ServerChoice::Existing);
    let port = port_of(&server(&env.manager.ensure()).url);
    assert!(answers(port));
    env.manager.shutdown();
    assert!(wait_for(10, || !answers(port)));
    assert!(!env.manager.shared.paths.record.exists());
}

// AC-10

#[test]
fn a_new_run_adopts_the_server_an_earlier_run_left_running() {
    let env = Env::new(vec![]);
    let root = env.installation("left", &FakeComfy::default(), ServerChoice::Existing);
    let port = port_of(&server(&env.manager.ensure()).url);
    // The app is killed: no shutdown. The next run starts.
    let next = env.restart();
    next.adopt_on_start();
    let status = next.ensure();
    assert_eq!(port_of(&server(&status).url), port);
    assert!(server(&status).started);
    assert_eq!(starts(&root).len(), 1);
    next.shutdown();
    assert!(wait_for(10, || !answers(port)));
}

#[test]
fn a_record_whose_server_is_gone_is_forgotten() {
    let env = Env::new(vec![]);
    let record = RunRecord {
        port: dead_port(),
        pid: u32::MAX,
        dir: "nowhere".to_string(),
    };
    settings::write_json(&env.manager.shared.paths.record, &record).unwrap();
    env.manager.adopt_on_start();
    assert!(!env.manager.shared.paths.record.exists());
}

// AC-11

#[test]
fn a_server_without_a_required_node_type_is_too_old() {
    let old = FakeHttp::comfy(
        "",
        &stats_json("0.1.0"),
        &info_json(&["a.safetensors"], &[], &["SetLatentNoiseMask"]),
    );
    let env = Env::new(vec![old.port]);
    let status = env.manager.ensure();
    assert_eq!(server(&status).missing_nodes, ["SetLatentNoiseMask"]);
    let err = require_usable(&status).unwrap_err();
    assert!(
        err.contains("too old") && err.contains("SetLatentNoiseMask"),
        "{err}"
    );
}

// AC-12

#[test]
fn a_server_without_checkpoints_has_no_model_installed() {
    let empty = FakeHttp::comfy("", &stats_json("0.3.40"), &info_json(&[], &[], &[]));
    let env = Env::new(vec![empty.port]);
    let status = env.manager.ensure();
    assert!(server(&status).checkpoints.is_empty());
    assert_eq!(require_usable(&status).unwrap_err(), "no model installed");
    assert_eq!(
        env.manager.ensure_usable().unwrap_err(),
        "no model installed"
    );
}

// AC-13

#[test]
fn a_missing_installation_folder_is_reported_with_its_path() {
    let env = Env::new(vec![]);
    let gone = env
        .tmp
        .path()
        .join("deleted")
        .to_string_lossy()
        .into_owned();
    env.store(Settings {
        server: ServerChoice::Existing,
        existing_dir: Some(gone.clone()),
        ..Settings::default()
    });
    assert_eq!(
        env.manager.ensure(),
        Status::InstallationNotFound { path: gone }
    );
}

#[test]
fn config_default_tries_8188_then_8000_and_waits_120_s() {
    // The defaults the app runs with (AC-03 port order, AC-07 start
    // timeout, the 2 s probe of the Technical notes); every other test
    // builds its own Config.
    let config = Config::default();
    assert_eq!(config.auto_ports, [8188, 8000]);
    assert_eq!(config.start_timeout, Duration::from_secs(120));
    assert_eq!(config.timeouts.probe, Duration::from_secs(2));
}
