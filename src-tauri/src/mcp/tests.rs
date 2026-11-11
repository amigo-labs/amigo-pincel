//! Endpoint tests over real loopback connections with a mocked webview
//! (docs/specs/mcp-live-desktop.md, Testing hints).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use super::endpoint::{Bridge, Endpoint, ForwardedCall, Timing};

/// What the mocked webview does with a forwarded call.
type Responder = Box<dyn Fn(ForwardedCall) + Send>;

/// The webview stand-in: records forwarded calls and session counts; an
/// optional responder answers each call.
#[derive(Default)]
struct Mock {
    calls: Mutex<Vec<ForwardedCall>>,
    sessions: Mutex<Vec<usize>>,
    responder: Mutex<Option<Responder>>,
}

impl Bridge for Arc<Mock> {
    fn forward(&self, call: &ForwardedCall) {
        self.calls.lock().unwrap().push(call.clone());
        if let Some(r) = self.responder.lock().unwrap().as_ref() {
            r(call.clone());
        }
    }

    fn sessions_changed(&self, external: usize) {
        self.sessions.lock().unwrap().push(external);
    }
}

fn timing() -> Timing {
    Timing {
        call: Duration::from_millis(400),
        gen_call: Duration::from_secs(5),
        idle: Duration::from_secs(600),
    }
}

/// An endpoint over `dir` whose default port is a free one.
fn endpoint(dir: &Path, timing: Timing) -> (Endpoint, Arc<Mock>) {
    let mock = Arc::new(Mock::default());
    let ep = Endpoint::new(
        Box::new(mock.clone()),
        dir.join("mcp.json"),
        free_port(),
        timing,
    );
    (ep, mock)
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// One HTTP exchange on a fresh connection: status, headers (lower-case
/// names), body.
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

fn request(
    port: u16,
    method: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> std::io::Result<Reply> {
    let mut s = TcpStream::connect(("127.0.0.1", port))?;
    s.set_read_timeout(Some(Duration::from_secs(20)))?;
    let mut req =
        format!("{method} /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    for (n, v) in headers {
        req.push_str(&format!("{n}: {v}\r\n"));
    }
    req.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    s.write_all(req.as_bytes())?;
    let mut raw = String::new();
    s.read_to_string(&mut raw)?;
    parse_reply(&raw)
}

fn parse_reply(raw: &str) -> std::io::Result<Reply> {
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, raw.to_owned());
    let (head, body) = raw.split_once("\r\n\r\n").ok_or_else(bad)?;
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or_else(bad)?;
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    Ok(Reply {
        status,
        headers,
        body: body.to_owned(),
    })
}

fn rpc(id: i64, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// `initialize` with `token`; returns the session id.
fn initialize(port: u16, token: &str) -> String {
    let auth = bearer(token);
    let r = request(
        port,
        "POST",
        &[
            ("Authorization", &auth),
            ("Content-Type", "application/json"),
        ],
        &rpc(
            1,
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {} }),
        ),
    )
    .unwrap();
    assert_eq!(r.status, 200, "{}", r.body);
    r.header("mcp-session-id").expect("session id").to_owned()
}

fn post(port: u16, token: &str, session: &str, body: &str) -> std::io::Result<Reply> {
    let auth = bearer(token);
    request(
        port,
        "POST",
        &[
            ("Authorization", &auth),
            ("Mcp-Session-Id", session),
            ("Content-Type", "application/json"),
        ],
        body,
    )
}

fn call_tool(port: u16, token: &str, session: &str, id: i64, name: &str) -> Value {
    post(
        port,
        token,
        session,
        &rpc(id, "tools/call", json!({ "name": name, "arguments": {} })),
    )
    .unwrap()
    .json()
}

fn answer_ok(ep: &Endpoint) -> impl Fn(ForwardedCall) + Send + 'static {
    let ep = ep.clone();
    move |call: ForwardedCall| {
        let ep = ep.clone();
        std::thread::spawn(move || {
            ep.deliver(
                call.id,
                json!({ "ok": true, "json": { "tool": call.tool }, "files": [] }),
            );
        });
    }
}

