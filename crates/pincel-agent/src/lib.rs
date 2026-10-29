//! Pincel agent tools: the platform-free tool layer behind Pincel's MCP
//! server.
//!
//! One registry of `area.verb` tools ([`list_tools`] / [`call`]) runs over
//! an in-memory [`Workspace`] of documents. The crate does no I/O: tools
//! that read or write files go through the host's [`FileHost`], which owns
//! the workspace sandbox. Hosts are the `pincel-mcp` binary (stdio) and,
//! later, the desktop app (live mode). See
//! `docs/specs/mcp-headless-pixel.md` and the Decision Log in
//! `docs/specs/pincel.md` §15 (2026-10-29).

mod args;
mod image;
mod pixel;
mod preview;
mod resources;
mod schema;
#[cfg(test)]
mod tests;
mod workspace;

use serde_json::Value;

pub use resources::{ResourceDef, list_resources, read_resource};
pub use workspace::Workspace;

/// The MCP description of one tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDef {
    /// Unique `area.verb` name, e.g. `draw.pixels`.
    pub name: &'static str,
    /// What the tool does, written for a model.
    pub description: &'static str,
    /// JSON Schema (`type: object`) of the tool's arguments.
    pub input_schema: Value,
}

/// The successful result of a tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    /// Structured result, returned to the client as JSON text.
    pub json: Value,
    /// A PNG image to return alongside the JSON (`doc.preview`).
    pub image_png: Option<Vec<u8>>,
}

impl ToolOutput {
    /// A JSON-only result.
    pub fn json(json: Value) -> Self {
        Self {
            json,
            image_png: None,
        }
    }
}

/// Why a tool call failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AgentError {
    /// No tool has this name. Hosts report it as a protocol error.
    #[error("unknown tool `{0}`")]
    UnknownTool(String),
    /// The arguments do not match the tool's input schema.
    #[error("invalid arguments for `{tool}`: {message}")]
    InvalidArgs {
        /// The tool that was called.
        tool: String,
        /// Which argument is wrong and how.
        message: String,
    },
    /// The tool ran and refused the request; the message says how to fix
    /// the call. Hosts report it as a tool result with `isError: true`.
    #[error("{0}")]
    Tool(String),
}

impl AgentError {
    pub(crate) fn tool(message: impl Into<String>) -> Self {
        Self::Tool(message.into())
    }
}

/// File access the host grants the tools. Paths are the workspace-relative
/// strings the client passed; the host resolves them inside its sandbox
/// and reports violations (`E007: path outside workspace`) or a read-only
/// workspace as an error message.
pub trait FileHost {
    /// Read the whole file at `path`.
    fn read(&mut self, path: &str) -> Result<Vec<u8>, String>;
    /// Create or replace the file at `path` with `bytes`.
    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<(), String>;
    /// Report whether [`FileHost::write`] to `path` would be refused (path
    /// outside the workspace, read-only workspace) without writing. Tools
    /// that write several files check every path before the first write.
    fn check_write(&mut self, path: &str) -> Result<(), String>;
}

/// Everything a tool handler may touch during one call.
pub(crate) struct Ctx<'a> {
    pub ws: &'a mut Workspace,
    pub host: &'a mut dyn FileHost,
}

/// A registered tool: its schema and its handler.
pub(crate) struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
    pub run: fn(&mut Ctx<'_>, &args::Args) -> Result<ToolOutput, AgentError>,
}

/// The shared and Pixel tools, then the Image-only tools.
fn all_tools() -> impl Iterator<Item = &'static Tool> {
    pixel::TOOLS.iter().chain(image::TOOLS)
}

/// Every tool, in a stable order.
pub fn list_tools() -> Vec<ToolDef> {
    all_tools()
        .map(|t| ToolDef {
            name: t.name,
            description: t.description,
            input_schema: (t.schema)(),
        })
        .collect()
}

/// Run the tool `name` with `args` (a JSON object; `null` counts as `{}`)
/// against `ws`. Arguments are validated against the tool's schema before
/// the tool runs; a failed call leaves every document unchanged.
pub fn call(
    ws: &mut Workspace,
    host: &mut dyn FileHost,
    name: &str,
    args: Value,
) -> Result<ToolOutput, AgentError> {
    let tool = all_tools()
        .find(|t| t.name == name)
        .ok_or_else(|| AgentError::UnknownTool(name.to_owned()))?;
    let args = if args.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        args
    };
    schema::validate(&(tool.schema)(), &args).map_err(|message| AgentError::InvalidArgs {
        tool: name.to_owned(),
        message,
    })?;
    let Value::Object(map) = args else {
        return Err(AgentError::InvalidArgs {
            tool: name.to_owned(),
            message: "arguments must be an object".to_owned(),
        });
    };
    let mut ctx = Ctx { ws, host };
    (tool.run)(&mut ctx, &args::Args::new(map))
}
