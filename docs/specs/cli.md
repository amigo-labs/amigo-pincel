---
id: cli
title: CLI Conventions
status: draft
version: 0.1.0
owner: daniel
created: 2026-05-13
last_updated: 2026-05-13
related:
  - lessons
  - canvas
  - mcp
---

# CLI Conventions

## Overview

Pincel's tooling consists of several command-line binaries that share
conventions for argument parsing, output formatting, configuration, logging,
and exit codes. This spec defines those conventions so new tools feel
consistent and scripting against them is predictable.

## Binaries

| Binary | Purpose | Crate |
|---|---|---|
| `pincel` | open files, headless export, scripting entry point | `packages/app` (CLI mode) |
| `pincel-lessons` | lesson authoring, linting, building, recording | `packages/lessons-cli` |
| `pincel-mcp` | MCP server (stdio and HTTP) | `packages/lessons-mcp` |

All binaries are produced from the cargo workspace. Each lives in its own
crate but shares the `core` and `schema` crates.

## Argument Parsing

`clap` v4 with the derive macro. Every binary uses `#[derive(Parser)]` for
the top-level and `#[derive(Subcommand)]` for command groups.

### Subcommand Style

Verb-first, kebab-case:

```
pincel-lessons new foundation/my-lesson
pincel-lessons lint
pincel-lessons build --output dist/
pincel-lessons record reference.aseprite
```

Not noun-first (`pincel-lessons lesson new`). Subcommands compose with
nouns when needed (`pincel-lessons palette add endesga-32.toml`).

### Global Flags

Every binary supports these consistently:

| Flag | Short | Description |
|---|---|---|
| `--verbose` | `-v` | repeatable; `-v` = info, `-vv` = debug, `-vvv` = trace |
| `--quiet` | `-q` | suppress non-error output |
| `--format <fmt>` | `-f` | `human` (default), `json`, `toml` |
| `--no-color` | | force-disable ANSI colour |
| `--config <path>` | | override config file location |
| `--help` | `-h` | (clap default) |
| `--version` | `-V` | (clap default) |

`--format=json` is the contract for scripting: stable schema across patch
versions, breaking changes only on major.

### Path Arguments

Positional paths are always plain filesystem paths. Special prefixes
(`pincel://`, `lessons://`) are reserved for resource URIs and only
accepted where explicitly documented.

Glob patterns are not expanded by the CLI — the shell does that. Where a
binary wants to match multiple files, it accepts multiple positional
arguments.

### Default I/O

- Stdin is read when no path is given and the command supports streaming
- Stdout is the primary output channel for data
- Stderr is for logs, progress, and human-readable messages
- `-` as a path means stdin/stdout depending on context

This lets `pincel-lessons lint - < lesson.toml` and similar pipelines work.

## Output Formatting

### Human Format (default)

Terminal-aware: ANSI colour and Unicode when stdout is a TTY, plain ASCII
otherwise. Progress bars on stderr only, never stdout.

Status indicators:
- `✓` (green) success
- `✗` (red) error
- `⚠` (yellow) warning
- `→` (cyan) progress / note

Plain ASCII fallback: `[OK]`, `[ERR]`, `[WARN]`, `->`.

### JSON Format

Single newline-terminated JSON object per top-level result. Stable schema
versioned via a `schema_version` field on the root. For commands that
produce streams (e.g. lint over many files), one JSON object per line
(NDJSON), each independently parseable.

```json
{"schema_version": 1, "command": "lint", "file": "lesson.toml", "ok": true, "warnings": []}
{"schema_version": 1, "command": "lint", "file": "other.toml", "ok": false, "errors": [{"code": "E001", "message": "..."}]}
```

### TOML Format

Used where output is itself a config artifact (e.g. `pincel-lessons new`
dumping a template). Not for diagnostics.

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | success |
| `1` | generic failure |
| `2` | usage error (invalid args, missing required) |
| `3` | I/O error (file not found, permission denied) |
| `4` | parse error (invalid TOML, malformed input) |
| `5` | validation error (lint failed, schema mismatch) |
| `6` | network error (CDN unreachable, MCP transport) |
| `64`–`78` | sysexits.h codes for tools called from system contexts |

Documented per-binary in `--help` output where deviations exist.

## Logging

`tracing` crate for structured logging. `--verbose` flags map to filter
levels:

| Flag | Filter |
|---|---|
| default | `warn` |
| `-v` | `info` |
| `-vv` | `debug` |
| `-vvv` | `trace` |

Override via `RUST_LOG` environment variable per standard `tracing-subscriber`
syntax. CLI flag wins over env var.

Logs go to stderr, formatted per output mode:
- Human: coloured `LEVEL target: message`
- JSON: one log object per line, distinguishable from data output by a
  `"_log": true` discriminator

## Configuration

### File Location

Standard XDG paths:

