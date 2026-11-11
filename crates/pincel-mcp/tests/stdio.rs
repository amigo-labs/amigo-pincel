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

    // The PNG reopens as an Image document; saving it without a path
    // writes the PNG back (mcp-parity-export AC-14), with the same pixels.
    let r = c.tool("doc.open", json!({ "path": "out/mock.png" }));
    assert_eq!(r["structuredContent"]["mode"], "image", "{r}");
    let r = c.tool("doc.save", json!({}));
    assert_eq!(r["isError"], false, "{r}");
    let again =
        import_png(&std::fs::read(ws.join("out").join("mock.png")).expect("png")).expect("png");
    assert_eq!(again.rgba, png.rgba);
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
fn read_only_mode_refuses_palette_export_and_allows_palette_import() {
    let root = temp_workspace("ro-palette");
    std::fs::write(root.join("p.gpl"), "GIMP Palette\n255 0 0 Red\n").expect("seed");
    let mut c = Client::spawn(&["--read-only"], &root);
    c.initialize();
    c.tool(
        "doc.new",
        json!({ "mode": "pixel", "width": 2, "height": 2 }),
    );
    let r = c.tool("palette.import", json!({ "path": "p.gpl" }));
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["colors"], 1);
    let r = c.tool(
        "palette.export",
        json!({ "path": "out.gpl", "format": "gpl" }),
    );
    assert_eq!(r["isError"], true, "{r}");
    let text = r["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("read-only"), "{text}");
    assert!(!root.join("out.gpl").exists());
}

#[test]
fn export_selection_that_cannot_write_leaves_the_document_unchanged() {
    // mcp-parity-selection-transform-content AC-19: the same failure as
    // `export.png` with that path, and a cut changes nothing.
    let root = temp_workspace("cut");
    let outside = format!("pincel-mcp-it-cut-out-{}.png", std::process::id());
    let escaping = format!("../{outside}");
    for (flags, path, reason) in [
        (&[][..], escaping.as_str(), "E007"),
        (&["--read-only"][..], "out.png", "read-only"),
    ] {
        for mode in ["pixel", "image"] {
            let mut c = Client::spawn(flags, &root);
            c.initialize();
            c.tool("doc.new", json!({ "mode": mode, "width": 2, "height": 2 }));
            c.tool("draw.fill", json!({ "x": 0, "y": 0, "color": "#ffffff" }));
            let region = json!({ "region": { "x": 0, "y": 0, "width": 2, "height": 2 } });
            let pixels = c.tool("doc.read_pixels", region.clone())["structuredContent"].clone();
            assert!(pixels["rows"].is_array(), "{mode}: {pixels}");
            let depth = c.tool("doc.info", json!({}))["structuredContent"]["undo_depth"].clone();
            let png = c.tool("export.png", json!({ "path": path }));
            assert_eq!(png["isError"], true, "{mode} {path}: {png}");
            let png_text = png["content"][0]["text"].as_str().unwrap_or_default();
            assert!(png_text.contains(reason), "{mode} {path}: {png_text}");
            for cut in [false, true] {
                let r = c.tool("export.selection", json!({ "path": path, "cut": cut }));
                assert_eq!(r["isError"], true, "{mode} {path} cut {cut}: {r}");
                assert_eq!(
                    r["content"][0]["text"], png["content"][0]["text"],
                    "{mode} {path} cut {cut}: fails as export.png does"
                );
            }
            let info = c.tool("doc.info", json!({}));
            assert_eq!(
                info["structuredContent"]["undo_depth"], depth,
                "{mode} {path}"
            );
            assert_eq!(
                c.tool("doc.read_pixels", region)["structuredContent"],
                pixels,
                "{mode} {path}: the cut left the pixels"
            );
        }
    }
    assert!(!root.join("out.png").exists());
    let parent = root.parent().expect("temp dir");
    assert!(!parent.join(&outside).exists());
}