#[test]
fn endpoint_listens_nowhere_until_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, _) = endpoint(dir.path(), timing());
    let status = ep.status();
    assert!(!status.enabled);
    assert_eq!(status.port, None);
    assert!(!dir.path().join("mcp.json").exists());
}

#[test]
fn enable_listens_on_loopback_with_a_new_256_bit_token_and_the_client_command() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, _) = endpoint(dir.path(), timing());
    let s = ep.enable(true).unwrap();
    let (port, token) = (s.port.unwrap(), s.token.clone().unwrap());
    assert!(s.enabled);
    assert_eq!(token.len(), 64);
    assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        s.command.as_deref(),
        Some(
            format!(
                "claude mcp add --transport http pincel http://127.0.0.1:{port}/mcp --header \"Authorization: Bearer {token}\""
            )
            .as_str()
        )
    );
    assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());
    let again = ep.enable(true).unwrap();
    assert_ne!(
        again.token.as_deref(),
        Some(token.as_str()),
        "a new token per switch-on"
    );
    ep.disable();
}

#[test]
fn endpoint_keeps_token_and_port_across_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let (first, _) = endpoint(dir.path(), timing());
    let s = first.enable(true).unwrap();
    first.shutdown();
    let mock = Arc::new(Mock::default());
    // A later start: a different default port, the persisted one wins.
    let second = Endpoint::new(
        Box::new(mock),
        dir.path().join("mcp.json"),
        free_port(),
        timing(),
    );
    let t = second.enable(false).unwrap();
    assert_eq!(t.token, s.token);
    assert_eq!(t.port, s.port);
    assert!(!t.port_changed);
    initialize(t.port.unwrap(), t.token.as_deref().unwrap());
    second.disable();
}

#[test]
fn disable_stops_the_endpoint_and_refuses_later_requests() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, mock) = endpoint(dir.path(), timing());
    let s = ep.enable(true).unwrap();
    let (port, token) = (s.port.unwrap(), s.token.unwrap());
    let session = initialize(port, &token);
    assert_eq!(mock.sessions.lock().unwrap().last(), Some(&1));
    // A kept-alive connection is closed too.
    let mut alive = TcpStream::connect(("127.0.0.1", port)).unwrap();
    alive
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let off = ep.disable();
    assert!(!off.enabled);
    assert_eq!(mock.sessions.lock().unwrap().last(), Some(&0));
    let mut buf = [0u8; 16];
    assert!(
        matches!(alive.read(&mut buf), Ok(0) | Err(_)),
        "connection closed"
    );
    assert!(post(port, &token, &session, &rpc(2, "ping", json!({}))).is_err());
}

#[test]
fn requests_without_or_with_a_wrong_token_are_refused_and_run_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, mock) = endpoint(dir.path(), timing());
    let s = ep.enable(true).unwrap();
    let port = s.port.unwrap();
    let body = rpc(
        1,
        "tools/call",
        json!({ "name": "doc.info", "arguments": {} }),
    );
    let none = request(port, "POST", &[("Content-Type", "application/json")], &body).unwrap();
    assert_eq!(none.status, 401);
    assert!(none.header("www-authenticate").is_some());
    let wrong = request(port, "POST", &[("Authorization", "Bearer 00ff")], &body).unwrap();
    assert_eq!(wrong.status, 401);
    let init = request(
        port,
        "POST",
        &[("Authorization", "Bearer nope")],
        &rpc(1, "initialize", json!({})),
    )
    .unwrap();
    assert_eq!(init.status, 401);
    assert!(mock.calls.lock().unwrap().is_empty());
    ep.disable();
}

