//! The live MCP endpoint (docs/specs/mcp-live-desktop.md): MCP Streamable
//! HTTP on 127.0.0.1 with JSON responses, bearer tokens, sessions by
//! `Mcp-Session-Id`, and tool calls forwarded to the editor in the webview
//! through a [`Bridge`].
//!
//! Every connection has its own thread; a call waits for the webview's
//! answer (or its timeout) on its connection's thread, so concurrent calls
//! never wait for each other (AC-08). Tokens: the external token while the
//! setting is on (persisted with the port, AC-03), plus one token per
//! Assistant chat, held in memory only (AC-22).

use std::collections::HashMap;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use pincel_agent::{ToolDef, list_tools};
use pincel_mcp::server::{Routed, ServerInfo, live_response, route, tool_error};
use serde::Serialize;
use serde_json::{Value, json};

use super::http::{ReadError, Request, Response, read_request, write_response};
use super::secret::{Config, new_session_id, new_token, token_eq};

/// The endpoint's default port (fixed, unassigned by IANA; AC-21).
pub const DEFAULT_PORT: u16 = 39217;

/// A connection without a byte for this long is closed.
const CONNECTION_IDLE: Duration = Duration::from_secs(120);
/// How often waiting threads look at the stop flag.
const POLL: Duration = Duration::from_millis(100);

const INFO: ServerInfo = ServerInfo {
    name: "pincel-desktop",
    instructions: "Pincel desktop app: the tools act on the documents open in the editor's tabs, live on screen. doc.list lists the open tabs; a call without `doc` acts on the active tab. Read pincel://guide first. Every mutating call is one undo step the user can undo. doc.open, doc.save to a new path and every file export or import open the app's file dialog for the user to confirm; a call fails with \"user cancelled\" when they cancel, and with \"editor busy\" while a dialog is open or the in-app assistant is working (retry later).",
};

/// Tools a chat's internal session neither lists nor runs (ai-chat-panel
/// AC-26): a chat belongs to one tab, and an answer is one undo step.
const CHAT_HIDDEN: &[&str] = &[
    "doc.open",
    "doc.new",
    "doc.close",
    "doc.list",
    "doc.undo",
    "doc.redo",
];

/// Tools that return pixel data, hidden from a chat while "Agent may see
/// the image" is off (ai-chat-panel AC-17).
const IMAGE_TOOLS: &[&str] = &["doc.preview", "doc.read_pixels", "tile.read_pixels"];

/// Tools the desktop app adds beside the document tools (`gen.*` from
/// `comfyui-generate`), listed whether or not they can run (AC-06).
fn host_tools() -> Vec<ToolDef> {
    Vec::new()
}

/// How long the endpoint waits (AC-12, AC-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// A tool call's answer.
    pub call: Duration,
    /// A `gen.*` call's answer.
    pub gen_call: Duration,
    /// A session without a request ends after this.
    pub idle: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            call: Duration::from_secs(30),
            gen_call: Duration::from_secs(300),
            idle: Duration::from_secs(600),
        }
    }
}

impl Timing {
    /// The answer timeout of the tool `tool`.
    pub fn limit_for(&self, tool: &str) -> Duration {
        if tool.starts_with("gen.") {
            self.gen_call
        } else {
            self.call
        }
    }
}

/// A tool call for the editor.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ForwardedCall {
    /// Answer it with [`Endpoint::deliver`] under this id.
    pub id: u64,
    /// The tool name.
    pub tool: String,
    /// The arguments (`null` without any).
    pub args: Value,
    /// The chat whose session made the call; `None` for an external client.
    pub chat: Option<String>,
}

/// The webview side: where calls go and who hears about sessions.
pub trait Bridge: Send + Sync {
    /// Hand `call` to the editor.
    fn forward(&self, call: &ForwardedCall);
    /// The number of open external sessions changed (the status-bar
    /// indicator, AC-12).
    fn sessions_changed(&self, external: usize);
}

