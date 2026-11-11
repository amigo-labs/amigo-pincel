//! Transport-agnostic MCP dispatch: one JSON-RPC 2.0 message in, at most
//! one message out. The stdio loop in `main.rs` feeds it lines; the live
//! desktop host routes HTTP bodies through [`route`] and runs the tool calls
//! in the editor (docs/specs/mcp-live-desktop.md, Decision Log 2026-10-29).

use pincel_agent::{
    AgentError, FileHost, FontProvider, ToolDef, ToolOutput, Workspace, list_resources, list_tools,
    read_resource,
};
use serde_json::{Value, json};

use crate::base64;

/// The one MCP protocol revision this server speaks.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const RESOURCE_NOT_FOUND: i64 = -32002;

const INSTRUCTIONS: &str = "Pincel edits documents in memory, in two modes that follow the file: Pixel documents (pixel art: .aseprite, frames, palette) and Image documents (photos, screenshots, mockups: PNG / JPEG / WebP / BMP / GIF / TIFF / .ora, brushes, shapes, text, selections). Read pincel://guide first. Loop: doc.new or doc.open, then layer and draw tools, check the result with doc.preview (an image) or doc.read_pixels (exact colors), and write files with doc.save / export.*. Paths are relative to the workspace. Every mutating call is one doc.undo step.";

/// How a server introduces itself in `initialize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerInfo {
    /// `serverInfo.name`.
    pub name: &'static str,
    /// The `instructions` for the model.
    pub instructions: &'static str,
}

/// The headless `pincel-mcp` server.
pub const HEADLESS: ServerInfo = ServerInfo {
    name: "pincel-mcp",
    instructions: INSTRUCTIONS,
};

/// What one message asks of the host.
#[derive(Debug, Clone, PartialEq)]
pub enum Routed {
    /// Send this response; `None` for a notification or a stray response.
    Reply(Option<Value>),
    /// Run the tool `name` with `args` and answer request `id` with
    /// [`tool_response`], [`live_response`] or [`tool_error`].
    ToolCall {
        /// The request id.
        id: Value,
        /// The tool name.
        name: String,
        /// The arguments (`null` when the call had none).
        args: Value,
    },
}

/// Route one raw message: answer everything but `tools/call`, which goes
/// to the host. `tools` lists the tools `tools/list` reports.
pub fn route_text(info: &ServerInfo, text: &str, tools: impl FnOnce() -> Vec<ToolDef>) -> Routed {
    match serde_json::from_str::<Value>(text) {
        Ok(msg) => route(info, msg, tools),
        Err(e) => Routed::Reply(Some(error(
            Value::Null,
            PARSE_ERROR,
            &format!("Parse error: {e}"),
        ))),
    }
}

/// Route one parsed message, as [`route_text`].
pub fn route(info: &ServerInfo, msg: Value, tools: impl FnOnce() -> Vec<ToolDef>) -> Routed {
    let Some(obj) = msg.as_object() else {
        return Routed::Reply(Some(error(
            Value::Null,
            INVALID_REQUEST,
            "Invalid Request: expected one JSON-RPC object (batches are not supported)",
        )));
    };
    let id = obj.get("id").cloned();
    let method = obj.get("method").and_then(Value::as_str);
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || method.is_none() {
        // A response or garbage: answer requests, ignore the rest.
        return Routed::Reply(id.map(|id| {
            error(
                id,
                INVALID_REQUEST,
                "Invalid Request: needs \"jsonrpc\": \"2.0\" and a \"method\"",
            )
        }));
    }
    let method = method.unwrap_or_default();
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    // Notifications (no id) get no response.
    let Some(id) = id else {
        return Routed::Reply(None);
    };
    if method == "tools/call" {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Routed::Reply(Some(error(
                id,
                INVALID_PARAMS,
                "tools/call needs a string `name`",
            )));
        };
        return Routed::ToolCall {
            id,
            name: name.to_owned(),
            args: params.get("arguments").cloned().unwrap_or(Value::Null),
        };
    }
    Routed::Reply(Some(match dispatch(info, method, &params, tools) {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error(id, code, &message),
    }))
}

