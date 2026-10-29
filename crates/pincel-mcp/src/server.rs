//! Transport-agnostic MCP dispatch: one JSON-RPC 2.0 message in, at most
//! one message out. The stdio loop in `main.rs` feeds it lines; the live
//! desktop host can feed it HTTP bodies (Decision Log 2026-10-29).

use pincel_agent::{AgentError, FileHost, Workspace, list_resources, list_tools, read_resource};
use serde_json::{Value, json};

use crate::base64;

/// The one MCP protocol revision this server speaks.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const RESOURCE_NOT_FOUND: i64 = -32002;

const INSTRUCTIONS: &str = "Pincel edits pixel-art documents in memory. Loop: doc.new or doc.open, then layer / frame / palette / draw tools, check the result with doc.preview (an image) or doc.read_pixels (exact colors), and write files with doc.save / export.png / export.sheet. Paths are relative to the workspace. Every mutating call is one doc.undo step.";

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

    /// Handle one raw message (a line of the stdio transport). Returns the
    /// response text, or `None` for a notification.
    pub fn handle_text(&mut self, text: &str) -> Option<String> {
        let response = match serde_json::from_str::<Value>(text) {
            Ok(msg) => self.handle(msg),
            Err(e) => Some(error(
                Value::Null,
                PARSE_ERROR,
                &format!("Parse error: {e}"),
            )),
        };
        response.map(|v| v.to_string())
    }

    /// Handle one parsed JSON-RPC message.
    pub fn handle(&mut self, msg: Value) -> Option<Value> {
        let Some(obj) = msg.as_object() else {
            return Some(error(
                Value::Null,
                INVALID_REQUEST,
                "Invalid Request: expected one JSON-RPC object (batches are not supported)",
            ));
        };
        let id = obj.get("id").cloned();
        let method = obj.get("method").and_then(Value::as_str);
        if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || method.is_none() {
            // A response or garbage: answer requests, ignore the rest.
            return id.map(|id| {
                error(
                    id,
                    INVALID_REQUEST,
                    "Invalid Request: needs \"jsonrpc\": \"2.0\" and a \"method\"",
                )
            });
        }
        let method = method.unwrap_or_default();
        let params = obj.get("params").cloned().unwrap_or(Value::Null);
        // Notifications (no id) get no response.
        let id = id?;
        Some(match self.dispatch(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => error(id, code, &message),
        })
    }

    fn dispatch(&mut self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "listChanged": false, "subscribe": false },
                },
                "serverInfo": { "name": "pincel-mcp", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => {
                let tools: Vec<Value> = list_tools()
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
            "tools/call" => self.call_tool(params),
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
                Ok(json!({
                    "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }],
                }))
            }
            other => Err((METHOD_NOT_FOUND, format!("Method not found: `{other}`"))),
        }
    }

    fn call_tool(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params.get("name").and_then(Value::as_str).ok_or((
            INVALID_PARAMS,
            "tools/call needs a string `name`".to_owned(),
        ))?;
        let args = params.get("arguments").cloned().unwrap_or(Value::Null);
        match pincel_agent::call(&mut self.ws, &mut self.host, name, args) {
            Ok(out) => {
                let mut content = vec![json!({ "type": "text", "text": out.json.to_string() })];
                if let Some(png) = &out.image_png {
                    content.push(json!({
                        "type": "image",
                        "data": base64::encode(png),
                        "mimeType": "image/png",
                    }));
                }
                Ok(json!({ "content": content, "structuredContent": out.json, "isError": false }))
            }
            Err(AgentError::UnknownTool(t)) => Err((
                INVALID_PARAMS,
                format!("Unknown tool: `{t}`; tools/list shows the tools"),
            )),
            Err(e) => Ok(json!({
                "content": [{ "type": "text", "text": e.to_string() }],
                "isError": true,
            })),
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
        assert_eq!(uris.len(), 5);
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