/// What the AI agents dialog shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// "Allow AI agents to control Pincel" is on.
    pub enabled: bool,
    /// The port while the endpoint listens.
    pub port: Option<u16>,
    /// The external token while the setting is on.
    pub token: Option<String>,
    /// `http://127.0.0.1:<port>/mcp` while the setting is on.
    pub url: Option<String>,
    /// The client command to copy while the setting is on.
    pub command: Option<String>,
    /// The last start could not bind the stored (or default) port (AC-21).
    pub port_changed: bool,
    /// Open external sessions.
    pub external_sessions: usize,
}

/// How a chat reaches the endpoint (AC-22).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAccess {
    /// `http://127.0.0.1:<port>/mcp`.
    pub url: String,
    /// The port.
    pub port: u16,
    /// The chat's bearer token.
    pub token: String,
}

/// Who a request speaks for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    External,
    Chat(String),
}

struct Session {
    kind: Kind,
    last_seen: Instant,
}

struct Chat {
    token: String,
    image_access: bool,
    answering: bool,
}

/// The listening socket's thread.
struct Server {
    port: u16,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct State {
    enabled: bool,
    token: Option<String>,
    port_changed: bool,
    chats: HashMap<String, Chat>,
    sessions: HashMap<String, Session>,
    server: Option<Server>,
    /// The external session count the bridge heard last.
    reported: usize,
}

impl State {
    fn external_sessions(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.kind == Kind::External)
            .count()
    }
}

struct Pending {
    reply: Option<Value>,
    paused_at: Option<Instant>,
    paused: Duration,
}

#[derive(Default)]
struct Calls {
    next: u64,
    pending: HashMap<u64, Pending>,
}

struct Inner {
    bridge: Box<dyn Bridge>,
    config_path: PathBuf,
    default_port: u16,
    timing: Timing,
    state: Mutex<State>,
    calls: Mutex<Calls>,
    answered: Condvar,
}

