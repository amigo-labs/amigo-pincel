//! The install steps against a local file server and a fake `uv`
//! (docs/specs/comfyui-install.md, Technical notes "Tests").

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::*;
use crate::comfyui::download::{Pinned, hex};
use crate::comfyui::extract::tests::{tar_gz_bytes, zip_bytes};
use crate::comfyui::install;
use crate::comfyui::pins::ModelKind;
use crate::comfyui::testing::{FakeComfy, FileServer, Serve, main_py_text, test_python};

/// A fake `uv`: records its arguments in `<install>/uv-calls.txt`, creates
/// a real (pip-less) venv for `venv`, and behaves as `<install>/fake_uv.json`
/// says for `pip sync` (lines, failures, a hang, a full disk).
const FAKE_UV: &str = r#"import json, os, subprocess, sys, time
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.dirname(here)
cfg_path = os.path.join(root, 'fake_uv.json')
cfg = json.load(open(cfg_path, encoding='utf-8')) if os.path.exists(cfg_path) else {}
args = sys.argv[1:]
with open(os.path.join(root, 'uv-calls.txt'), 'a', encoding='utf-8') as f:
    f.write(json.dumps(args) + '\n')
def bump(name):
    p = os.path.join(root, name + '-count.txt')
    n = int(open(p).read()) + 1 if os.path.exists(p) else 1
    open(p, 'w').write(str(n))
    return n
if args[0] == 'venv':
    print('Using CPython ' + args[args.index('--python') + 1], flush=True)
    if cfg.get('venv_fail'):
        print('error: No download found for request', flush=True)
        sys.exit(1)
    subprocess.check_call([sys.executable, '-m', 'venv', '--without-pip', args[-1]])
    print('Creating virtual environment at: ' + args[-1], flush=True)
elif args[:2] == ['pip', 'sync']:
    n = bump('sync')
    for i in range(cfg.get('lines', 3)):
        print('sync line %d' % i, flush=True)
    if cfg.get('hang'):
        time.sleep(120)
    if n <= cfg.get('sync_fail_times', 0):
        if cfg.get('disk_full'):
            print('error: Failed to write: No space left on device (os error 28)', flush=True)
        sys.exit(2)
    print('Installed 42 packages', flush=True)
"#;

