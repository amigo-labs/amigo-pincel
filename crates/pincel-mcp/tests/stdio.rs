//! Spawn the `pincel-mcp` binary and speak JSON-RPC over stdin / stdout
//! (docs/specs/mcp-headless-pixel.md AC-01, AC-34 … AC-38, AC-40).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use pincel_core::import_png;
use serde_json::{Value, json};

fn temp_workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pincel-mcp-it-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create workspace");
    dir
}

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Client {
    fn spawn(args: &[&str], cwd: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_pincel-mcp"))
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn pincel-mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    fn send_raw(&mut self, line: &str) -> Value {
        writeln!(self.stdin, "{line}").expect("write");
        self.stdin.flush().expect("flush");
        let mut out = String::new();
        self.stdout.read_line(&mut out).expect("read");
        serde_json::from_str(&out).expect("response is JSON")
    }

    fn notify(&mut self, method: &str) {
        let msg = json!({ "jsonrpc": "2.0", "method": method });
        writeln!(self.stdin, "{msg}").expect("write");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let msg =
            json!({ "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params });
        let r = self.send_raw(&msg.to_string());
        assert_eq!(r["id"], self.next_id);
        r
    }

    fn tool(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": args }))["result"].clone()
    }

    fn initialize(&mut self) -> Value {
        let r = self.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "it", "version": "0" } }),
        );
        self.notify("notifications/initialized");
        r
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn end_to_end_draw_preview_export_in_temp_workspace() {
    let ws = temp_workspace("e2e");
    let mut c = Client::spawn(&["--workspace", ws.to_str().expect("utf8 path")], &ws);
    let init = c.initialize();
    assert_eq!(init["result"]["serverInfo"]["name"], "pincel-mcp");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(init["result"]["capabilities"]["resources"].is_object());
    let tools = c.request("tools/list", json!({}));
    assert!(
        tools["result"]["tools"]
            .as_array()
            .is_some_and(|t| !t.is_empty())
    );
    assert_eq!(c.request("ping", json!({}))["result"], json!({}));
    let res = c.request("resources/list", json!({}));
    assert!(
        res["result"]["resources"]
            .as_array()
            .is_some_and(|r| r.len() == 7)
    );
    let res = c.request("resources/read", json!({ "uri": "pincel://palettes/db16" }));
    assert!(res["result"]["contents"][0]["text"].as_str().is_some());

    let r = c.tool(
        "doc.new",
        json!({ "mode": "pixel", "width": 8, "height": 8 }),
    );
    assert_eq!(r["isError"], false);
    let r = c.tool(
        "draw.pixels",
        json!({ "pixels": [{ "x": 2, "y": 3, "color": "#ff0000" }, { "x": 7, "y": 7, "color": "#00ff00" }] }),
    );
    assert_eq!(r["isError"], false);
    let r = c.tool("doc.preview", json!({}));
    let content = r["content"].as_array().expect("content");
    assert!(
        content
            .iter()
            .any(|item| item["type"] == "image" && item["mimeType"] == "image/png")
    );
    let r = c.tool("export.png", json!({ "path": "out/sprite.png" }));
    assert_eq!(r["isError"], false, "{r}");
    let file = ws.join("out").join("sprite.png");
    let png = import_png(&std::fs::read(&file).expect("exported file")).expect("png");
    assert_eq!((png.width, png.height), (8, 8));
    let at = |x: usize, y: usize| &png.rgba[(y * 8 + x) * 4..(y * 8 + x) * 4 + 4];
    assert_eq!(at(2, 3), [255, 0, 0, 255]);
    assert_eq!(at(7, 7), [0, 255, 0, 255]);
    assert_eq!(at(0, 0), [0, 0, 0, 0]);
}

#[test]
fn image_document_open_edit_save_and_export_over_stdio() {
    let ws = temp_workspace("image");
    let mut c = Client::spawn(&[], &ws);
    c.initialize();
    let guide = c.request("resources/read", json!({ "uri": "pincel://guide" }));
    assert_eq!(guide["result"]["contents"][0]["mimeType"], "text/markdown");

    let r = c.tool(
        "doc.new",
        json!({ "mode": "image", "width": 40, "height": 30 }),
    );
    assert_eq!(r["isError"], false, "{r}");
    let r = c.tool(
        "draw.shape",
        json!({ "shape": "rect", "x0": 4, "y0": 4, "x1": 30, "y1": 20, "fill": "#3366ff" }),
    );
    assert_eq!(r["isError"], false, "{r}");
    let r = c.tool(
        "draw.text",
        json!({ "text": "Hi", "x": 6, "y": 6, "size": 12, "color": "#ffffff" }),
    );
    assert_eq!(r["isError"], false, "{r}");
    let r = c.tool("doc.save", json!({ "path": "mock.ora" }));
    assert_eq!(r["isError"], false, "{r}");
    let r = c.tool("export.webp", json!({ "path": "out/mock.webp" }));
    assert_eq!(r["isError"], false, "{r}");
    let r = c.tool("export.png", json!({ "path": "out/mock.png" }));
    assert_eq!(r["isError"], false, "{r}");
    assert!(ws.join("mock.ora").is_file());
    assert!(ws.join("out").join("mock.webp").is_file());
    let png =
        import_png(&std::fs::read(ws.join("out").join("mock.png")).expect("png")).expect("decodes");
    assert_eq!((png.width, png.height), (40, 30));
    let at = |x: usize, y: usize| &png.rgba[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
    assert_eq!(at(25, 18), [0x33, 0x66, 0xff, 255]);
    assert_eq!(at(0, 0), [0, 0, 0, 0]);

    // The PNG reopens as an Image document; saving it without a path asks
    // for a .ora path and leaves the PNG alone.
    let before = std::fs::read(ws.join("out").join("mock.png")).expect("png");
    let r = c.tool("doc.open", json!({ "path": "out/mock.png" }));
    assert_eq!(r["structuredContent"]["mode"], "image", "{r}");
    let r = c.tool("doc.save", json!({}));
    assert_eq!(r["isError"], true);
    assert_eq!(
        std::fs::read(ws.join("out").join("mock.png")).expect("png"),
        before
    );
}

#[test]
fn bad_messages_get_errors_and_serving_continues() {
    let ws = temp_workspace("errors");
    let mut c = Client::spawn(&[], &ws);
    c.initialize();
    let r = c.send_raw("this is not json");
    assert_eq!(r["error"]["code"], -32700);
    let r = c.request(
        "tools/call",
        json!({ "name": "nope.tool", "arguments": {} }),
    );
    assert!(
        r["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("nope.tool"))
    );
    let r = c.tool(
        "doc.new",
        json!({ "mode": "pixel", "width": 4, "height": 4, "depth": 8 }),
    );
    assert_eq!(r["isError"], true);
    assert!(
        r["content"][0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("depth"))
    );
    assert_eq!(c.request("ping", json!({}))["result"], json!({}));
}

#[test]
fn paths_resolve_in_cwd_without_workspace_and_escapes_fail() {
    let ws = temp_workspace("cwd");
    let mut c = Client::spawn(&[], &ws);
    c.initialize();
    c.tool(
        "doc.new",
        json!({ "mode": "pixel", "width": 2, "height": 2 }),
    );
    let r = c.tool("doc.save", json!({ "path": "a.aseprite" }));
    assert_eq!(r["isError"], false, "{r}");
    assert!(ws.join("a.aseprite").exists());
    for bad in ["../escape.png", "/tmp/escape.png"] {
        let r = c.tool("export.png", json!({ "path": bad }));
        assert_eq!(r["isError"], true);
        let text = r["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.contains("E007: path outside workspace"), "{text}");
    }
    assert!(!ws.parent().expect("parent").join("escape.png").exists());
    let r = c.tool("doc.open", json!({ "path": "../a.aseprite" }));
    assert!(
        r["content"][0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("E007"))
    );
    let r = c.tool("doc.open", json!({ "path": "a.aseprite" }));
    assert_eq!(r["isError"], false, "{r}");
}

#[test]
fn workspace_option_and_read_only_mode() {
    let root = temp_workspace("ro");
    let ws = root.join("ws");
    std::fs::create_dir_all(&ws).expect("mkdir");
    // Seed a document in the workspace with a writable server.
    {
        let mut c = Client::spawn(&["--workspace", ws.to_str().expect("utf8")], &root);
        c.initialize();
        c.tool(
            "doc.new",
            json!({ "mode": "pixel", "width": 2, "height": 2 }),
        );
        let r = c.tool("doc.save", json!({ "path": "seed.aseprite" }));
        assert_eq!(r["isError"], false, "{r}");
    }
    assert!(ws.join("seed.aseprite").exists());
    assert!(!root.join("seed.aseprite").exists());

    let mut c = Client::spawn(
        &["--workspace", ws.to_str().expect("utf8"), "--read-only"],
        &root,
    );
    c.initialize();
    let r = c.tool("doc.open", json!({ "path": "seed.aseprite" }));
    assert_eq!(r["isError"], false, "{r}");
    let r = c.tool(
        "draw.pixels",
        json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ffffff" }] }),
    );
    assert_eq!(r["isError"], false);
    for (name, args) in [
        ("doc.save", json!({})),
        ("export.png", json!({ "path": "x.png" })),
        (
            "export.sheet",
            json!({ "columns": 1, "path": "s.png", "json": "s.json" }),
        ),
    ] {
        let r = c.tool(name, args);
        assert_eq!(r["isError"], true, "{name}");
        let text = r["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.contains("read-only"), "{name}: {text}");
    }
    let names: Vec<String> = std::fs::read_dir(&ws)
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    assert_eq!(names, ["seed.aseprite"]);
}
