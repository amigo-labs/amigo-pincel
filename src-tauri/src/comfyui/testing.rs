//! Test doubles: a fake HTTP server, fake ComfyUI answers and fake
//! installations whose `main.py` is a small Python HTTP server.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use super::api::REQUIRED_NODES;

/// One canned answer of a [`FakeHttp`].
pub struct Route {
    path: String,
    response: String,
}

impl Route {
    /// `status` with a JSON `body` for `GET path`.
    pub fn json(path: &str, status: u16, body: &str) -> Self {
        Self {
            path: path.to_string(),
            response: format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        }
    }

    /// A verbatim response for `GET path`.
    pub fn raw(path: &str, response: &str) -> Self {
        Self {
            path: path.to_string(),
            response: response.to_string(),
        }
    }
}

/// An HTTP server on 127.0.0.1 answering fixed routes (404 otherwise),
/// alive for the rest of the test process.
pub struct FakeHttp {
    pub port: u16,
}

impl FakeHttp {
    pub fn start(routes: Vec<Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let path = read_request_path(&stream);
                let response = routes
                    .iter()
                    .find(|r| Some(&r.path) == path.as_ref())
                    .map(|r| r.response.clone())
                    .unwrap_or_else(|| Route::json("", 404, "{}").response);
                let mut stream = stream;
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self { port }
    }

    /// Accepts connections and never answers.
    pub fn silent() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming().flatten() {
                held.push(stream);
            }
        });
        Self { port }
    }

    /// A fake ComfyUI: `/system_stats` and `/object_info` (with `prefix`).
    pub fn comfy(prefix: &str, stats: &str, info: &str) -> Self {
        Self::start(vec![
            Route::json(&format!("{prefix}/system_stats"), 200, stats),
            Route::json(&format!("{prefix}/object_info"), 200, info),
        ])
    }
}

fn read_request_path(mut stream: &TcpStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&raw);
    text.split_whitespace().nth(1).map(str::to_string)
}

/// A TCP server that keeps the first bytes each client sends and closes
/// the connection: it shows what a client speaks (TLS starts with 0x16).
pub struct RecordingTcp {
    pub port: u16,
    pub first: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
}

impl RecordingTcp {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let first = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = std::sync::Arc::clone(&first);
        thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let mut buf = [0u8; 16];
                if let Ok(n) = stream.read(&mut buf) {
                    let mut first = sink.lock().expect("lock");
                    if first.is_empty() {
                        first.extend_from_slice(&buf[..n]);
                    }
                }
            }
        });
        Self { port, first }
    }
}

/// A port nothing listens on (bound once, then released).
pub fn dead_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr").port()
}

/// `/system_stats` of a ComfyUI `version` with one CUDA device.
pub fn stats_json(version: &str) -> String {
    format!(
        r#"{{"system":{{"os":"nt","comfyui_version":"{version}","python_version":"3.12"}},"devices":[{{"name":"cuda:0 NVIDIA GeForce RTX 4070 : cudaMallocAsync","type":"cuda","index":0,"vram_total":12878086144,"vram_free":11000000000}}]}}"#
    )
}

/// `/object_info` with every required node type except `missing`, and
/// the given checkpoints and LoRAs.
pub fn info_json(checkpoints: &[&str], loras: &[&str], missing: &[&str]) -> String {
    let list = |names: &[&str]| {
        names
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join(",")
    };
    let nodes: Vec<String> = REQUIRED_NODES
        .iter()
        .filter(|n| !missing.contains(n))
        .map(|n| match *n {
            "CheckpointLoaderSimple" => format!(
                r#""{n}":{{"input":{{"required":{{"ckpt_name":[[{}],{{}}]}}}}}}"#,
                list(checkpoints)
            ),
            "LoraLoader" => format!(
                r#""{n}":{{"input":{{"required":{{"model":["MODEL"],"clip":["CLIP"],"lora_name":[[{}],{{}}],"strength_model":["FLOAT",{{}}]}}}}}}"#,
                list(loras)
            ),
            _ => format!(r#""{n}":{{"input":{{"required":{{}}}}}}"#),
        })
        .collect();
    format!("{{{}}}", nodes.join(","))
}

/// The Python interpreter the fake installations run, from `PATH`.
fn system_python() -> &'static str {
    static PYTHON: OnceLock<&'static str> = OnceLock::new();
    PYTHON.get_or_init(|| {
        let candidates: &[&'static str] = if cfg!(windows) {
            &["python", "python3", "py"]
        } else {
            &["python3", "python"]
        };
        candidates
            .iter()
            .copied()
            .find(|p| {
                Command::new(p)
                    .arg("--version")
                    .output()
                    .is_ok_and(|o| o.status.success())
            })
            .expect("the ComfyUI process tests need Python 3 on PATH")
    })
}