#[test]
fn tools_list_matches_the_headless_server() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, _) = endpoint(dir.path(), timing());
    let s = ep.enable(true).unwrap();
    let (port, token) = (s.port.unwrap(), s.token.unwrap());
    let session = initialize(port, &token);
    let r = post(port, &token, &session, &rpc(2, "tools/list", json!({}))).unwrap();
    let got: Vec<(String, Value)> = r.json()["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["name"].as_str().unwrap().to_owned(),
                t["inputSchema"].clone(),
            )
        })
        .collect();
    let want: Vec<(String, Value)> = pincel_agent::list_tools()
        .into_iter()
        .map(|t| (t.name.to_owned(), t.input_schema))
        .collect();
    assert_eq!(got, want);
    ep.disable();
}

#[test]
fn concurrent_calls_each_get_their_own_result() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, mock) = endpoint(dir.path(), timing());
    // Hold the first call until the second arrives, then answer in
    // reverse order.
    let held: Arc<Mutex<Vec<ForwardedCall>>> = Arc::default();
    let (ep2, held2) = (ep.clone(), held.clone());
    *mock.responder.lock().unwrap() = Some(Box::new(move |call: ForwardedCall| {
        let mut h = held2.lock().unwrap();
        h.push(call);
        if h.len() == 2 {
            for c in h.drain(..).rev() {
                ep2.deliver(
                    c.id,
                    json!({ "ok": true, "json": { "tool": c.tool }, "files": [] }),
                );
            }
        }
    }));
    let s = ep.enable(true).unwrap();
    let (port, token) = (s.port.unwrap(), s.token.unwrap());
    let session = initialize(port, &token);
    let threads: Vec<_> = ["doc.info", "layer.list"]
        .into_iter()
        .enumerate()
        .map(|(i, tool)| {
            let (token, session) = (token.clone(), session.clone());
            std::thread::spawn(move || {
                (tool, call_tool(port, &token, &session, 10 + i as i64, tool))
            })
        })
        .collect();
    for t in threads {
        let (tool, r) = t.join().unwrap();
        assert_eq!(r["result"]["isError"], false, "{r}");
        assert_eq!(r["result"]["structuredContent"]["tool"], tool);
    }
    ep.disable();
}

#[test]
fn sessions_open_on_initialize_and_end_on_delete_disconnect_or_idle() {
    let dir = tempfile::tempdir().unwrap();
    let mut t = timing();
    t.idle = Duration::from_millis(300);
    let (ep, mock) = endpoint(dir.path(), t);
    let s = ep.enable(true).unwrap();
    let (port, token) = (s.port.unwrap(), s.token.clone().unwrap());
    let a = initialize(port, &token);
    let b = initialize(port, &token);
    assert_eq!(ep.status().external_sessions, 2);
    let auth = bearer(&token);
    let del = request(
        port,
        "DELETE",
        &[("Authorization", &auth), ("Mcp-Session-Id", &a)],
        "",
    )
    .unwrap();
    assert!(del.status == 200 || del.status == 204, "{}", del.status);
    assert_eq!(ep.status().external_sessions, 1);
    assert_eq!(
        post(port, &token, &a, &rpc(3, "ping", json!({})))
            .unwrap()
            .status,
        404
    );
    ep.disconnect();
    assert_eq!(ep.status().external_sessions, 0);
    assert_eq!(
        post(port, &token, &b, &rpc(4, "ping", json!({})))
            .unwrap()
            .status,
        404
    );
    assert_eq!(ep.status().token, s.token, "disconnect keeps the token");
    let c = initialize(port, &token);
    assert_eq!(ep.status().external_sessions, 1);
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(ep.status().external_sessions, 0, "idle session ended");
    assert_eq!(
        post(port, &token, &c, &rpc(5, "ping", json!({})))
            .unwrap()
            .status,
        404
    );
    let counts = mock.sessions.lock().unwrap().clone();
    assert_eq!(counts, vec![1, 2, 1, 0, 1, 0]);
    ep.disable();
}

