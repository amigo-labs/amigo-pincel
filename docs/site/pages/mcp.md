`pincel-mcp` is a headless [Model Context Protocol](https://modelcontextprotocol.io) server. Claude Code, Claude Desktop or any other MCP client can create, draw, preview and export documents through it: Pixel documents (`.aseprite`, PNG, sprite sheets) and Image documents (PNG, JPEG, WebP, BMP, GIF, TIFF, OpenRaster `.ora`).

Documents live in memory. Files are read and written only inside the workspace folder you name.

## Install

Download `pincel-mcp-windows-x64.exe`, `pincel-mcp-macos-universal` or `pincel-mcp-linux-x64` from the [latest release](https://github.com/amigo-labs/amigo-pincel/releases/latest). Rename it to `pincel-mcp` (`pincel-mcp.exe` on Windows), make it executable (`chmod +x pincel-mcp`) and put it on your `PATH`.

Or build it from a checkout of the repository:

```bash
cargo install --path crates/pincel-mcp
```

## Register it

With Claude Code, using the current project as the workspace:

```bash
claude mcp add pincel -- pincel-mcp --workspace .
```

| Option | Meaning |
|--------|---------|
| `--workspace <dir>` | The folder files may be read from and written to. Default: the current folder. |
| `--read-only` | No saving and no exports (`doc.save`, `export.*`). |

## What the agent gets

- **Tools** named `area.verb`, for example `doc.new`, `draw.pixels`, `doc.preview`, `export.png`. `tools/list` describes each one.
- **Resources:** `pincel://guide` (how to use the tools, written for the model), `pincel://effects` (effect and adjustment parameters) and the built-in palettes at `pincel://palettes/{id}`.
