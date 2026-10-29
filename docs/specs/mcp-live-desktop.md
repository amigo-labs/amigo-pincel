---
status: todo
created: 2026-10-28
size: L
mode: both
design: [mcp.md, pincel.md §9.3, pincel.md §10.3, pincel.md §11, pincel.md §6]
depends_on: [mcp-headless-pixel, mcp-headless-image]
---

# MCP live control of the desktop app

## Context

With `mcp-headless-pixel` and `mcp-headless-image`, agents edit files headlessly. The second stage of the legacy plan `18-mcp` (removed 2026-10-29, see git history) (owner's top priority) lets an agent work on the document **open on screen** in the desktop app: the user watches every step, each tool call is one undo step, and saving stays under the user's control.

Documents live in the webview's wasm (pincel.md §9.3, ADR-001), not in the Tauri process. The Tauri backend therefore only forwards JSON to the mounted editor, which runs the same `pincel-agent` tool code against its document; no tool is implemented twice. A browser page cannot host a server, so live mode is desktop-only.

## Scope

### In scope

- A setting "Allow AI agents to control Pincel", off by default, persisted in the IndexedDB prefs
- An MCP endpoint (Streamable HTTP) on 127.0.0.1 with a random bearer token while the setting is on, and the client command / config to copy
- Forwarding tool calls to the mounted editor (Pixel or Image) and returning results
- The same tools and schemas as the headless server, acting on the open document; each call one undo step
- `doc.open`, `doc.new` and `doc.save` through the shell's existing flows; saving to a new path asks the user
- A status-bar indicator "AI connected" with a disconnect button
- English and German strings

### Out of scope

- Live mode in the PWA — because a browser page cannot host a server; browser proposals for exposing page tools to agents ("WebMCP") are a later option
- Hosted or remote MCP — because the endpoint is local-only by design
- File-system access beyond the shell's open / new / save flows — because saving stays under the user's control (plan)
- Generative image models — because the agent draws with Pincel's tools

## Acceptance criteria

- [ ] AC-01 (always): On a fresh install, the desktop app runs no MCP endpoint until the user turns on "Allow AI agents to control Pincel".
- [ ] AC-02 (event): When the user turns the setting on, then the app listens for MCP Streamable HTTP on 127.0.0.1 only, with a newly generated random bearer token, and the setting shows the exact client command or configuration (address, port and token) to copy.
- [ ] AC-03 (state): While the setting is on, it stays on across app restarts; while it is off, no endpoint listens.
- [ ] AC-04 (event): When the user turns the setting off, then the endpoint stops, connected clients are disconnected, and later requests are refused.
- [ ] AC-05 (error): If a request carries no bearer token or a wrong one, then it is refused with an authorization error and no tool runs.
- [ ] AC-06 (event): When a connected client calls `tools/list`, then it receives the same tool names and input schemas as the headless server.
- [ ] AC-07 (event): When a client calls a mutating tool, then the change appears on the document open on screen as after any editor command, and the editor's Undo reverts exactly that one call.
- [ ] AC-08 (event): When two tool calls are in flight at once, then each client receives the result of its own call.
- [ ] AC-09 (error): If a document tool is called while no document is open, then the call fails with `isError: true` saying that no document is open.
- [ ] AC-10 (event): When a client calls `doc.save` without `path` on a document that has a file, then the document is saved through the editor's normal Save; with a new `path`, the normal Save As dialog asks the user first.
- [ ] AC-11 (error): If the user cancels the Save As dialog opened by `doc.save`, then the call fails with `isError: true` saying the user cancelled, and nothing is written.
- [ ] AC-12 (state): While a client is connected, the status bar shows "AI connected" with a disconnect button; clicking it ends that client's connection and the indicator disappears.
- [ ] AC-13 (option): Where the app runs in a browser (PWA), the setting and the indicator are not shown.
- [ ] AC-14 (always): The setting, its help text, the indicator and the disconnect button are translated in English and German.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

**Routing.** (1) The Tauri command receives `tools/call`; (2) it emits an event to the webview with a request id; (3) the mounted editor calls `agentCall(json) -> json` on its wasm document (new export in `pincel-wasm` and `pincel-image-wasm`, both calling `pincel-agent`); (4) the result returns via `invoke`. The UI refreshes like after any command (`syncInfo` in Image mode; recompose in Pixel mode).

**Pieces.** `agentCall` in both wasm crates; the Tauri endpoint and routing in `src-tauri/`; the settings toggle (Help or View menu until a settings dialog exists), status indicator and token display in the shell; i18n keys in `ui/src/lib/i18n/en.ts` and `de.ts`. Prefs live in `ui/src/lib/idb/`.

**Optional bridge.** `pincel-mcp --attach` would act as a stdio bridge for stdio-only clients, reading port and token from a file in the app's config dir (see Open questions).

**Testing hints.** UI side: a Playwright test calls the webview's `agentCall` bridge directly and checks that the canvas updates and Undo reverts it (e2e may compare element screenshots; app code never reads canvas pixels, CLAUDE.md §9). Tauri side: a Rust test of request/response routing with a mocked webview (AC-05, AC-08). CI checks `pincel-tauri` in its own job (GTK/WebKit libs).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Client connects while the start screen is shown | `tools/list` works; document tools fail as in AC-09 |
| Tool call arrives during a pointer stroke by the user | Open question (see below) |
| User undoes an agent's change, then the agent calls `doc.redo` | Redo re-applies it, as the editor's Redo would |
| The user switches the setting off mid-call | The call's result may still arrive or fail; later requests are refused (AC-04) |
| Port already in use | Open question (see below) |
| Request from another host on the LAN | Not reachable; the endpoint binds 127.0.0.1 only |

## Open questions

- Decision Log entries not written yet: live mode off by default; localhost + bearer token; WebMCP as a later option for the PWA — confirm and write them with this spec.
- The endpoint needs an HTTP server in the Tauri process (Streamable HTTP). Which one — a new dependency (which crate?), or the HTTP transport of `rmcp` if `mcp-headless-pixel` chooses it?
- Is `pincel-mcp --attach` (stdio bridge) part of this spec or a follow-up?
- Where does the setting live until a settings dialog exists: Help menu or View menu?
- What do `doc.open` and `doc.new` do while a document with unsaved changes is open — the shell's usual unsaved-changes prompt, or refuse?
- What do `export.png` / `export.jpeg` / `export.webp` / `export.sheet` with a `path` do in live mode — go through a save dialog, or fail?
- What happens to a tool call while a modal dialog (e.g. an effect dialog) or a user stroke is in progress — queue, fail, or time out, and after how long?
- Does the disconnect button also rotate the bearer token so the client cannot reconnect at once, and is a new token generated on every app start?
- When does a Streamable-HTTP client count as "connected" for the indicator (session open until it ends or times out — which timeout)?
- What does the app show when the chosen port is in use?

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |

## Decisions

- The Tauri backend only forwards JSON; the tools run in the webview's wasm against the open document — documents live in the webview (§9.3), and no tool is re-implemented (plan, 2026-10-28).
- Live mode is desktop-only — a browser page cannot host a server (plan).
- The endpoint binds 127.0.0.1 only and requires a random bearer token — local agents only, no LAN exposure (plan; Decision Log entry pending, see Open questions).
- Saving to a new path always asks the user — saving stays under the user's control (plan).