fn dispatch(
    info: &ServerInfo,
    method: &str,
    params: &Value,
    tools: impl FnOnce() -> Vec<ToolDef>,
) -> Result<Value, (i64, String)> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "listChanged": false, "subscribe": false },
            },
            "serverInfo": { "name": info.name, "version": env!("CARGO_PKG_VERSION") },
            "instructions": info.instructions,
        })),
        "ping" => Ok(json!({})),
        "tools/list" => {
            let tools: Vec<Value> = tools()
                .into_iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })
                })
                .collect();
            Ok(json!({ "tools": tools }))
        }
        "resources/list" => {
            let resources: Vec<Value> = list_resources()
                .into_iter()
                .map(|r| {
                    json!({
                        "uri": r.uri,
                        "name": r.name,
                        "description": r.description,
                        "mimeType": r.mime_type,
                    })
                })
                .collect();
            Ok(json!({ "resources": resources }))
        }
        "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
        "resources/read" => {
            let uri = params.get("uri").and_then(Value::as_str).ok_or((
                INVALID_PARAMS,
                "resources/read needs a string `uri`".to_owned(),
            ))?;
            let text = read_resource(uri)
                .map_err(|e| (RESOURCE_NOT_FOUND, format!("Resource not found: {e}")))?;
            let mime = list_resources()
                .into_iter()
                .find(|r| r.uri == uri)
                .map_or("application/json", |r| r.mime_type);
            Ok(json!({
                "contents": [{ "uri": uri, "mimeType": mime, "text": text }],
            }))
        }
        other => Err((METHOD_NOT_FOUND, format!("Method not found: `{other}`"))),
    }
}

/// The successful tool result for `json` and an optional base64 PNG.
fn tool_success(json: &Value, png_base64: Option<String>) -> Value {
    let mut content = vec![json!({ "type": "text", "text": json.to_string() })];
    if let Some(data) = png_base64 {
        content.push(json!({ "type": "image", "data": data, "mimeType": "image/png" }));
    }
    json!({ "content": content, "structuredContent": json, "isError": false })
}

/// The response to request `id` for a tool call that ran here.
pub fn tool_response(id: Value, result: Result<ToolOutput, AgentError>) -> Value {
    match result {
        Ok(out) => {
            let png = out.image_png.as_deref().map(base64::encode);
            json!({ "jsonrpc": "2.0", "id": id, "result": tool_success(&out.json, png) })
        }
        Err(AgentError::UnknownTool(t)) => error(
            id,
            INVALID_PARAMS,
            &format!("Unknown tool: `{t}`; tools/list shows the tools"),
        ),
        Err(e) => tool_error(id, &e.to_string()),
    }
}

/// The response to request `id` for a tool call the desktop editor ran,
/// from its reply `wire` (`pincel_agent::live::LiveReply::to_json`; a
/// reply the app makes itself, such as "user cancelled", has the same
/// shape).
pub fn live_response(id: Value, wire: &Value) -> Value {
    if wire["ok"] == Value::Bool(true) {
        let png = wire["png"].as_str().map(str::to_owned);
        return json!({ "jsonrpc": "2.0", "id": id, "result": tool_success(&wire["json"], png) });
    }
    let message = wire["error"]
        .as_str()
        .unwrap_or("the editor failed to run the tool");
    if wire["unknown"] == Value::Bool(true) {
        return error(id, INVALID_PARAMS, message);
    }
    tool_error(id, message)
}

/// The response to request `id` for a tool call that failed with
/// `message`: a tool result with `isError: true` the model can act on.
pub fn tool_error(id: Value, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": message }], "isError": true },
    })
}

/// An MCP server over one session of documents.
pub struct Server<H: FileHost> {
    ws: Workspace,
    host: H,
}

impl<H: FileHost> Server<H> {
    /// A server with an empty session whose file tools use `host`.
    pub fn new(host: H) -> Self {
        Self {
            ws: Workspace::new(),
            host,
        }
    }

