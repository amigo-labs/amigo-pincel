//! Pincel's own downloads for the managed install (docs/specs/comfyui-install.md
//! AC-06 – AC-09): a pinned URL, checked against its pinned SHA-256, resumed
//! with an HTTP range where the server supports it, cancellable.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Why an install step stopped (AC-07, AC-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InstallError {
    /// AC-07: the file did not match its pinned SHA-256 and was deleted.
    Corrupted { file: String },
    /// AC-08: the pinned URL answers 404 / 410 — "update Pincel".
    Gone { file: String },
    /// AC-08: no connection (DNS, connect, a dropped transfer, a timeout).
    Offline,
    /// AC-08: the disk is full.
    DiskFull,
    /// AC-08: Python or the package setup failed; its last output lines.
    SetupFailed { log: Vec<String> },
    /// Another HTTP error of a pinned URL.
    Http { file: String, status: u16 },
    /// The user cancelled.
    Cancelled,
    /// AC-12: the updated server did not start; the previous one is back.
    UpdateNotStarted { log: Vec<String> },
    /// AC-14: a generation is running.
    GenerationRunning,
    /// Another install job is running.
    Busy,
    /// Anything else (a folder that cannot be written, a broken archive).
    Failed { message: String },
}

impl InstallError {
    /// Maps a file-system error: a full disk is its own message (AC-08).
    pub fn io(e: &io::Error) -> Self {
        if e.kind() == io::ErrorKind::StorageFull || is_disk_full_code(e.raw_os_error()) {
            Self::DiskFull
        } else {
            Self::Failed {
                message: e.to_string(),
            }
        }
    }
}

/// ENOSPC (28) on Unix, ERROR_HANDLE_DISK_FULL (39) / ERROR_DISK_FULL (112)
/// on Windows.
fn is_disk_full_code(code: Option<i32>) -> bool {
    match code {
        Some(28) => cfg!(unix),
        Some(39 | 112) => cfg!(windows),
        _ => false,
    }
}

/// A pinned file: where it comes from, its size and SHA-256 (lower-case hex).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

/// How often progress is reported while bytes arrive.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// The download client: no overall timeout (models are gigabytes), but a
/// stalled connection ends; loopback skips the system proxy (tests).
fn client(loopback: bool) -> Result<&'static reqwest::Client, InstallError> {
    static LOCAL: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    static REMOTE: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    let cell = if loopback { &LOCAL } else { &REMOTE };
    cell.get_or_init(|| {
        super::http::install_crypto_provider();
        let builder = reqwest::Client::builder()
            .user_agent("Pincel")
            .connect_timeout(Duration::from_secs(20))
            .read_timeout(Duration::from_secs(60));
        let builder = if loopback {
            builder.no_proxy()
        } else {
            builder
        };
        builder.build().map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(|e| InstallError::Failed { message: e.clone() })
}

fn is_loopback(url: &str) -> bool {
    super::http::parse_url(url).is_some_and(|u| u.is_loopback())
}

/// The partial file next to `dest`.
pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Downloads `pin` to `dest` unless it is already there (a file only ever
/// lands at `dest` after its hash matched, AC-09). A `<dest>.part` from an
/// earlier try continues with a range request; a server that answers the
/// whole file instead restarts it. `progress` gets (bytes, total).
pub fn fetch(
    pin: &Pinned,
    dest: &Path,
    name: &str,
    cancel: &AtomicBool,
    progress: &dyn Fn(u64, u64),
) -> Result<(), InstallError> {
    if dest.is_file() {
        progress(pin.size, pin.size);
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        fs::create_dir_all(dir).map_err(|e| InstallError::io(&e))?;
    }
    let part = part_path(dest);
    let have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if have < pin.size || pin.size == 0 {
        transfer(pin, &part, have, name, cancel, progress)?;
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(InstallError::Cancelled);
    }
    if sha256_file(&part).map_err(|e| InstallError::io(&e))? != pin.sha256.to_ascii_lowercase() {
        let _ = fs::remove_file(&part);
        return Err(InstallError::Corrupted {
            file: name.to_string(),
        });
    }
    fs::rename(&part, dest).map_err(|e| InstallError::io(&e))
}

fn transfer(
    pin: &Pinned,
    part: &Path,
    have: u64,
    name: &str,
    cancel: &AtomicBool,
    progress: &dyn Fn(u64, u64),
) -> Result<(), InstallError> {
    let offline = |_: reqwest::Error| InstallError::Offline;
    let mut request = client(is_loopback(&pin.url))?.get(&pin.url);
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut response = block_on(async move { request.send().await }).map_err(offline)?;
    let status = response.status().as_u16();
    let mut done = match status {
        // The server continues where the part file ends.
        206 if have > 0 => have,
        200 => 0,
        // The part is longer than the file: start over.
        416 => {
            let _ = fs::remove_file(part);
            return transfer(pin, part, 0, name, cancel, progress);
        }
        404 | 410 => {
            return Err(InstallError::Gone {
                file: name.to_string(),
            });
        }
        _ => {
            return Err(InstallError::Http {
                file: name.to_string(),
                status,
            });
        }
    };
    let mut file = if done == 0 {
        File::create(part)
    } else {
        OpenOptions::new().append(true).open(part)
    }
    .map_err(|e| InstallError::io(&e))?;
    let total = pin.size.max(done);
    progress(done, total);
    let mut last = Instant::now();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(InstallError::Cancelled);
        }
        let Some(chunk) = block_on(async { response.chunk().await }).map_err(offline)? else {
            break;
        };
        file.write_all(&chunk).map_err(|e| InstallError::io(&e))?;
        done += chunk.len() as u64;
        if last.elapsed() >= PROGRESS_EVERY {
            progress(done, total.max(done));
            last = Instant::now();
        }
    }
    file.flush().map_err(|e| InstallError::io(&e))?;
    progress(done, total.max(done));
    Ok(())
}