/// One `--without-pip` virtual environment per test process, whose
/// interpreter the fake installations borrow.
fn shared_venv() -> &'static Path {
    static VENV: OnceLock<PathBuf> = OnceLock::new();
    VENV.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("pincel-comfy-venv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let status = Command::new(system_python())
            .args(["-m", "venv", "--without-pip"])
            .arg(&dir)
            .status()
            .expect("python -m venv");
        assert!(status.success(), "python -m venv failed");
        dir
    })
}

/// Gives `root` a `venv` that runs the shared interpreter: its
/// `pyvenv.cfg` plus the launcher (Windows) or a symlink (Unix).
pub fn add_venv(root: &Path, name: &str) {
    let src = shared_venv();
    let venv = root.join(name);
    std::fs::create_dir_all(&venv).expect("venv dir");
    std::fs::copy(src.join("pyvenv.cfg"), venv.join("pyvenv.cfg")).expect("pyvenv.cfg");
    #[cfg(windows)]
    {
        std::fs::create_dir_all(venv.join("Scripts")).expect("Scripts");
        std::fs::copy(
            src.join("Scripts").join("python.exe"),
            venv.join("Scripts").join("python.exe"),
        )
        .expect("python.exe");
    }
    #[cfg(unix)]
    {
        std::fs::create_dir_all(venv.join("bin")).expect("bin");
        let target = std::fs::canonicalize(src.join("bin").join("python")).expect("python");
        std::os::unix::fs::symlink(target, venv.join("bin").join("python")).expect("symlink");
    }
}

/// How a fake `main.py` behaves.
pub struct FakeComfy {
    /// Lines printed before anything else.
    pub lines: usize,
    /// Exit with this code right after printing.
    pub exit: Option<i32>,
    /// Seconds before the server listens (`None`: never).
    pub delay: Option<f64>,
    pub checkpoints: Vec<String>,
}

impl Default for FakeComfy {
    fn default() -> Self {
        Self {
            lines: 3,
            exit: None,
            delay: Some(0.0),
            checkpoints: vec!["sd15.safetensors".to_string()],
        }
    }
}

/// Writes a fake `main.py` into `dir`. Every start appends its argument
/// list as one line to `dir/starts.txt`.
pub fn write_main_py(dir: &Path, fake: &FakeComfy) {
    let ckpts: Vec<&str> = fake.checkpoints.iter().map(String::as_str).collect();
    let script = format!(
        r#"import http.server, json, os, sys, time
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'starts.txt'), 'a') as f:
    f.write(' '.join(sys.argv[1:]) + '\n')
for i in range({lines}):
    print('fake comfy line %d' % i)
exit_code = {exit}
if exit_code is not None:
    sys.exit(exit_code)
delay = {delay}
if delay is None:
    while True:
        time.sleep(1)
time.sleep(delay)
port = int(sys.argv[sys.argv.index('--port') + 1])
host = sys.argv[sys.argv.index('--listen') + 1]
STATS = {stats}
INFO = {info}
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = {{'/system_stats': STATS, '/object_info': INFO}}.get(self.path)
        self.send_response(200 if body else 404)
        data = (body or '{{}}').encode()
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *args):
        pass
print('To see the GUI go to: http://%s:%d' % (host, port))
http.server.HTTPServer((host, port), H).serve_forever()
"#,
        lines = fake.lines,
        exit = fake.exit.map_or("None".to_string(), |c| c.to_string()),
        delay = fake.delay.map_or("None".to_string(), |d| d.to_string()),
        stats = PyStr(&stats_json("0.3.99")),
        info = PyStr(&info_json(&ckpts, &["pixel.safetensors"], &[])),
    );
    std::fs::write(dir.join("main.py"), script).expect("main.py");
}

/// A string as a single-quoted Python literal.
struct PyStr<'a>(&'a str);