/// The endpoint; cheap to clone, every clone is the same endpoint.
#[derive(Clone)]
pub struct Endpoint(Arc<Inner>);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    // A thread that panicked mid-update leaves plain data behind; keep
    // serving rather than taking the app down.
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Endpoint {
    /// A stopped endpoint that persists its config at `config_path` and
    /// tries `default_port` first.
    pub fn new(
        bridge: Box<dyn Bridge>,
        config_path: PathBuf,
        default_port: u16,
        timing: Timing,
    ) -> Self {
        Self(Arc::new(Inner {
            bridge,
            config_path,
            default_port,
            timing,
            state: Mutex::new(State::default()),
            calls: Mutex::new(Calls::default()),
            answered: Condvar::new(),
        }))
    }

    /// What the AI agents dialog shows.
    pub fn status(&self) -> Status {
        self.0.sweep();
        let st = lock(&self.0.state);
        self.0.status(&st)
    }

    /// Turn the setting on (AC-02, AC-03): listen (keeping the port), with
    /// a new token when `fresh` (the user switched it on) or none is
    /// stored, else the stored one (app start).
    pub fn enable(&self, fresh: bool) -> Result<Status, String> {
        let mut st = lock(&self.0.state);
        let mut cfg = Config::load(&self.0.config_path);
        if fresh || cfg.token.is_none() {
            cfg.token = Some(new_token());
        }
        let started = st.server.is_none();
        self.0.ensure_server(&mut st, &mut cfg)?;
        if let Err(e) = cfg.save(&self.0.config_path) {
            // Without a stored token the setting cannot survive a restart:
            // stay off, and stop a listener started only for it.
            let server = if started && st.chats.is_empty() {
                st.server.take()
            } else {
                None
            };
            drop(st);
            halt(server);
            return Err(e);
        }
        if st.token != cfg.token {
            // Sessions opened with the old token cannot go on.
            st.sessions.retain(|_, s| s.kind != Kind::External);
        }
        st.enabled = true;
        st.token = cfg.token;
        let status = self.0.status(&st);
        self.0.report(st);
        Ok(status)
    }

    /// Turn the setting off (AC-04): end the external sessions and, unless
    /// a chat holds a token, stop listening.
    pub fn disable(&self) -> Status {
        let mut st = lock(&self.0.state);
        st.enabled = false;
        st.token = None;
        st.sessions.retain(|_, s| s.kind != Kind::External);
        let server = if st.chats.is_empty() {
            st.server.take()
        } else {
            None
        };
        let status = self.0.status(&st);
        self.0.report(st);
        halt(server);
        status
    }

    /// Replace the external token (AC-20); open external sessions end.
    pub fn regenerate(&self) -> Result<Status, String> {
        let mut st = lock(&self.0.state);
        if !st.enabled {
            return Err("AI agent access is off".to_owned());
        }
        let mut cfg = Config::load(&self.0.config_path);
        cfg.token = Some(new_token());
        cfg.save(&self.0.config_path)?;
        st.token = cfg.token;
        st.sessions.retain(|_, s| s.kind != Kind::External);
        let status = self.0.status(&st);
        self.0.report(st);
        Ok(status)
    }

    /// The indicator's disconnect button (AC-12): end the external
    /// sessions; the token stays.
    pub fn disconnect(&self) -> Status {
        let mut st = lock(&self.0.state);
        st.sessions.retain(|_, s| s.kind != Kind::External);
        let status = self.0.status(&st);
        self.0.report(st);
        status
    }

    /// Stop listening for good (app exit, tests).
    pub fn shutdown(&self) {
        let server = lock(&self.0.state).server.take();
        halt(server);
    }

    /// The editor's answer to the forwarded call `id`; an answer after the
    /// timeout is dropped.
    pub fn deliver(&self, id: u64, reply: Value) {
        let mut calls = lock(&self.0.calls);
        if let Some(p) = calls.pending.get_mut(&id) {
            p.reply = Some(reply);
            self.0.answered.notify_all();
        }
    }

    /// A dialog the call `id` opened waits for the user (`true`) or closed
    /// (`false`): the wait does not count towards the timeout (AC-19).
    pub fn set_waiting(&self, id: u64, waiting: bool) {
        let mut calls = lock(&self.0.calls);
        if let Some(p) = calls.pending.get_mut(&id) {
            match (waiting, p.paused_at) {
                (true, None) => p.paused_at = Some(Instant::now()),
                (false, Some(at)) => {
                    p.paused += at.elapsed();
                    p.paused_at = None;
                }
                _ => {}
            }
            self.0.answered.notify_all();
        }
    }

    /// A chat sends its first message (AC-22): its token, the endpoint
    /// started if needed, whatever the setting.
    pub fn chat_start(&self, chat: &str, image_access: bool) -> Result<ChatAccess, String> {
        let mut st = lock(&self.0.state);
        if st.server.is_none() {
            let mut cfg = Config::load(&self.0.config_path);
            self.0.ensure_server(&mut st, &mut cfg)?;
            // Keep the port for later starts; the stored token stays.
            cfg.save(&self.0.config_path)?;
        }
        let port = st.server.as_ref().map_or(0, |s| s.port);
        let entry = st.chats.entry(chat.to_owned()).or_insert_with(|| Chat {
            token: new_token(),
            image_access,
            answering: false,
        });
        Ok(ChatAccess {
            url: url(port),
            port,
            token: entry.token.clone(),
        })
    }

    /// The chat ended (New chat, tab closed, CLI or model changed, app
    /// quit): its token stops working and its sessions end (AC-22).
    pub fn chat_end(&self, chat: &str) {
        let mut st = lock(&self.0.state);
        st.chats.remove(chat);
        let kind = Kind::Chat(chat.to_owned());
        st.sessions.retain(|_, s| s.kind != kind);
        let server = if !st.enabled && st.chats.is_empty() {
            st.server.take()
        } else {
            None
        };
        drop(st);
        halt(server);
    }

    /// The chat's answer runs (`true`) or ended (AC-23).
    pub fn chat_answering(&self, chat: &str, running: bool) {
        if let Some(c) = lock(&self.0.state).chats.get_mut(chat) {
            c.answering = running;
        }
    }

    /// "Agent may see the image" changed for the chat (ai-chat-panel AC-17).
    pub fn chat_image_access(&self, chat: &str, on: bool) {
        if let Some(c) = lock(&self.0.state).chats.get_mut(chat) {
            c.image_access = on;
        }
    }
}

fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/mcp")
}

/// Stop a listener taken out of the state (with the state unlocked: its
/// thread locks the state to sweep); connection threads close at their
/// next poll.
fn halt(server: Option<Server>) {
    if let Some(mut server) = server {
        server.stop.store(true, Ordering::SeqCst);
        if let Some(t) = server.accept.take() {
            let _ = t.join();
        }
    }
}

impl Inner {
    fn status(&self, st: &State) -> Status {
        let port = st.server.as_ref().map(|s| s.port);
        let (url, command) = match (st.enabled, port, &st.token) {
            (true, Some(port), Some(token)) => (
                Some(url(port)),
                Some(format!(
                    "claude mcp add --transport http pincel {} --header \"Authorization: Bearer {token}\"",
                    url(port)
                )),
            ),
            _ => (None, None),
        };
        Status {
            enabled: st.enabled,
            port,
            token: if st.enabled { st.token.clone() } else { None },
            url,
            command,
            port_changed: st.port_changed,
            external_sessions: st.external_sessions(),
        }
    }

    /// Tell the bridge the external session count if it changed, after
    /// releasing the state.
    fn report(&self, mut st: MutexGuard<'_, State>) {
        let n = st.external_sessions();
        if n == st.reported {
            return;
        }
        st.reported = n;
        drop(st);
        self.bridge.sessions_changed(n);
    }

    /// End sessions idle for longer than [`Timing::idle`] (AC-12).
    fn sweep(&self) {
        let mut st = lock(&self.state);
        let idle = self.timing.idle;
        st.sessions.retain(|_, s| s.last_seen.elapsed() < idle);
        self.report(st);
    }

    /// Listen if not yet: the stored port, else the default; an OS-assigned
    /// one when that is taken (AC-21). Updates `cfg.port`.
    fn ensure_server(self: &Arc<Self>, st: &mut State, cfg: &mut Config) -> Result<(), String> {
        if st.server.is_some() {
            return Ok(());
        }
        let want = cfg.port.unwrap_or(self.default_port);
        let (listener, changed) = match TcpListener::bind(("127.0.0.1", want)) {
            Ok(l) => (l, false),
            Err(_) => (
                TcpListener::bind(("127.0.0.1", 0))
                    .map_err(|e| format!("cannot listen on 127.0.0.1: {e}"))?,
                true,
            ),
        };
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        cfg.port = Some(port);
        st.port_changed = changed;
        let stop = Arc::new(AtomicBool::new(false));
        let inner = Arc::clone(self);
        let flag = Arc::clone(&stop);
        let accept = std::thread::Builder::new()
            .name("mcp-accept".to_owned())
            .spawn(move || accept_loop(inner, listener, flag))
            .map_err(|e| e.to_string())?;
        st.server = Some(Server {
            port,
            stop,
            accept: Some(accept),
        });
        Ok(())
    }

    /// Who the `Authorization` header speaks for, if anyone (AC-05, AC-22).
    fn authorize(&self, header: Option<&str>) -> Option<Kind> {
        let presented = header.and_then(|h| {
            let (scheme, token) = h.split_once(' ')?;
            scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
        })?;
        let st = lock(&self.state);
        if st.enabled && st.token.as_deref().is_some_and(|t| token_eq(presented, t)) {
            return Some(Kind::External);
        }
        st.chats
            .iter()
            .find(|(_, c)| token_eq(presented, &c.token))
            .map(|(id, _)| Kind::Chat(id.clone()))
    }