#[test]
fn a_call_without_answer_times_out_but_not_while_a_dialog_waits() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, mock) = endpoint(dir.path(), timing());
    let s = ep.enable(true).unwrap();
    let (port, token) = (s.port.unwrap(), s.token.unwrap());
    let session = initialize(port, &token);
    let r = call_tool(port, &token, &session, 2, "doc.info");
    assert_eq!(r["result"]["isError"], true);
    assert!(
        r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("timed out"),
        "{r}"
    );
    // A dialog the call opened pauses the clock.
    let ep2 = ep.clone();
    *mock.responder.lock().unwrap() = Some(Box::new(move |call: ForwardedCall| {
        let ep = ep2.clone();
        std::thread::spawn(move || {
            ep.set_waiting(call.id, true);
            std::thread::sleep(Duration::from_millis(900));
            ep.set_waiting(call.id, false);
            ep.deliver(call.id, json!({ "ok": true, "json": {}, "files": [] }));
        });
    }));
    let r = call_tool(port, &token, &session, 3, "doc.save");
    assert_eq!(r["result"]["isError"], false, "{r}");
    // A late answer to a timed-out call is dropped.
    let first = mock.calls.lock().unwrap()[0].id;
    ep.deliver(first, json!({ "ok": true, "json": {}, "files": [] }));
    ep.disable();
}

#[test]
fn generation_tools_get_five_minutes() {
    let t = Timing::default();
    assert_eq!(t.call, Duration::from_secs(30));
    assert_eq!(t.limit_for("gen.image"), Duration::from_secs(300));
    assert_eq!(t.limit_for("draw.rect"), Duration::from_secs(30));
    assert_eq!(t.idle, Duration::from_secs(600));
}

#[test]
fn regenerate_token_refuses_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, _) = endpoint(dir.path(), timing());
    let s = ep.enable(true).unwrap();
    let (port, old) = (s.port.unwrap(), s.token.unwrap());
    let session = initialize(port, &old);
    let n = ep.regenerate().unwrap();
    let new = n.token.unwrap();
    assert_ne!(new, old);
    assert!(n.command.unwrap().contains(&new));
    assert_eq!(
        post(port, &old, &session, &rpc(2, "ping", json!({})))
            .unwrap()
            .status,
        401
    );
    initialize(port, &new);
    ep.disable();
}

#[test]
fn a_taken_default_port_binds_an_os_port_and_persists_it() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = TcpListener::bind("127.0.0.1:0").unwrap();
    let taken = blocker.local_addr().unwrap().port();
    let mock = Arc::new(Mock::default());
    let ep = Endpoint::new(Box::new(mock), dir.path().join("mcp.json"), taken, timing());
    let s = ep.enable(true).unwrap();
    let port = s.port.unwrap();
    assert_ne!(port, taken);
    assert!(s.port_changed);
    assert!(s.command.unwrap().contains(&format!(":{port}/mcp")));
    ep.shutdown();
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("mcp.json")).unwrap())
            .unwrap();
    assert_eq!(saved["port"], port);
    drop(blocker);
}

#[test]
fn a_chat_token_works_while_the_setting_is_off_and_ends_with_the_chat() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, mock) = endpoint(dir.path(), timing());
    *mock.responder.lock().unwrap() = Some(Box::new(answer_ok(&ep)));
    let chat = ep.chat_start("chat-1", true).unwrap();
    assert!(!ep.status().enabled);
    let port = chat.port;
    let session = initialize(port, &chat.token);
    assert_eq!(
        ep.status().external_sessions,
        0,
        "chat sessions are not external"
    );
    let r = call_tool(port, &chat.token, &session, 2, "draw.rect");
    assert_eq!(r["result"]["isError"], false, "{r}");
    assert_eq!(
        mock.calls.lock().unwrap()[0].chat.as_deref(),
        Some("chat-1")
    );
    // The setting and the indicator's disconnect leave chats alone.
    let s = ep.enable(true).unwrap();
    assert_eq!(s.port, Some(port), "one endpoint for both");
    ep.disconnect();
    ep.disable();
    assert_eq!(
        call_tool(port, &chat.token, &session, 3, "draw.rect")["result"]["isError"],
        false
    );
    // Every other token is refused while the setting is off.
    let r = request(
        port,
        "POST",
        &[("Authorization", &bearer(s.token.as_deref().unwrap()))],
        &rpc(1, "initialize", json!({})),
    )
    .unwrap();
    assert_eq!(r.status, 401);
    ep.chat_end("chat-1");
    assert!(
        post(port, &chat.token, &session, &rpc(4, "ping", json!({}))).is_err(),
        "nothing listens"
    );
}