impl std::fmt::Display for PyStr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // JSON text has no single quotes or backslash escapes we emit.
        write!(f, "'{}'", self.0)
    }
}

/// A startable installation in `root`: `main.py` plus a `venv`.
pub fn fake_installation(root: &Path, fake: &FakeComfy) {
    std::fs::create_dir_all(root).expect("root");
    write_main_py(root, fake);
    add_venv(root, "venv");
}

/// The argument lines `main.py` recorded, one per start.
pub fn starts(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join("starts.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// How a [`FileServer`] serves one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serve {
    /// Honours `Range: bytes=N-` with 206.
    Ranges,
    /// Ignores ranges and always sends the whole file with 200.
    Whole,
    /// The first request announces the whole file but the connection
    /// drops after this many bytes; later requests behave like `Ranges`.
    DropAfter(usize),
}

type Requests = std::sync::Arc<std::sync::Mutex<Vec<(String, Option<String>)>>>;

/// A static file server on 127.0.0.1 (404 for unknown paths) that records
/// each request's path and `Range` header.
pub struct FileServer {
    pub port: u16,
    requests: Requests,
}

impl FileServer {
    pub fn start(files: Vec<(&str, Vec<u8>, Serve)>) -> Self {
        let files: Vec<(String, Vec<u8>, Serve)> = files
            .into_iter()
            .map(|(p, b, s)| (p.to_string(), b, s))
            .collect();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let requests: Requests = Default::default();
        let log = std::sync::Arc::clone(&requests);
        thread::spawn(move || {
            let mut dropped = false;
            for stream in listener.incoming().flatten() {
                let Some((path, range)) = read_request_head(&stream) else {
                    continue;
                };
                log.lock()
                    .expect("lock")
                    .push((path.clone(), range.clone()));
                let mut stream = stream;
                let Some((_, bytes, serve)) = files.iter().find(|(p, _, _)| *p == path) else {
                    let _ = stream.write_all(Route::json("", 404, "{}").response.as_bytes());
                    continue;
                };
                let start = range
                    .as_deref()
                    .and_then(|r| r.strip_prefix("bytes="))
                    .and_then(|r| r.strip_suffix('-'))
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|_| *serve != Serve::Whole);
                let (head, body) = match start {
                    Some(start) if start >= bytes.len() => (
                        format!(
                            "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            bytes.len()
                        ),
                        &bytes[..0],
                    ),
                    Some(start) => (
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            bytes.len() - 1,
                            bytes.len(),
                            bytes.len() - start
                        ),
                        &bytes[start..],
                    ),
                    None => (
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            bytes.len()
                        ),
                        &bytes[..],
                    ),
                };
                let _ = stream.write_all(head.as_bytes());
                match serve {
                    Serve::DropAfter(n) if !dropped => {
                        dropped = true;
                        let _ = stream.write_all(&body[..(*n).min(body.len())]);
                        let _ = stream.flush();
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                    }
                    _ => {
                        let _ = stream.write_all(body);
                    }
                }
            }
        });
        Self { port, requests }
    }

    /// `http://127.0.0.1:<port><path>`.
    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    /// Path and `Range` header of every request so far.
    pub fn requests(&self) -> Vec<(String, Option<String>)> {
        self.requests.lock().expect("lock").clone()
    }
}

/// The path and `Range` header of a request.
fn read_request_head(mut stream: &TcpStream) -> Option<(String, Option<String>)> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        raw.extend_from_slice(&buf[..n]);
    }
    let text = String::from_utf8_lossy(&raw);
    let path = text.split_whitespace().nth(1)?.to_string();
    let range = text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("range")
            .then(|| value.trim().to_string())
    });
    Some((path, range))
}

/// The interpreter of the shared test venv (runs fake `uv` and `main.py`).
pub fn test_python() -> PathBuf {
    let venv = shared_venv();
    if cfg!(windows) {
        venv.join("Scripts").join("python.exe")
    } else {
        venv.join("bin").join("python")
    }
}

/// The text of a fake `main.py` (see [`write_main_py`]).
pub fn main_py_text(fake: &FakeComfy) -> Vec<u8> {
    let tmp = tempfile::tempdir().expect("tmp");
    write_main_py(tmp.path(), fake);
    std::fs::read(tmp.path().join("main.py")).expect("main.py")
}