#[test]
fn file_tools_refuse_read_only_and_escaping_paths() {
    // mcp-parity-export AC-03, AC-04.
    let root = temp_workspace("export-ro");
    std::fs::write(
        root.join("p.png"),
        pincel_core::encode_png(&[0, 0, 0, 255], 1, 1).expect("png"),
    )
    .expect("seed");
    let mut c = Client::spawn(&["--read-only"], &root);
    c.initialize();
    c.tool(
        "doc.new",
        json!({ "mode": "pixel", "width": 2, "height": 2 }),
    );
    for (name, args) in [
        ("export.svg", json!({ "path": "a.svg" })),
        ("export.apng", json!({ "path": "a.png" })),
        ("export.gif", json!({ "path": "a.gif" })),
    ] {
        let r = c.tool(name, args);
        assert_eq!(r["isError"], true, "{name}");
        assert!(
            r["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("read-only"),
            "{name}: {r}"
        );
    }
    c.tool(
        "doc.new",
        json!({ "mode": "image", "width": 2, "height": 2 }),
    );
    for (name, args) in [
        ("export.bmp", json!({ "path": "a.bmp" })),
        ("export.tiff", json!({ "path": "a.tif" })),
        ("doc.save", json!({ "path": "a.jpg" })),
        ("doc.save", json!({ "path": "a.ora" })),
    ] {
        let r = c.tool(name, args);
        assert_eq!(r["isError"], true, "{name}");
        assert!(
            r["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("read-only"),
            "{name}: {r}"
        );
    }
    // Opening keeps working, `mode` included.
    let r = c.tool("doc.open", json!({ "path": "p.png", "mode": "pixel" }));
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(r["structuredContent"]["mode"], "pixel");
    let names: Vec<String> = std::fs::read_dir(&root)
        .expect("dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    assert_eq!(names, ["p.png"]);
    drop(c);

    let mut c = Client::spawn(&[], &root);
    c.initialize();
    c.tool(
        "doc.new",
        json!({ "mode": "image", "width": 2, "height": 2 }),
    );
    for (name, args) in [
        ("export.svg", json!({ "path": "../a.svg" })),
        ("export.bmp", json!({ "path": "../a.bmp" })),
        ("doc.save", json!({ "path": "../a.png" })),
        ("doc.open", json!({ "path": "../a.psd" })),
    ] {
        let r = c.tool(name, args);
        assert_eq!(r["isError"], true, "{name}");
        assert!(
            r["content"][0]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("E007"),
            "{name}: {r}"
        );
    }
}

/// The text of a tool result, asserting it is an error.
fn error_text(r: &Value, what: &str) -> String {
    assert_eq!(r["isError"], true, "{what}: {r}");
    r["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// Names of the files directly in `dir`, sorted.
fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("dir")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

const SAMPLE_PSD: &[u8] = include_bytes!("../../pincel-image-core/tests/fixtures/sample.psd");

#[test]
fn read_only_refuses_every_save_format_and_keeps_opening() {
    // mcp-parity-export AC-03: the same refusal as export.png, for every
    // format doc.save writes (also back to an opened file); doc.open of a
    // PSD and with `mode` keeps working.
    let root = temp_workspace("export-ro-all");
    let png = pincel_core::encode_png(&[9, 9, 9, 255], 1, 1).expect("png");
    std::fs::write(root.join("p.png"), &png).expect("seed");
    std::fs::write(root.join("art.psd"), SAMPLE_PSD).expect("seed");
    let refusal = "the workspace is read-only (--read-only); nothing was written";
    let mut c = Client::spawn(&["--read-only"], &root);
    c.initialize();
    c.tool(
        "doc.new",
        json!({ "mode": "pixel", "width": 2, "height": 2 }),
    );
    let png_refusal = error_text(
        &c.tool("export.png", json!({ "path": "x.png" })),
        "export.png",
    );
    assert!(png_refusal.contains(refusal), "{png_refusal}");
    for (name, args) in [
        ("export.svg", json!({ "path": "a.svg" })),
        ("export.apng", json!({ "path": "a.png" })),
        ("export.gif", json!({ "path": "a.gif" })),
        ("doc.save", json!({ "path": "a.aseprite" })),
    ] {
        let e = error_text(&c.tool(name, args.clone()), name);
        assert!(e.contains(refusal), "{name} {args}: {e}");
    }
    let r = c.tool("doc.open", json!({ "path": "p.png" }));
    assert_eq!(r["structuredContent"]["mode"], "image", "{r}");
    let e = error_text(&c.tool("doc.save", json!({})), "doc.save back to p.png");
    assert!(e.contains(refusal), "{e}");
    for path in [
        "a.png", "a.jpg", "a.jpeg", "a.webp", "a.bmp", "a.tif", "a.tiff", "a.ora",
    ] {
        let e = error_text(&c.tool("doc.save", json!({ "path": path })), path);
        assert!(e.contains(refusal), "{path}: {e}");
    }
    for (name, path) in [
        ("export.svg", "i.svg"),
        ("export.bmp", "i.bmp"),
        ("export.tiff", "i.tif"),
    ] {
        let e = error_text(&c.tool(name, json!({ "path": path })), name);
        assert!(e.contains(refusal), "{name}: {e}");
    }
    let r = c.tool("doc.open", json!({ "path": "art.psd" }));
    assert_eq!(r["isError"], false, "{r}");
    assert_eq!(
        r["structuredContent"]["notices"].as_array().map(Vec::len),
        Some(3)
    );
    let r = c.tool("doc.open", json!({ "path": "p.png", "mode": "pixel" }));
    assert_eq!(r["structuredContent"]["mode"], "pixel", "{r}");
    drop(c);
    assert_eq!(
        file_names(&root),
        ["art.psd", "p.png"],
        "nothing is written"
    );
    assert_eq!(std::fs::read(root.join("p.png")).expect("p.png"), png);
}

#[test]
fn every_path_argument_outside_the_workspace_is_refused() {
    // mcp-parity-export AC-04: E007 for each tool of the spec, nothing
    // written outside, and an existing file outside is never read.
    let root = temp_workspace("export-escape");
    let outside = root.parent().expect("parent").to_owned();
    let tag = format!("pincel-it-escape-{}", std::process::id());
    let png = pincel_core::encode_png(&[9, 9, 9, 255], 1, 1).expect("png");
    let readable = format!("{tag}-in.png");
    std::fs::write(outside.join(&readable), &png).expect("seed outside");
    let mut c = Client::spawn(&[], &root);
    c.initialize();
    c.tool(
        "doc.new",
        json!({ "mode": "pixel", "width": 2, "height": 2 }),
    );
    let mut written = Vec::new();
    for (name, ext) in [
        ("export.svg", "svg"),
        ("export.apng", "png"),
        ("export.gif", "gif"),
    ] {
        let path = format!("{tag}-{name}.{ext}");
        let e = error_text(&c.tool(name, json!({ "path": format!("../{path}") })), name);
        assert!(e.contains("E007: path outside workspace"), "{name}: {e}");
        written.push(path);
    }
    c.tool(
        "doc.new",
        json!({ "mode": "image", "width": 2, "height": 2 }),
    );
    for (name, ext) in [
        ("export.svg", "svg"),
        ("export.bmp", "bmp"),
        ("export.tiff", "tif"),
        ("doc.save", "png"),
        ("doc.save", "ora"),
    ] {
        let path = format!("{tag}-{name}-image.{ext}");
        let e = error_text(&c.tool(name, json!({ "path": format!("../{path}") })), name);
        assert!(e.contains("E007: path outside workspace"), "{name}: {e}");
        written.push(path);
    }
    let docs = c.tool("doc.list", json!({}))["structuredContent"].clone();
    assert_eq!(docs["documents"].as_array().map(Vec::len), Some(2));
    for args in [
        json!({ "path": format!("../{readable}") }),
        json!({ "path": format!("../{readable}"), "mode": "pixel" }),
        json!({ "path": format!("../{readable}"), "mode": "image" }),
    ] {
        let e = error_text(&c.tool("doc.open", args.clone()), "doc.open");
        assert!(e.contains("E007: path outside workspace"), "{args}: {e}");
    }
    assert_eq!(
        c.tool("doc.list", json!({}))["structuredContent"],
        docs,
        "nothing is read"
    );
    drop(c);
    for path in written {
        assert!(!outside.join(&path).exists(), "{path} was written");
    }
    assert!(file_names(&root).is_empty(), "nothing is written inside");
    let _ = std::fs::remove_file(outside.join(readable));
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

/// The families `font.list` returns, in order.
fn family_names(result: &Value) -> Vec<String> {
    let text = result["content"][0]["text"].as_str().expect("text result");
    let list: Value = serde_json::from_str(text).expect("JSON result");
    list["families"]
        .as_array()
        .expect("families")
        .iter()
        .map(|f| f["name"].as_str().expect("name").to_owned())
        .collect()
}

#[test]
fn font_list_reports_the_bundled_and_the_system_families() {
    // docs/specs/system-fonts-mcp.md AC-09: the running server lists
    // Liberation Sans plus every family fontdb finds on this machine,
    // without any user interaction.
    let ws = temp_workspace("fonts");
    let mut c = Client::spawn(&["--workspace", ws.to_str().expect("utf8 path")], &ws);
    c.initialize();
    let r = c.tool("font.list", json!({}));
    assert_eq!(r["isError"], false, "{r}");
    let got = family_names(&r);

    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut want: Vec<String> = db
        .faces()
        .filter(|f| !matches!(f.source, fontdb::Source::Binary(_)))
        .filter_map(|f| f.families.first().map(|(name, _)| name.clone()))
        .chain(std::iter::once("Liberation Sans".to_owned()))
        .collect();
    want.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    want.dedup();
    assert_eq!(got, want);

    // A listed system family draws (the bundled one where none is
    // installed, as in a bare container).
    let family = got
        .iter()
        .find(|f| *f != "Liberation Sans")
        .cloned()
        .unwrap_or_else(|| "Liberation Sans".to_owned());
    c.tool(
        "doc.new",
        json!({ "mode": "image", "width": 64, "height": 32 }),
    );
    let r = c.tool(
        "draw.text",
        json!({ "text": "Ag", "x": 2, "y": 2, "size": 20, "font": family }),
    );
    // A face fontdb lists may still be unusable (AC-08); then the error
    // names the family.
    if r["isError"] == true {
        let text = r["content"][0]["text"].as_str().unwrap_or_default();
        assert!(text.contains(&family), "{text}");
    }
}