| Platform | Path |
|---|---|
| Linux | `$XDG_CONFIG_HOME/pincel/config.toml`, fallback `~/.config/pincel/config.toml` |
| macOS | `~/Library/Application Support/pincel/config.toml` |
| Windows | `%APPDATA%\pincel\config.toml` |

Override with `--config <path>`.

### Schema

```toml
[general]
default_locale = "de"
default_palette = "endesga-32"

[lessons]
repo_path = "~/code/pincel"
cdn_url = "https://lessons.pincel.app"

[mcp]
default_transport = "stdio"
http_port = 7711

[author]
name = "Daniel"
github_handle = "daniel-rck"
```

Missing keys fall back to compiled defaults. Unknown keys are warnings, not
errors, to keep forward-compat tolerant.

### Environment Variables

`PINCEL_*` prefix. Maps to config keys with double-underscore section
separator:

| Variable | Maps to |
|---|---|
| `PINCEL_GENERAL__DEFAULT_LOCALE` | `general.default_locale` |
| `PINCEL_LESSONS__CDN_URL` | `lessons.cdn_url` |
| `PINCEL_LOG` | alias for `RUST_LOG` filter |

Precedence: CLI flag > env var > config file > compiled default.

## Errors

Errors carry codes for machine-readable handling:

```rust
pub enum CliError {
    Io { source: std::io::Error, code: ErrorCode },
    Parse { file: PathBuf, message: String, line: Option<u32>, code: ErrorCode },
    Validation { rule: String, message: String, code: ErrorCode },
    Network { url: String, source: reqwest::Error, code: ErrorCode },
    // ...
}
```

In `human` format, errors print as:

```
✗ E004: Parse error in lesson.toml (line 12)
  unknown validation function: count_pixel
  expected one of: count_colors_used, has_line_at_angle, ...
  → run `pincel-lessons list-validations` to see the registry
```

In `json` format, the same data is structured:

```json
{
  "schema_version": 1,
  "ok": false,
  "errors": [{
    "code": "E004",
    "category": "parse",
    "file": "lesson.toml",
    "line": 12,
    "message": "unknown validation function: count_pixel",
    "hint": "run `pincel-lessons list-validations` to see the registry"
  }]
}
```

Error codes follow letter+number: `E` (error), `W` (warning), `H` (hint).
Numbers are stable across versions once assigned. A central registry lives
in `packages/core/src/errors.rs`.

## Subcommand Conventions

### `new`

Scaffolds. Refuses to overwrite existing files without `--force`. Prints
the created paths and a suggested next command.

### `list`

Plural noun output. Default `human` format is a table; `json` is an array.
Filters via `--filter <key>=<value>`.

### `show`

Single noun. Default `human` is a labelled multi-line layout; `json` is one
object.

### `lint` / `check`

Reports without modifying. Exit code 0 on clean, 5 on findings. Always
prints a summary line at the end:

```
✓ 28 files, 0 errors, 2 warnings
```

### `build`

Produces artifacts in an output directory (`--output`, default `dist/`).
Idempotent; reproducible builds where inputs are deterministic.

### `serve` / `preview`

Long-running. Listens on a port (`--port`, default per-binary). Prints
the URL once ready. SIGINT/SIGTERM trigger graceful shutdown.

## Shell Completions

Each binary supports `<binary> completions <shell>` to print completions
for `bash`, `zsh`, `fish`, `powershell`. Standard `clap_complete` output.

Installation instructions in `--help` epilogue.

## Versioning

`--version` prints:

```
pincel-lessons 0.4.2 (commit abc123, built 2026-05-13)
```

With `--format=json`:

```json
{
  "schema_version": 1,
  "name": "pincel-lessons",
  "version": "0.4.2",
  "commit": "abc123",
  "built": "2026-05-13T10:00:00Z",
  "core_version": "0.4.0"
}
```

The `--json` schema_version of each binary increments on breaking changes
to its machine-readable output, independent of the binary's SemVer.

## Testing

Each CLI crate has integration tests under `tests/` using `assert_cmd` and
`predicates`. Conventions:

- One file per subcommand
- Fixtures under `tests/fixtures/`
- `human` output asserts on substrings only (formatting is unstable)
- `json` output asserts on full structure via `serde_json::from_slice`

Snapshot tests via `insta` for help text and `--format=human` of common
flows.

## Open Questions

1. **Cross-binary plugins**: should subcommands be discoverable as separate
   binaries (`pincel-lessons-foo` → `pincel-lessons foo`)? Git-style plugin
   discovery. Probably not needed at v1.
2. **Progress bar library**: `indicatif` is the obvious choice but adds
   weight. Confirm before committing.
3. **`--format=yaml`?** Not currently needed; deferrable.
4. **i18n of CLI output**: error messages stay EN even for DE-locale users?
   Lean yes for now — CLI is dev-facing, EN is fine.

## References

- `lessons.md` — primary CLI consumer
- `canvas.md` — exit code categories partially overlap
- `mcp.md` — `pincel-mcp` binary