    fn handle(&self, req: &Request) -> Response {
        self.sweep();
        if req.path != "/mcp" {
            return Response::json(404, &json!({ "error": "not found; the endpoint is /mcp" }));
        }
        if let Some(origin) = req.header("origin")
            && !local_origin(origin)
        {
            return Response::json(403, &json!({ "error": "origin not allowed" }));
        }
        let Some(kind) = self.authorize(req.header("authorization")) else {
            return Response::json(401, &json!({ "error": "missing or wrong bearer token" }))
                .with("WWW-Authenticate", "Bearer");
        };
        match req.method.as_str() {
            "POST" => self.post(kind, req),
            "DELETE" => {
                let ended = req.header("mcp-session-id").is_some_and(|id| {
                    let mut st = lock(&self.state);
                    let ours = st.sessions.get(id).is_some_and(|s| s.kind == kind);
                    if ours {
                        st.sessions.remove(id);
                    }
                    self.report(st);
                    ours
                });
                Response::empty(if ended { 200 } else { 404 })
            }
            // No server-sent event stream: JSON responses only.
            _ => Response::empty(405).with("Allow", "POST, DELETE"),
        }
    }

    fn post(&self, kind: Kind, req: &Request) -> Response {
        let msg: Value = match serde_json::from_slice(&req.body) {
            Ok(v) => v,
            Err(e) => {
                return Response::json(
                    400,
                    &json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("Parse error: {e}") } }),
                );
            }
        };
        let initialize = msg.get("method").and_then(Value::as_str) == Some("initialize")
            && msg.get("id").is_some();
        if !initialize {
            let Some(id) = req.header("mcp-session-id") else {
                return Response::json(400, &json!({ "error": "missing Mcp-Session-Id header" }));
            };
            let mut st = lock(&self.state);
            match st.sessions.get_mut(id) {
                Some(s) if s.kind == kind => s.last_seen = Instant::now(),
                _ => return Response::json(404, &json!({ "error": "session not found" })),
            }
        }
        let response = match route(&INFO, msg, || self.tools_for(&kind)) {
            Routed::Reply(None) => return Response::empty(202),
            Routed::Reply(Some(v)) => Response::json(200, &v),
            Routed::ToolCall { id, name, args } => {
                Response::json(200, &self.call_tool(&kind, id, &name, args))
            }
        };
        if !initialize {
            return response;
        }
        let id = new_session_id();
        let mut st = lock(&self.state);
        st.sessions.insert(
            id.clone(),
            Session {
                kind,
                last_seen: Instant::now(),
            },
        );
        self.report(st);
        response.with("Mcp-Session-Id", id)
    }

    /// The tools `tools/list` reports to `kind` (AC-06).
    fn tools_for(&self, kind: &Kind) -> Vec<ToolDef> {
        let all = list_tools().into_iter().chain(host_tools());
        match kind {
            Kind::External => all.collect(),
            Kind::Chat(chat) => {
                let image = lock(&self.state)
                    .chats
                    .get(chat)
                    .is_some_and(|c| c.image_access);
                all.filter(|t| {
                    !CHAT_HIDDEN.contains(&t.name) && (image || !IMAGE_TOOLS.contains(&t.name))
                })
                .collect()
            }
        }
    }

    fn call_tool(&self, kind: &Kind, id: Value, name: &str, args: Value) -> Value {
        {
            let st = lock(&self.state);
            match kind {
                Kind::Chat(chat) => {
                    if CHAT_HIDDEN.contains(&name) {
                        return tool_error(id, &format!("`{name}` is not available in the chat"));
                    }
                    let image = st.chats.get(chat).is_some_and(|c| c.image_access);
                    if !image && IMAGE_TOOLS.contains(&name) {
                        return tool_error(id, "image access is off");
                    }
                }
                Kind::External => {
                    if st.chats.values().any(|c| c.answering) {
                        return tool_error(id, "editor busy: the assistant is working");
                    }
                }
            }
        }
        let call_id = {
            let mut calls = lock(&self.calls);
            calls.next += 1;
            let call_id = calls.next;
            calls.pending.insert(
                call_id,
                Pending {
                    reply: None,
                    paused_at: None,
                    paused: Duration::ZERO,
                },
            );
            call_id
        };
        self.bridge.forward(&ForwardedCall {
            id: call_id,
            tool: name.to_owned(),
            args,
            chat: match kind {
                Kind::Chat(c) => Some(c.clone()),
                Kind::External => None,
            },
        });
        let limit = self.timing.limit_for(name);
        match self.wait(call_id, limit) {
            Some(wire) => live_response(id, &wire),
            None => tool_error(
                id,
                &format!(
                    "`{name}` timed out after {} s without a result from the editor",
                    limit.as_secs_f32().round()
                ),
            ),
        }
    }

    /// Wait for the answer to `call_id`, not counting time a dialog waits
    /// for the user; `None` on timeout.
    fn wait(&self, call_id: u64, limit: Duration) -> Option<Value> {
        let start = Instant::now();
        let mut calls = lock(&self.calls);
        loop {
            let p = calls.pending.get_mut(&call_id)?;
            if let Some(reply) = p.reply.take() {
                calls.pending.remove(&call_id);
                return Some(reply);
            }
            let paused = p.paused + p.paused_at.map_or(Duration::ZERO, |at| at.elapsed());
            let waited = start.elapsed().saturating_sub(paused);
            let dialog_open = p.paused_at.is_some();
            if !dialog_open && waited >= limit {
                calls.pending.remove(&call_id);
                return None;
            }
            let nap = if dialog_open {
                POLL
            } else {
                (limit - waited).min(POLL)
            };
            calls = self
                .answered
                .wait_timeout(calls, nap)
                .map(|(g, _)| g)
                .unwrap_or_else(|e| e.into_inner().0);
        }
    }
}