    /// A server whose `draw.text` and `font.list` also offer the faces of
    /// `fonts` (docs/specs/system-fonts-mcp.md).
    pub fn with_fonts(host: H, fonts: impl FontProvider + 'static) -> Self {
        Self {
            ws: Workspace::with_fonts(fonts),
            host,
        }
    }

    /// Handle one raw message (a line of the stdio transport). Returns the
    /// response text, or `None` for a notification.
    pub fn handle_text(&mut self, text: &str) -> Option<String> {
        let routed = route_text(&HEADLESS, text, list_tools);
        self.run(routed).map(|v| v.to_string())
    }

    /// Handle one parsed JSON-RPC message.
    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        let routed = route(&HEADLESS, msg, list_tools);
        self.run(routed)
    }

    fn run(&mut self, routed: Routed) -> Option<Value> {
        match routed {
            Routed::Reply(reply) => reply,
            Routed::ToolCall { id, name, args } => Some(tool_response(
                id,
                pincel_agent::call(&mut self.ws, &mut self.host, &name, args),
            )),
        }
    }
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<String, Vec<u8>>);

    impl FileHost for Mem {
        fn read(&mut self, path: &str) -> Result<Vec<u8>, String> {
            self.0
                .get(path)
                .cloned()
                .ok_or_else(|| "missing".to_owned())
        }
        fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
            self.0.insert(path.to_owned(), bytes.to_vec());
            Ok(())
        }

        fn check_write(&mut self, _path: &str) -> Result<(), String> {
            Ok(())
        }
    }

    fn rpc(s: &mut Server<Mem>, id: i64, method: &str, params: Value) -> Value {
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let text = s.handle_text(&msg.to_string()).expect("response");
        serde_json::from_str(&text).expect("json")
    }

    #[test]
    fn initialize_announces_name_version_and_capabilities() {
        let mut s = Server::new(Mem::default());
        let r = rpc(
            &mut s,
            1,
            "initialize",
            json!({ "protocolVersion": PROTOCOL_VERSION, "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } }),
        );
        let res = &r["result"];
        assert_eq!(res["serverInfo"]["name"], "pincel-mcp");
        assert_eq!(res["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(res["protocolVersion"], PROTOCOL_VERSION);
        assert!(res["capabilities"]["tools"].is_object());
        assert!(res["capabilities"]["resources"].is_object());
        assert!(
            s.handle_text(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
        assert_eq!(rpc(&mut s, 2, "ping", json!({}))["result"], json!({}));
        let tools = rpc(&mut s, 3, "tools/list", json!({}));
        assert!(
            tools["result"]["tools"]
                .as_array()
                .is_some_and(|t| t.len() >= 35)
        );
        let r = rpc(
            &mut s,
            4,
            "tools/call",
            json!({ "name": "doc.new", "arguments": { "mode": "pixel", "width": 4, "height": 4 } }),
        );
        assert_eq!(r["result"]["isError"], false);
        assert_eq!(r["result"]["structuredContent"]["doc"], "doc-1");
    }

    #[test]
    fn unknown_tool_and_bad_arguments_are_named_and_serving_continues() {
        let mut s = Server::new(Mem::default());
        let r = rpc(
            &mut s,
            1,
            "tools/call",
            json!({ "name": "draw.spray", "arguments": {} }),
        );
        assert_eq!(r["error"]["code"], INVALID_PARAMS);
        assert!(
            r["error"]["message"]
                .as_str()
                .is_some_and(|m| m.contains("draw.spray"))
        );
        let r = rpc(
            &mut s,
            2,
            "tools/call",
            json!({ "name": "doc.new", "arguments": { "mode": "pixel", "width": "big", "height": 4 } }),
        );
        assert_eq!(r["result"]["isError"], true);
        let text = r["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(text.contains("doc.new") && text.contains("width"), "{text}");
        assert_eq!(rpc(&mut s, 3, "ping", json!({}))["result"], json!({}));
    }

    #[test]
    fn invalid_json_gets_parse_error_and_serving_continues() {
        let mut s = Server::new(Mem::default());
        let r: Value =
            serde_json::from_str(&s.handle_text("{not json").expect("response")).expect("json");
        assert_eq!(r["error"]["code"], PARSE_ERROR);
        assert_eq!(r["id"], Value::Null);
        let r: Value =
            serde_json::from_str(&s.handle_text("[1, 2]").expect("response")).expect("json");
        assert_eq!(r["error"]["code"], INVALID_REQUEST);
        let r: Value = serde_json::from_str(
            &s.handle_text(r#"{"id":7,"method":"ping"}"#)
                .expect("response"),
        )
        .expect("json");
        assert_eq!(
            (r["id"].as_i64(), r["error"]["code"].as_i64()),
            (Some(7), Some(INVALID_REQUEST))
        );
        assert_eq!(rpc(&mut s, 8, "ping", json!({}))["result"], json!({}));
        assert_eq!(
            rpc(&mut s, 9, "nope", json!({}))["error"]["code"],
            METHOD_NOT_FOUND
        );
    }

    #[test]
    fn resources_list_and_read_palette_presets() {
        let mut s = Server::new(Mem::default());
        let list = rpc(&mut s, 1, "resources/list", json!({}));
        let uris: Vec<&str> = list["result"]["resources"]
            .as_array()
            .map(|a| a.iter().filter_map(|r| r["uri"].as_str()).collect())
            .unwrap_or_default();
        assert_eq!(uris.len(), 7);
        for id in ["db32", "db16", "pico8", "gameboy", "nes"] {
            assert!(
                uris.contains(&format!("pincel://palettes/{id}").as_str()),
                "{id}"
            );
        }
        let r = rpc(
            &mut s,
            2,
            "resources/read",
            json!({ "uri": "pincel://palettes/pico8" }),
        );
        let text = r["result"]["contents"][0]["text"].as_str().expect("text");
        let v: Value = serde_json::from_str(text).expect("json");
        assert_eq!(v["colors"].as_array().map(Vec::len), Some(16));
        assert_eq!(v["colors"][8], "#FF004DFF");
        let r = rpc(
            &mut s,
            3,
            "resources/read",
            json!({ "uri": "pincel://palettes/vga" }),
        );
        assert_eq!(r["error"]["code"], RESOURCE_NOT_FOUND);
    }

    #[test]
    fn resources_read_guide_and_effects_with_their_mime_types() {
        let mut s = Server::new(Mem::default());
        let list = rpc(&mut s, 1, "resources/list", json!({}));
        let listed = |uri: &str| {
            list["result"]["resources"]
                .as_array()
                .and_then(|a| a.iter().find(|r| r["uri"] == uri))
                .map(|r| r["mimeType"].clone())
                .expect(uri)
        };
        for (i, (uri, mime)) in [
            ("pincel://guide", "text/markdown"),
            ("pincel://effects", "application/json"),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(listed(uri), mime);
            let r = rpc(
                &mut s,
                i as i64 + 2,
                "resources/read",
                json!({ "uri": uri }),
            );
            let content = &r["result"]["contents"][0];
            assert_eq!(content["mimeType"], mime, "{uri}");
            let text = content["text"].as_str().expect("text");
            assert_eq!(text, pincel_agent::read_resource(uri).expect("read"));
        }
    }

    #[test]
    fn export_sheet_with_escaping_json_path_writes_nothing() {
        let dir = crate::test_dir("sheet-escape");
        let ws = dir.join("ws");
        std::fs::create_dir_all(&ws).expect("mkdir");
        let sandbox = crate::sandbox::Sandbox::new(&ws, false).expect("sandbox");
        let mut s = Server::new(sandbox);
        let call = |s: &mut Server<crate::sandbox::Sandbox>, id: i64, name: &str, args: Value| {
            let msg = json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": name, "arguments": args } });
            let text = s.handle_text(&msg.to_string()).expect("response");
            serde_json::from_str::<Value>(&text).expect("json")
        };
        call(
            &mut s,
            1,
            "doc.new",
            json!({ "mode": "pixel", "width": 2, "height": 2 }),
        );
        let r = call(
            &mut s,
            2,
            "export.sheet",
            json!({ "columns": 1, "path": "s.png", "json": "../s.json" }),
        );
        assert_eq!(r["result"]["isError"], true);
        let text = r["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        assert!(text.contains("E007"), "{text}");
        assert!(!ws.join("s.png").exists(), "the PNG must not be written");
        assert!(!dir.join("s.json").exists());
        let r = call(
            &mut s,
            3,
            "export.sheet",
            json!({ "columns": 1, "path": "../s.png", "json": "s.json" }),
        );
        assert_eq!(r["result"]["isError"], true);
        assert!(!ws.join("s.json").exists(), "the JSON must not be written");
    }

    #[test]
    fn preview_result_carries_png_image_content() {
        let mut s = Server::new(Mem::default());
        rpc(
            &mut s,
            1,
            "tools/call",
            json!({ "name": "doc.new", "arguments": { "mode": "pixel", "width": 2, "height": 2 } }),
        );
        let r = rpc(
            &mut s,
            2,
            "tools/call",
            json!({ "name": "doc.preview", "arguments": {} }),
        );
        let img = &r["result"]["content"][1];
        assert_eq!(img["type"], "image");
        assert_eq!(img["mimeType"], "image/png");
        let data = base64::decode(img["data"].as_str().expect("data")).expect("base64");
        assert_eq!(&data[..8], b"\x89PNG\r\n\x1a\n");
    }
}

#[cfg(test)]
mod route_tests {
    use super::*;

    fn msg(id: i64, method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    #[test]
    fn route_answers_protocol_methods_and_hands_tool_calls_to_the_host() {
        let info = ServerInfo {
            name: "pincel-live",
            instructions: "live",
        };
        let Routed::Reply(Some(r)) = route(&info, msg(1, "initialize", json!({})), list_tools)
        else {
            panic!("initialize is answered");
        };
        assert_eq!(r["result"]["serverInfo"]["name"], "pincel-live");
        assert_eq!(r["result"]["instructions"], "live");
        let Routed::Reply(Some(r)) = route(&info, msg(2, "tools/list", json!({})), || {
            list_tools()
                .into_iter()
                .filter(|t| t.name.starts_with("draw."))
                .collect()
        }) else {
            panic!("tools/list is answered");
        };
        let tools = r["result"]["tools"].as_array().expect("tools");
        assert!(!tools.is_empty());
        assert!(
            tools
                .iter()
                .all(|t| t["name"].as_str().is_some_and(|n| n.starts_with("draw.")))
        );
        match route(
            &info,
            msg(
                3,
                "tools/call",
                json!({ "name": "draw.rect", "arguments": { "x0": 1 } }),
            ),
            list_tools,
        ) {
            Routed::ToolCall { id, name, args } => {
                assert_eq!(id, json!(3));
                assert_eq!(name, "draw.rect");
                assert_eq!(args, json!({ "x0": 1 }));
            }
            Routed::Reply(r) => panic!("tools/call goes to the host: {r:?}"),
        }
        assert!(matches!(
            route(
                &info,
                json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
                list_tools
            ),
            Routed::Reply(None)
        ));
    }

    #[test]
    fn live_response_maps_the_wire_reply_to_a_tool_result() {
        let png = [0x89u8, b'P', b'N', b'G'];
        let wire = json!({ "ok": true, "json": { "doc": "tab-1" }, "png": base64::encode(&png), "files": [] });
        let r = live_response(json!(4), &wire);
        assert_eq!(r["id"], 4);
        assert_eq!(r["result"]["isError"], false);
        assert_eq!(r["result"]["structuredContent"]["doc"], "tab-1");
        assert_eq!(r["result"]["content"][1]["type"], "image");
        let r = live_response(
            json!(5),
            &json!({ "ok": false, "error": "no document is open" }),
        );
        assert_eq!(r["result"]["isError"], true);
        assert_eq!(r["result"]["content"][0]["text"], "no document is open");
        let r = live_response(
            json!(6),
            &json!({ "ok": false, "unknown": true, "error": "unknown tool `x`" }),
        );
        assert_eq!(r["error"]["code"], INVALID_PARAMS);
        let r = tool_error(json!(7), "editor busy: a dialog is open");
        assert_eq!(r["result"]["isError"], true);
    }
}
