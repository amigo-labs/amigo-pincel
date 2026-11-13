//! A fake ComfyUI for the generation tests: uploads, `/prompt`, `/queue`,
//! `/interrupt`, `/history`, `/view` and the progress socket, with a
//! script that holds, stalls, fails or shortens a prompt.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{self, Message};

/// How the fake treats the next prompts.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// Other prompts shown ahead of a held prompt.
    pub ahead: u32,
    /// Keep prompts pending until [`FakeGen::release`].
    pub hold: bool,
    /// Keep a running prompt at its first step until [`FakeGen::release`].
    pub stall: bool,
    /// Images to save; `None` saves one per variant.
    pub images: Option<u32>,
    /// `execution_error` (type, message) after the first variant.
    pub error: Option<(String, String)>,
    /// Answer `/prompt` with 400 and this body.
    pub reject: Option<String>,
    /// Refuse the socket (the job polls).
    pub no_socket: bool,
}

#[derive(Default)]
struct State {
    script: Script,
    dead: bool,
    next: u32,
    uploads: Vec<(String, Vec<u8>)>,
    prompts: Vec<Value>,
    deleted: Vec<String>,
    interrupts: Vec<Value>,
    pending: Vec<String>,
    running: Option<String>,
    history: serde_json::Map<String, Value>,
    sockets: HashMap<String, Sender<Message>>,
}

/// The fake server; alive for the rest of the test process.
pub struct FakeGen {
    pub port: u16,
    state: Arc<Mutex<State>>,
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl FakeGen {
    pub fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let state = Arc::new(Mutex::new(State {
            script,
            ..State::default()
        }));
        let shared = Arc::clone(&state);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                if lock(&shared).dead {
                    continue;
                }
                let shared = Arc::clone(&shared);
                thread::spawn(move || serve(&shared, stream));
            }
        });
        Self { port, state }
    }

    /// `http://127.0.0.1:<port>`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Lets held and stalled prompts go on.
    pub fn release(&self) {
        let mut st = lock(&self.state);
        st.script.hold = false;
        st.script.stall = false;
    }

    /// Stops answering: open sockets close, new connections are dropped.
    pub fn die(&self) {
        lock(&self.state).dead = true;
    }

    /// Uploaded files: name and bytes.
    pub fn uploads(&self) -> Vec<(String, Vec<u8>)> {
        lock(&self.state).uploads.clone()
    }

    /// The `/prompt` bodies.
    pub fn prompts(&self) -> Vec<Value> {
        lock(&self.state).prompts.clone()
    }

    /// Prompt ids deleted from the queue.
    pub fn deleted(&self) -> Vec<String> {
        lock(&self.state).deleted.clone()
    }

    /// `/interrupt` bodies.
    pub fn interrupts(&self) -> Vec<Value> {
        lock(&self.state).interrupts.clone()
    }

    /// Queues a prompt of another client (it never runs on its own).
    pub fn queue_foreign(&self, id: &str) {
        lock(&self.state).pending.insert(0, id.to_string());
    }
}

struct Request {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut raw = Vec::new();
    let mut buf = [0u8; 8192];
    let head_end = loop {
        if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        raw.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&raw[..head_end]).to_string();
    let mut words = head.split_whitespace();
    let method = words.next()?.to_string();
    let path = words.next()?.to_string();
    let length = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    let mut body = raw[head_end..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
    }
    Some(Request { method, path, body })
}