/// `true` for an `Origin` of this machine (DNS-rebinding guard).
fn local_origin(origin: &str) -> bool {
    if origin == "null" {
        return true;
    }
    let host = origin
        .split_once("://")
        .map_or(origin, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or_default();
    let host = host
        .rsplit_once(':')
        .filter(|(h, p)| !h.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        .map_or(host, |(h, _)| h);
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

fn accept_loop(inner: Arc<Inner>, listener: TcpListener, stop: Arc<AtomicBool>) {
    let mut last_sweep = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, addr)) => {
                let (inner, stop) = (Arc::clone(&inner), Arc::clone(&stop));
                let _ = std::thread::Builder::new()
                    .name("mcp-connection".to_owned())
                    .spawn(move || serve(inner, stream, addr, stop));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
        if last_sweep.elapsed() >= Duration::from_millis(250) {
            // Idle sessions end, and the indicator hides, without a request.
            inner.sweep();
            last_sweep = Instant::now();
        }
    }
}

fn serve(inner: Arc<Inner>, mut stream: TcpStream, addr: SocketAddr, stop: Arc<AtomicBool>) {
    if !addr.ip().is_loopback()
        || stream.set_nonblocking(false).is_err()
        || stream.set_read_timeout(Some(POLL)).is_err()
    {
        return;
    }
    let _ = stream.set_nodelay(true);
    let mut buf = Vec::new();
    let stopped = || stop.load(Ordering::SeqCst);
    loop {
        let req = match read_request(&mut stream, &mut buf, &stopped, CONNECTION_IDLE) {
            Ok(r) => r,
            Err(ReadError::Bad) => {
                let _ = write_response(&mut stream, &Response::empty(400), false);
                return;
            }
            Err(ReadError::TooLarge) => {
                let _ = write_response(&mut stream, &Response::empty(413), false);
                return;
            }
            Err(ReadError::Closed | ReadError::Stopped) => return,
        };
        let res = inner.handle(&req);
        if stopped() {
            return;
        }
        if write_response(&mut stream, &res, req.keep_alive).is_err() || !req.keep_alive {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_origin_accepts_only_this_machine() {
        assert!(local_origin("http://localhost:5173"));
        assert!(local_origin("http://127.0.0.1"));
        assert!(local_origin("http://[::1]:80"));
        assert!(!local_origin("https://evil.example"));
        assert!(!local_origin("http://localhost.evil.example"));
    }
}