fn sha(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The fake uv archive: (published path, bytes, binary inside it).
fn uv_archive() -> (&'static str, Vec<u8>, &'static str) {
    let python = test_python().to_string_lossy().into_owned();
    if cfg!(windows) {
        let cmd = format!("@echo off\r\n\"{python}\" \"%~dp0fake_uv.py\" %*\r\n");
        let bytes = zip_bytes(&[
            ("uv.cmd", cmd.as_bytes()),
            ("fake_uv.py", FAKE_UV.as_bytes()),
        ]);
        ("/uv-test.zip", bytes, "uv.cmd")
    } else {
        let sh = format!("#!/bin/sh\nexec \"{python}\" \"$(dirname \"$0\")/fake_uv.py\" \"$@\"\n");
        let bytes = tar_gz_bytes(&[
            ("uv-test/uv", sh.as_bytes()),
            ("uv-test/fake_uv.py", FAKE_UV.as_bytes()),
        ]);
        ("/uv-test.tar.gz", bytes, "uv-test/uv")
    }
}

fn comfy_archive(version: &str, fake: &FakeComfy) -> Vec<u8> {
    let main = main_py_text(fake);
    zip_bytes(&[
        (&format!("ComfyUI-{version}/main.py"), &main),
        (
            &format!("ComfyUI-{version}/version.txt"),
            version.as_bytes(),
        ),
        (&format!("ComfyUI-{version}/requirements.txt"), b"torch\n"),
    ])
}

fn model_bytes(seed: u8) -> Vec<u8> {
    (0..40_000u32)
        .map(|i| (i as u8).wrapping_mul(seed))
        .collect()
}

/// A file server with a fake uv, two ComfyUI releases and three models,
/// and an install folder below a location with a space and non-ASCII
/// characters (edge cases).
pub(crate) struct Fixture {
    /// Keeps timing tests from running beside this one's processes.
    _spawning: crate::test_support::Spawning,
    pub(crate) tmp: TempDir,
    server: FileServer,
    pub(crate) layout: Layout,
    uv: (&'static str, Vec<u8>, &'static str),
    comfy: FakeComfy,
    progress: Mutex<Vec<Progress>>,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        Self::with_comfy(FakeComfy::default())
    }

    /// ComfyUI's `main.py` behaves as `comfy` says.
    pub(crate) fn with_comfy(comfy: FakeComfy) -> Self {
        let spawning = crate::test_support::spawning();
        let tmp = tempfile::tempdir().unwrap();
        // A location with a space and non-ASCII characters (edge cases).
        let layout = Layout::new(tmp.path().join("Ablage für Modelle").join(FOLDER));
        let uv = uv_archive();
        let server = FileServer::start(vec![
            (uv.0, uv.1.clone(), Serve::Ranges),
            (
                "/ComfyUI-0.3.1.zip",
                comfy_archive("0.3.1", &comfy),
                Serve::Ranges,
            ),
            (
                "/ComfyUI-0.3.2.zip",
                comfy_archive("0.3.2", &comfy),
                Serve::Ranges,
            ),
            ("/sd15.safetensors", model_bytes(3), Serve::Ranges),
            ("/sdxl.safetensors", model_bytes(5), Serve::Ranges),
            ("/pixel.safetensors", model_bytes(7), Serve::Ranges),
        ]);
        Self {
            _spawning: spawning,
            tmp,
            server,
            layout,
            uv,
            comfy,
            progress: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn pins(&self, version: &str) -> Pins {
        let comfy = comfy_archive(version, &self.comfy);
        Pins {
            id: format!("test-{version}"),
            backend: Backend::Cuda,
            uv_version: "0.0.1".into(),
            uv: Pinned {
                url: self.server.url(self.uv.0),
                size: self.uv.1.len() as u64,
                sha256: sha(&self.uv.1),
            },
            uv_binary: self.uv.2.into(),
            python: "3.12.7".into(),
            comfyui_version: version.into(),
            comfyui: Pinned {
                url: self.server.url(&format!("/ComfyUI-{version}.zip")),
                size: comfy.len() as u64,
                sha256: sha(&comfy),
            },
            comfyui_dir: format!("ComfyUI-{version}"),
            lock: "torch==2.0.0 --hash=sha256:00\n".into(),
            base_size: 1,
        }
    }

    pub(crate) fn package(&self, id: &str, kind: ModelKind, seed: u8) -> Package {
        let bytes = model_bytes(seed);
        Package {
            id: id.into(),
            name: format!("Model {id}"),
            kind,
            file: format!("{id}.safetensors"),
            pin: Pinned {
                url: self.server.url(&format!("/{id}.safetensors")),
                size: bytes.len() as u64,
                sha256: sha(&bytes),
            },
            size: bytes.len() as u64,
            license: "Test".into(),
            license_url: "https://example.com".into(),
            min_vram: 0,
            min_memory: 0,
            requires: None,
        }
    }

    pub(crate) fn sd15(&self) -> Package {
        self.package("sd15", ModelKind::Checkpoint, 3)
    }

    pub(crate) fn pixel(&self) -> Package {
        self.package("pixel", ModelKind::Lora, 7)
    }

    fn run<T>(&self, pins: &Pins, cancel: &AtomicBool, f: impl FnOnce(&Ctx) -> T) -> T {
        let progress = |p: Progress| self.progress.lock().unwrap().push(p);
        f(&Ctx {
            layout: &self.layout,
            pins,
            cancel,
            progress: &progress,
        })
    }

    fn install(&self, pins: &Pins, models: &[Package]) -> Result<(), InstallError> {
        self.run(pins, &AtomicBool::new(false), |ctx| install(ctx, models))
    }

    pub(crate) fn configure(&self, json: &str) {
        fs::create_dir_all(&self.layout.root).unwrap();
        fs::write(self.layout.root.join("fake_uv.json"), json).unwrap();
    }

    /// The fake uv's calls, by their first argument(s).
    pub(crate) fn uv_calls(&self) -> Vec<Vec<String>> {
        fs::read_to_string(self.layout.root.join("uv-calls.txt"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    pub(crate) fn requests_for(&self, path: &str) -> usize {
        self.server
            .requests()
            .iter()
            .filter(|(p, _)| p == path)
            .count()
    }
}

#[test]
fn install_runs_every_step_and_writes_version_last() {
    let fx = Fixture::new();
    let pins = fx.pins("0.3.1");
    fx.install(&pins, &[fx.sd15(), fx.pixel()]).unwrap();
    let root = &fx.layout.root;

    // uv made the venv with the pinned Python and synced the lock file.
    let calls = fx.uv_calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert_eq!(calls[0][0], "venv");
    assert!(calls[0].windows(2).any(|w| w == ["--python", "3.12.7"]));
    assert_eq!(
        calls[0].last().unwrap(),
        &fx.layout.venv().to_string_lossy()
    );
    assert_eq!(calls[1][..2], ["pip", "sync"]);
    assert!(calls[1].contains(&"--require-hashes".to_string()));
    let lock = calls[1].last().unwrap();
    assert_eq!(fs::read_to_string(lock).unwrap(), pins.lock);

    // ComfyUI, the model paths and the models.
    assert_eq!(
        fs::read_to_string(fx.layout.comfy().join("version.txt")).unwrap(),
        "0.3.1"
    );
    let yaml = fs::read_to_string(fx.layout.comfy().join("extra_model_paths.yaml")).unwrap();
    assert!(
        yaml.contains(&format!("base_path: '{}'", fx.layout.models().display())),
        "{yaml}"
    );
    assert_eq!(
        fs::read(fx.layout.model(&fx.sd15())).unwrap(),
        model_bytes(3)
    );
    assert!(
        fx.layout
            .models()
            .join("loras")
            .join("pixel.safetensors")
            .is_file()
    );
    assert_eq!(installed_models(&fx.layout), ["sd15", "pixel"]);

    // `.version` names the pin set; the folder is a startable installation.
    let version = read_version(&fx.layout).unwrap();
    assert_eq!(version.pins, "test-0.3.1");
    assert_eq!(version.comfyui, "0.3.1");
    assert_eq!(version.backend, Backend::Cuda);
    let inst = install::detect(root).unwrap();
    assert_eq!(inst.comfy_dir, fx.layout.comfy());
    assert_eq!(inst.python, fx.layout.python());
    // The verified archives were removed after extraction.
    assert_eq!(fs::read_dir(root.join("downloads")).unwrap().count(), 0);

    // The steps in the wizard's order (AC-06).
    let steps: Vec<Step> = fx.progress.lock().unwrap().iter().map(|p| p.step).collect();
    let mut order = steps.clone();
    order.dedup();
    assert_eq!(
        order,
        [Step::Python, Step::Comfyui, Step::Packages, Step::Model]
    );
    let progress = fx.progress.lock().unwrap();
    assert!(progress.iter().any(|p| p.step == Step::Packages
        && p.line.as_deref() == Some("Installed 42 packages")));
    let last = progress.last().unwrap();
    assert_eq!(last.package.as_deref(), Some("pixel"));
    assert_eq!((last.done, last.total), (Some(40_000), Some(40_000)));
}

#[test]
fn install_with_a_corrupted_model_stops_and_a_retry_skips_finished_steps() {
    let fx = Fixture::new();
    let pins = fx.pins("0.3.1");
    let mut bad = fx.sd15();
    bad.pin.sha256 = sha(b"not the model");
    let err = fx.install(&pins, &[bad]).unwrap_err();
    assert_eq!(
        err,
        InstallError::Corrupted {
            file: "Model sd15".into()
        }
    );
    assert!(!fx.layout.model(&fx.sd15()).exists());
    assert!(
        read_version(&fx.layout).is_none(),
        "incomplete without .version"
    );

    fx.install(&pins, &[fx.sd15()]).unwrap();
    assert_eq!(fx.uv_calls().len(), 2, "venv and sync ran once");
    assert_eq!(fx.requests_for(fx.uv.0), 1);
    assert_eq!(fx.requests_for("/ComfyUI-0.3.1.zip"), 1);
    assert!(read_version(&fx.layout).is_some());
}

#[test]
fn install_when_package_setup_fails_reports_its_last_50_lines() {
    let fx = Fixture::new();
    let pins = fx.pins("0.3.1");
    fx.configure(r#"{"lines": 60, "sync_fail_times": 1}"#);
    let err = fx.install(&pins, &[fx.sd15()]).unwrap_err();
    let InstallError::SetupFailed { log } = err else {
        panic!("{err:?}");
    };
    assert_eq!(log.len(), LOG_LINES);
    assert_eq!(log[0], "sync line 10");
    assert_eq!(log[49], "sync line 59");
    // Retry: Python is not prepared again, the sync runs again and passes.
    fx.install(&pins, &[fx.sd15()]).unwrap();
    let calls = fx.uv_calls();
    assert_eq!(calls.iter().filter(|c| c[0] == "venv").count(), 1);
    assert_eq!(calls.iter().filter(|c| c[0] == "pip").count(), 2);
}

#[test]
fn install_when_python_setup_fails_reports_its_output() {
    let fx = Fixture::new();
    fx.configure(r#"{"venv_fail": true}"#);
    let err = fx.install(&fx.pins("0.3.1"), &[fx.sd15()]).unwrap_err();
    assert_eq!(
        err,
        InstallError::SetupFailed {
            log: vec![
                "Using CPython 3.12.7".into(),
                "error: No download found for request".into()
            ]
        }
    );
}

#[test]
fn install_when_the_disk_fills_up_says_disk_full() {
    let fx = Fixture::new();
    fx.configure(r#"{"sync_fail_times": 1, "disk_full": true}"#);
    let err = fx.install(&fx.pins("0.3.1"), &[fx.sd15()]).unwrap_err();
    assert_eq!(err, InstallError::DiskFull);
}

#[test]
fn install_from_a_pinned_url_that_is_gone_says_so() {
    let fx = Fixture::new();
    let mut pins = fx.pins("0.3.1");
    pins.comfyui.url = fx.server.url("/ComfyUI-9.9.9.zip");
    let err = fx.install(&pins, &[fx.sd15()]).unwrap_err();
    assert_eq!(
        err,
        InstallError::Gone {
            file: "ComfyUI".into()
        }
    );
}

#[test]
fn install_cancelled_during_setup_ends_uv_and_a_retry_resumes() {
    let fx = Fixture::new();
    let pins = fx.pins("0.3.1");
    fx.configure(r#"{"hang": true}"#);
    let cancel = AtomicBool::new(false);
    let started = std::time::Instant::now();
    let err = thread::scope(|s| {
        s.spawn(|| {
            // Cancel once the sync has printed its first line.
            while !fx
                .progress
                .lock()
                .unwrap()
                .iter()
                .any(|p| p.step == Step::Packages)
            {
                thread::sleep(Duration::from_millis(20));
            }
            cancel.store(true, Ordering::SeqCst);
        });
        fx.run(&pins, &cancel, |ctx| install(ctx, &[fx.sd15()]))
    })
    .unwrap_err();
    assert_eq!(err, InstallError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(60), "uv was ended");
    fx.configure("{}");
    fx.install(&pins, &[fx.sd15()]).unwrap();
    let calls = fx.uv_calls();
    assert_eq!(calls.iter().filter(|c| c[0] == "venv").count(), 1);
    assert_eq!(fx.requests_for("/ComfyUI-0.3.1.zip"), 1);
}

#[test]
fn add_models_downloads_only_new_ones_and_remove_models_deletes_only_those() {
    let fx = Fixture::new();
    let pins = fx.pins("0.3.1");
    fx.install(&pins, &[fx.sd15()]).unwrap();
    let sdxl = fx.package("sdxl", ModelKind::Checkpoint, 5);
    fx.run(&pins, &AtomicBool::new(false), |ctx| {
        add_models(ctx, &[fx.sd15(), sdxl.clone()])
    })
    .unwrap();
    assert_eq!(fx.requests_for("/sd15.safetensors"), 1);
    assert_eq!(fx.requests_for("/sdxl.safetensors"), 1);
    assert_eq!(installed_models(&fx.layout), ["sd15", "sdxl"]);

    remove_models(&fx.layout, &[fx.sd15()]).unwrap();
    assert!(!fx.layout.model(&fx.sd15()).exists());
    assert!(fx.layout.model(&sdxl).is_file());
    assert_eq!(installed_models(&fx.layout), ["sdxl"]);
    assert!(read_version(&fx.layout).is_some(), "still complete");
}

#[test]
fn update_replaces_comfyui_and_keeps_the_models() {
    let fx = Fixture::new();
    fx.install(&fx.pins("0.3.1"), &[fx.sd15()]).unwrap();
    let new = fx.pins("0.3.2");
    let started = AtomicBool::new(false);
    fx.run(&new, &AtomicBool::new(false), |ctx| {
        update(ctx, &|| {
            started.store(true, Ordering::SeqCst);
            Ok(())
        })
    })
    .unwrap();
    assert!(started.load(Ordering::SeqCst));
    assert_eq!(
        fs::read_to_string(fx.layout.comfy().join("version.txt")).unwrap(),
        "0.3.2"
    );
    assert_eq!(read_version(&fx.layout).unwrap().pins, "test-0.3.2");
    assert_eq!(
        fs::read(fx.layout.model(&fx.sd15())).unwrap(),
        model_bytes(3)
    );
    assert_eq!(installed_models(&fx.layout), ["sd15"]);
    assert!(!fx.layout.root.join("rollback").exists());
    assert_eq!(fx.requests_for("/sd15.safetensors"), 1, "models kept");
}

#[test]
fn update_that_fails_restores_the_previous_installation() {
    let fx = Fixture::new();
    fx.install(&fx.pins("0.3.1"), &[fx.sd15()]).unwrap();
    fx.configure(r#"{"sync_fail_times": 2}"#);
    let err = fx
        .run(&fx.pins("0.3.2"), &AtomicBool::new(false), |ctx| {
            update(ctx, &|| Ok(()))
        })
        .unwrap_err();
    assert!(matches!(err, InstallError::SetupFailed { .. }), "{err:?}");
    assert_eq!(
        fs::read_to_string(fx.layout.comfy().join("version.txt")).unwrap(),
        "0.3.1"
    );
    assert_eq!(read_version(&fx.layout).unwrap().pins, "test-0.3.1");
    assert!(install::detect(&fx.layout.root).is_ok());
    assert!(fx.layout.model(&fx.sd15()).is_file());
    assert!(!fx.layout.root.join("rollback").exists());
}

#[test]
fn update_whose_server_does_not_start_restores_the_previous_installation() {
    let fx = Fixture::new();
    fx.install(&fx.pins("0.3.1"), &[fx.sd15()]).unwrap();
    let err = fx
        .run(&fx.pins("0.3.2"), &AtomicBool::new(false), |ctx| {
            update(ctx, &|| Err(vec!["Traceback".into()]))
        })
        .unwrap_err();
    assert_eq!(
        err,
        InstallError::UpdateNotStarted {
            log: vec!["Traceback".into()]
        }
    );
    assert_eq!(read_version(&fx.layout).unwrap().pins, "test-0.3.1");
    assert_eq!(
        fs::read_to_string(fx.layout.comfy().join("version.txt")).unwrap(),
        "0.3.1"
    );
    assert_eq!(installed_models(&fx.layout), ["sd15"]);
}

#[test]
fn mentions_full_disk_knows_uv_and_windows_wording() {
    assert!(mentions_full_disk(
        "error: failed to write: No space left on device (os error 28)"
    ));
    assert!(mentions_full_disk(
        "There is not enough space on the disk. (os error 112)"
    ));
    assert!(!mentions_full_disk("Resolved 42 packages"));
}