fn respond(stream: &mut TcpStream, status: u16, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn serve(state: &Arc<Mutex<State>>, mut stream: TcpStream) {
    let mut peek = [0u8; 4096];
    let n = stream.peek(&mut peek).unwrap_or(0);
    let head = String::from_utf8_lossy(&peek[..n]).to_string();
    if head.to_ascii_lowercase().contains("upgrade: websocket") {
        if lock(state).script.no_socket {
            respond(&mut stream, 404, b"{}");
        } else {
            let client = head
                .split_whitespace()
                .nth(1)
                .and_then(|path| path.split_once("clientId="))
                .map(|(_, id)| id.to_string())
                .unwrap_or_default();
            socket(state, stream, client);
        }
        return;
    }
    let Some(req) = read_request(&mut stream) else {
        return;
    };
    let (status, body) = route(state, &req);
    respond(&mut stream, status, &body);
}

/// Accepts the progress socket of `client` and writes what the executor
/// sends it.
fn socket(state: &Arc<Mutex<State>>, stream: TcpStream, client: String) {
    let Ok(mut ws) = tungstenite::accept(stream) else {
        return;
    };
    let (tx, rx): (Sender<Message>, Receiver<Message>) = channel();
    lock(state).sockets.insert(client, tx);
    loop {
        if lock(state).dead {
            return;
        }
        match rx.recv_timeout(Duration::from_millis(20)) {
            Ok(msg) => {
                if ws.send(msg).is_err() {
                    return;
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn json_body(v: &Value) -> Vec<u8> {
    v.to_string().into_bytes()
}

fn route(state: &Arc<Mutex<State>>, req: &Request) -> (u16, Vec<u8>) {
    let (path, query) = req.path.split_once('?').unwrap_or((&req.path, ""));
    match (req.method.as_str(), path) {
        ("POST", "/upload/image") => {
            let (name, bytes) = multipart_file(&req.body);
            lock(state).uploads.push((name.clone(), bytes));
            (
                200,
                json_body(&json!({ "name": name, "subfolder": "", "type": "input" })),
            )
        }
        ("POST", "/prompt") => {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let mut st = lock(state);
            st.prompts.push(body.clone());
            if let Some(reject) = st.script.reject.clone() {
                return (400, reject.into_bytes());
            }
            st.next += 1;
            let id = format!("prompt-{}", st.next);
            st.pending.push(id.clone());
            let client = body
                .get("client_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let prompt = body.get("prompt").cloned().unwrap_or(Value::Null);
            drop(st);
            let shared = Arc::clone(state);
            let run_id = id.clone();
            thread::spawn(move || execute(&shared, &run_id, &client, &prompt));
            (200, json_body(&json!({ "prompt_id": id, "number": 1 })))
        }
        ("GET", "/queue") => {
            let st = lock(state);
            let held = st.script.hold;
            let mut pending: Vec<Value> = Vec::new();
            if held {
                for k in 0..st.script.ahead {
                    pending.push(json!([k, format!("other-{k}"), {}, {}, []]));
                }
            }
            for (i, id) in st.pending.iter().enumerate() {
                pending.push(json!([100 + i, id, {}, {}, []]));
            }
            let running: Vec<Value> = st
                .running
                .iter()
                .map(|id| json!([50, id, {}, {}, []]))
                .collect();
            (
                200,
                json_body(&json!({ "queue_running": running, "queue_pending": pending })),
            )
        }
        ("POST", "/queue") => {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let mut st = lock(state);
            for id in body
                .get("delete")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                st.deleted.push(id.to_string());
                st.pending.retain(|p| p != id);
            }
            (200, b"{}".to_vec())
        }
        ("POST", "/interrupt") => {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            lock(state).interrupts.push(body);
            (200, b"{}".to_vec())
        }
        ("GET", "/view") => {
            let name = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("filename="))
                .unwrap_or_default();
            (200, format!("PNG:{name}").into_bytes())
        }
        ("GET", p) if p.starts_with("/history/") => {
            let id = &p["/history/".len()..];
            let st = lock(state);
            let entry = st
                .history
                .get(id)
                .map(|e| json!({ id: e }))
                .unwrap_or_else(|| json!({}));
            (200, json_body(&entry))
        }
        _ => (404, b"{}".to_vec()),
    }
}

/// The first file part of a multipart body: its file name and bytes.
fn multipart_file(body: &[u8]) -> (String, Vec<u8>) {
    let text = String::from_utf8_lossy(body);
    let name = text
        .split("filename=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .to_string();
    let start = body
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map_or(0, |i| i + 4);
    let end = body[start..]
        .windows(4)
        .position(|w| w == b"\r\n--")
        .map_or(body.len(), |i| start + i);
    (name, body[start..end].to_vec())
}

fn send(state: &Arc<Mutex<State>>, client: &str, msg: Message) {
    if let Some(tx) = lock(state).sockets.get(client) {
        let _ = tx.send(msg);
    }
}

fn event(kind: &str, data: Value) -> Message {
    Message::Text(json!({ "type": kind, "data": data }).to_string().into())
}

/// Plays a prompt: waits while held, runs its samplers step by step,
/// saves the images and writes the history last, as ComfyUI does.
fn execute(state: &Arc<Mutex<State>>, id: &str, client: &str, prompt: &Value) {
    let pause = || thread::sleep(Duration::from_millis(5));
    loop {
        let st = lock(state);
        if st.dead || st.deleted.iter().any(|d| d == id) {
            return;
        }
        if !st.script.hold && st.pending.first().is_some_and(|p| p == id) {
            break;
        }
        drop(st);
        pause();
    }
    let script = {
        let mut st = lock(state);
        st.pending.retain(|p| p != id);
        st.running = Some(id.to_string());
        st.script.clone()
    };
    // The socket may connect a moment after the prompt arrived.
    send(
        state,
        client,
        event("execution_start", json!({ "prompt_id": id })),
    );
    let samplers: Vec<(u32, u32)> = (0..)
        .map_while(|i| {
            prompt.get(format!("sample{i}")).map(|n| {
                let steps = n
                    .pointer("/inputs/steps")
                    .and_then(Value::as_u64)
                    .unwrap_or(1);
                (i, u32::try_from(steps).unwrap_or(1))
            })
        })
        .collect();
    let mut outputs = serde_json::Map::new();
    let wanted = script.images.unwrap_or(u32::MAX);
    let interrupted = |st: &State| {
        st.interrupts
            .iter()
            .any(|b| b.get("prompt_id").and_then(Value::as_str) == Some(id))
    };
    let finish = |outputs: &serde_json::Map<String, Value>, status: Value| {
        let mut st = lock(state);
        st.running = None;
        st.history.insert(
            id.to_string(),
            json!({ "outputs": outputs, "status": status }),
        );
    };
    for (i, steps) in &samplers {
        for value in 1..=*steps {
            loop {
                let st = lock(state);
                if st.dead {
                    return;
                }
                if interrupted(&st) {
                    drop(st);
                    send(
                        state,
                        client,
                        event("execution_interrupted", json!({ "prompt_id": id })),
                    );
                    finish(
                        &outputs,
                        json!({ "status_str": "error", "completed": false, "messages": [] }),
                    );
                    return;
                }
                if !(st.script.stall && value > 1) {
                    break;
                }
                drop(st);
                pause();
            }
            send(
                state,
                client,
                event(
                    "progress",
                    json!({ "prompt_id": id, "node": format!("sample{i}"), "value": value, "max": steps }),
                ),
            );
            if *i == 0 && value == 1 {
                let mut frame = vec![0, 0, 0, 1, 0, 0, 0, 1];
                frame.extend_from_slice(b"jpeg-preview");
                send(state, client, Message::Binary(frame.into()));
            }
        }
        if *i < wanted {
            outputs.insert(
                format!("save{i}"),
                json!({ "images": [{ "filename": format!("pincel_{i}.png"), "subfolder": "", "type": "output" }] }),
            );
        }
        if let Some((kind, message)) = &script.error
            && *i == 0
        {
            let data =
                json!({ "prompt_id": id, "exception_type": kind, "exception_message": message });
            send(state, client, event("execution_error", data.clone()));
            finish(
                &outputs,
                json!({ "status_str": "error", "completed": false, "messages": [["execution_error", data]] }),
            );
            return;
        }
    }
    send(
        state,
        client,
        event("executing", json!({ "prompt_id": id, "node": null })),
    );
    pause();
    finish(
        &outputs,
        json!({ "status_str": "success", "completed": true, "messages": [] }),
    );
}