#[test]
fn a_chat_session_lists_and_runs_only_its_tools() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, mock) = endpoint(dir.path(), timing());
    *mock.responder.lock().unwrap() = Some(Box::new(answer_ok(&ep)));
    let chat = ep.chat_start("c", false).unwrap();
    let session = initialize(chat.port, &chat.token);
    let r = post(
        chat.port,
        &chat.token,
        &session,
        &rpc(2, "tools/list", json!({})),
    )
    .unwrap();
    let names: Vec<String> = r.json()["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    for gone in [
        "doc.open",
        "doc.new",
        "doc.close",
        "doc.list",
        "doc.undo",
        "doc.redo",
        "doc.preview",
        "doc.read_pixels",
        "tile.read_pixels",
    ] {
        assert!(!names.iter().any(|n| n == gone), "{gone} listed");
    }
    assert!(names.iter().any(|n| n == "draw.rect"));
    let r = call_tool(chat.port, &chat.token, &session, 3, "doc.undo");
    assert!(
        r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not available in the chat")
    );
    let r = call_tool(chat.port, &chat.token, &session, 4, "doc.preview");
    assert!(
        r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("image access is off")
    );
    ep.chat_image_access("c", true);
    let r = call_tool(chat.port, &chat.token, &session, 5, "doc.preview");
    assert_eq!(r["result"]["isError"], false);
    assert_eq!(
        mock.calls.lock().unwrap().len(),
        1,
        "refused calls never reach the editor"
    );
    ep.chat_end("c");
}

#[test]
fn an_external_call_during_a_chat_answer_is_refused_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, mock) = endpoint(dir.path(), timing());
    *mock.responder.lock().unwrap() = Some(Box::new(answer_ok(&ep)));
    let s = ep.enable(true).unwrap();
    let (port, token) = (s.port.unwrap(), s.token.unwrap());
    let session = initialize(port, &token);
    let _chat = ep.chat_start("c", true).unwrap();
    ep.chat_answering("c", true);
    let r = call_tool(port, &token, &session, 2, "draw.rect");
    assert_eq!(r["result"]["isError"], true);
    assert_eq!(
        r["result"]["content"][0]["text"],
        "editor busy: the assistant is working"
    );
    assert!(mock.calls.lock().unwrap().is_empty());
    ep.chat_answering("c", false);
    assert_eq!(
        call_tool(port, &token, &session, 3, "draw.rect")["result"]["isError"],
        false
    );
    ep.chat_end("c");
    ep.disable();
}

#[test]
fn a_foreign_origin_and_a_get_stream_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (ep, _) = endpoint(dir.path(), timing());
    let s = ep.enable(true).unwrap();
    let (port, token) = (s.port.unwrap(), s.token.unwrap());
    let auth = bearer(&token);
    let r = request(
        port,
        "POST",
        &[("Authorization", &auth), ("Origin", "https://evil.example")],
        &rpc(1, "initialize", json!({})),
    )
    .unwrap();
    assert_eq!(r.status, 403);
    let r = request(port, "GET", &[("Authorization", &auth)], "").unwrap();
    assert_eq!(r.status, 405);
    let session = initialize(port, &token);
    let r = post(
        port,
        &token,
        &session,
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    )
    .unwrap();
    assert_eq!(r.status, 202);
    ep.disable();
}