/// Runs `future` on Tauri's runtime; requests must be started inside it
/// (their timeouts create timers).
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tauri::async_runtime::block_on(future)
}

/// SHA-256 of a file, lower-case hex.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

/// Lower-case hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use crate::comfyui::testing::{FileServer, Serve, dead_port};

    fn sha(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    fn body(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 7 % 251) as u8).collect()
    }

    fn pin(server: &FileServer, path: &str, bytes: &[u8]) -> Pinned {
        Pinned {
            url: server.url(path),
            size: bytes.len() as u64,
            sha256: sha(bytes),
        }
    }

    fn quiet(_: u64, _: u64) {}

    #[test]
    fn fetch_verifies_the_hash_and_moves_the_file_into_place() {
        let bytes = body(300_000);
        let server = FileServer::start(vec![("/uv.zip", bytes.clone(), Serve::Ranges)]);
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("downloads").join("uv.zip");
        let seen = Mutex::new(Vec::new());
        let progress = |a, b| seen.lock().unwrap().push((a, b));
        fetch(
            &pin(&server, "/uv.zip", &bytes),
            &dest,
            "uv",
            &AtomicBool::new(false),
            &progress,
        )
        .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), bytes);
        assert!(!part_path(&dest).exists());
        let last = *seen.lock().unwrap().last().unwrap();
        assert_eq!(last, (300_000, 300_000));
        // A verified file is not fetched again (AC-09).
        fetch(
            &pin(&server, "/uv.zip", &bytes),
            &dest,
            "uv",
            &AtomicBool::new(false),
            &quiet,
        )
        .unwrap();
        assert_eq!(server.requests().len(), 1);
    }

    #[test]
    fn fetch_with_a_wrong_hash_deletes_the_file_and_reports_it_corrupted() {
        let bytes = body(1000);
        let server = FileServer::start(vec![("/m.safetensors", bytes.clone(), Serve::Ranges)]);
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("m.safetensors");
        let mut wrong = pin(&server, "/m.safetensors", &bytes);
        wrong.sha256 = sha(b"something else");
        let err = fetch(&wrong, &dest, "SD 1.5", &AtomicBool::new(false), &quiet).unwrap_err();
        assert_eq!(
            err,
            InstallError::Corrupted {
                file: "SD 1.5".into()
            }
        );
        assert!(!dest.exists());
        assert!(!part_path(&dest).exists());
    }

    #[test]
    fn fetch_resumes_a_part_file_with_a_range_request() {
        let bytes = body(50_000);
        let server = FileServer::start(vec![("/f", bytes.clone(), Serve::Ranges)]);
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("f");
        fs::write(part_path(&dest), &bytes[..20_000]).unwrap();
        fetch(
            &pin(&server, "/f", &bytes),
            &dest,
            "f",
            &AtomicBool::new(false),
            &quiet,
        )
        .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), bytes);
        assert_eq!(
            server.requests(),
            vec![("/f".to_string(), Some("bytes=20000-".to_string()))]
        );
    }

    #[test]
    fn fetch_restarts_when_the_server_ignores_the_range() {
        let bytes = body(50_000);
        let server = FileServer::start(vec![("/f", bytes.clone(), Serve::Whole)]);
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("f");
        // A stale part that a resume would corrupt.
        fs::write(part_path(&dest), vec![0u8; 20_000]).unwrap();
        fetch(
            &pin(&server, "/f", &bytes),
            &dest,
            "f",
            &AtomicBool::new(false),
            &quiet,
        )
        .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), bytes);
    }

    #[test]
    fn fetch_after_a_dropped_transfer_is_offline_and_a_retry_continues() {
        let bytes = body(80_000);
        let server = FileServer::start(vec![("/f", bytes.clone(), Serve::DropAfter(30_000))]);
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("f");
        let p = pin(&server, "/f", &bytes);
        let err = fetch(&p, &dest, "f", &AtomicBool::new(false), &quiet).unwrap_err();
        assert_eq!(err, InstallError::Offline);
        let kept = fs::metadata(part_path(&dest)).unwrap().len();
        assert!(kept > 0, "the part file keeps what arrived");
        // The second request asks for the rest (DropAfter only drops once).
        fetch(&p, &dest, "f", &AtomicBool::new(false), &quiet).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), bytes);
        let requests = server.requests();
        assert_eq!(requests[1].1, Some(format!("bytes={kept}-")));
    }

    #[test]
    fn fetch_maps_a_missing_url_and_a_dead_server() {
        let server = FileServer::start(vec![]);
        let tmp = tempfile::tempdir().unwrap();
        let gone = Pinned {
            url: server.url("/gone.zip"),
            size: 10,
            sha256: sha(b"x"),
        };
        let err = fetch(
            &gone,
            &tmp.path().join("g"),
            "ComfyUI",
            &AtomicBool::new(false),
            &quiet,
        )
        .unwrap_err();
        assert_eq!(
            err,
            InstallError::Gone {
                file: "ComfyUI".into()
            }
        );
        let dead = Pinned {
            url: format!("http://127.0.0.1:{}/x", dead_port()),
            ..gone
        };
        let err = fetch(
            &dead,
            &tmp.path().join("d"),
            "x",
            &AtomicBool::new(false),
            &quiet,
        )
        .unwrap_err();
        assert_eq!(err, InstallError::Offline);
    }

    #[test]
    fn fetch_stops_when_cancelled() {
        let bytes = body(10_000);
        let server = FileServer::start(vec![("/f", bytes.clone(), Serve::Ranges)]);
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("f");
        let cancel = AtomicBool::new(true);
        let err = fetch(&pin(&server, "/f", &bytes), &dest, "f", &cancel, &quiet).unwrap_err();
        assert_eq!(err, InstallError::Cancelled);
        assert!(!dest.exists());
    }

    #[test]
    fn install_error_io_maps_a_full_disk() {
        let full = io::Error::from(io::ErrorKind::StorageFull);
        assert_eq!(InstallError::io(&full), InstallError::DiskFull);
        let code = if cfg!(windows) { 112 } else { 28 };
        assert_eq!(
            InstallError::io(&io::Error::from_raw_os_error(code)),
            InstallError::DiskFull
        );
        assert!(matches!(
            InstallError::io(&io::Error::from(io::ErrorKind::PermissionDenied)),
            InstallError::Failed { .. }
        ));
    }
}
