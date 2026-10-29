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

- A setting "Allow AI agents to control Pincel", off by default, persisted in the IndexedDB prefs, in a dialog opened from View ▸ AI agents… (next to Language and Touch Gestures) that also shows the copyable client command, the token and a "Regenerate token" button
- An MCP endpoint (Streamable HTTP) on 127.0.0.1 with a random bearer token while the setting is on, and the client command / config to copy
- Forwarding tool calls to the mounted editor (Pixel or Image) and returning results
- The same tools and schemas as the headless server, acting on the open document; each call one undo step
- `doc.open`, `doc.new` and `doc.save` through the shell's existing flows (unsaved-changes prompt, Save, Save As); saving to a new path asks the user
- Exports (`export.*`) go through the shell's export flow: a path opens the normal export / save dialog
- Tool calls during a user stroke wait for the stroke; calls during a modal dialog fail; every call times out after 30 s
- A status-bar indicator "AI connected" with a disconnect button
- English and German strings

### Out of scope

- Live mode in the PWA — because a browser page cannot host a server; browser proposals for exposing page tools to agents ("WebMCP") are a later option
- Hosted or remote MCP — because the endpoint is local-only by design
- File-system access beyond the shell's open / new / save flows — because saving stays under the user's control (plan)
- Generative image models — because the agent draws with Pincel's tools
- The `pincel-mcp --attach` stdio bridge (a stdio client for stdio-only MCP clients, reading port and token from a file in the app's config dir) — follow-up spec, because Claude Code speaks HTTP natively and the bridge needs a token-file contract of its own; v1 documents `claude mcp add --transport http pincel http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"`

## Acceptance criteria

- [ ] AC-01 (always): On a fresh install, the desktop app runs no MCP endpoint until the user turns on "Allow AI agents to control Pincel".
- [ ] AC-02 (event): When the user turns the setting on in the dialog opened from View ▸ AI agents…, then the app listens for MCP Streamable HTTP on 127.0.0.1 only, with a newly generated random 256-bit bearer token, and the dialog shows the token and the exact client command (`claude mcp add --transport http pincel http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"`, address, port and token filled in) to copy.
- [ ] AC-03 (state): While the setting is on, it stays on across app restarts and the endpoint keeps the same bearer token (persisted in the app config dir) and port; while it is off, no endpoint listens.
- [ ] AC-04 (event): When the user turns the setting off, then the endpoint stops, connected clients are disconnected, and later requests are refused.
- [ ] AC-05 (error): If a request carries no bearer token or a wrong one, then it is refused with an authorization error and no tool runs.
- [ ] AC-06 (event): When a connected client calls `tools/list`, then it receives the same tool names and input schemas as the headless server.
- [ ] AC-07 (event): When a client calls a mutating tool, then the change appears on the document open on screen as after any editor command, and the editor's Undo reverts exactly that one call.
- [ ] AC-08 (event): When two tool calls are in flight at once, then each client receives the result of its own call.
- [ ] AC-09 (error): If a document tool is called while no document is open, then the call fails with `isError: true` saying that no document is open.
- [ ] AC-10 (event): When a client calls `doc.save` without `path` on a document that has a file, then the document is saved through the editor's normal Save; with a new `path`, the normal Save As dialog asks the user first.
- [ ] AC-11 (error): If the user cancels the Save As dialog opened by `doc.save`, then the call fails with `isError: true` saying the user cancelled, and nothing is written.
- [ ] AC-12 (state): While at least one client session is open, the status bar shows "AI connected" with a disconnect button; clicking it ends the open sessions and the indicator disappears. A session opens with a successful `initialize` (session id issued) and ends when the client sends DELETE for it, the user disconnects, or 10 minutes pass without a request in that session.
- [ ] AC-13 (option): Where the app runs in a browser (PWA), the setting and the indicator are not shown.
- [ ] AC-14 (always): The setting, its help text, the indicator and the disconnect button are translated in English and German.
- [ ] AC-15 (event): When a client calls `doc.open` or `doc.new` while the open document has unsaved changes, then the shell's usual unsaved-changes prompt is shown; if the user cancels it, the call fails with `isError: true` and "user cancelled", and the open document is unchanged.
- [ ] AC-16 (event): When a client calls `export.png`, `export.jpeg`, `export.webp` or `export.sheet` with a `path`, then the normal export / save dialog opens with the file name from `path` filled in, and the file is written only where the user confirms; if the user cancels, the call fails with `isError: true` and "user cancelled", and nothing is written.
- [ ] AC-17 (state): While the user is drawing a stroke (pointer down), an incoming tool call waits and runs after pointerup, and its result is returned then.
- [ ] AC-18 (error): If a tool call arrives while a modal dialog is open, then it fails at once with `isError: true` and "editor busy: a dialog is open", and the document is unchanged.
- [ ] AC-19 (error): If a tool call has no result 30 s after it arrived, then it fails with `isError: true` saying it timed out.
- [ ] AC-20 (event): When the user clicks "Regenerate token" in the AI agents dialog, then a new random token replaces the old one, the dialog shows the new token and command, and requests with the old token are refused as in AC-05; disconnecting alone never changes the token.
- [ ] AC-21 (error): If the default port is in use when the endpoint starts, then the app binds a free port assigned by the OS, persists it for later starts, and the AI agents dialog shows "Port changed — update your client config" with the updated command.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

**Routing.** (1) The Tauri command receives `tools/call`; (2) it emits an event to the webview with a request id; (3) the mounted editor calls `agentCall(json) -> json` on its wasm document (new export in `pincel-wasm` and `pincel-image-wasm`, both calling `pincel-agent`); (4) the result returns via `invoke`. The UI refreshes like after any command (`syncInfo` in Image mode; recompose in Pixel mode).

**Pieces.** `agentCall` in both wasm crates; the Tauri endpoint and routing in `src-tauri/`; the View ▸ AI agents… dialog (toggle, copyable command, token, Regenerate token), status indicator and port-changed notice in the shell; i18n keys in `ui/src/lib/i18n/en.ts` and `de.ts`. The on/off pref lives in `ui/src/lib/idb/`; the token and the port live in the app config dir, so a future `--attach` bridge can read them.

**HTTP server.** `hyper` 1.x (features `server`, `http1`) plus `hyper-util` 0.1 (`tokio`), on Tauri's tokio runtime — both already in `Cargo.lock` through the updater plugin, so they become direct dependencies of `pincel-tauri` without adding a crate to the tree (MIT). One `/mcp` route answers with JSON responses (no SSE stream) and sessions by `Mcp-Session-Id`. The JSON-RPC dispatch is the in-house loop of `mcp-headless-pixel`, reused over HTTP.

**Port.** Default port 39217 (fixed, unassigned by IANA); if it is taken, an OS-assigned port is bound and persisted (AC-21).

**Concurrency.** Calls arriving during a user stroke are queued until pointerup (AC-17); during a modal dialog they fail at once (AC-18); a 30 s timeout covers every call (AC-19).

**Testing hints.** UI side: a Playwright test calls the webview's `agentCall` bridge directly and checks that the canvas updates and Undo reverts it (e2e may compare element screenshots; app code never reads canvas pixels, CLAUDE.md §9). Tauri side: a Rust test of request/response routing with a mocked webview (AC-05, AC-08). CI checks `pincel-tauri` in its own job (GTK/WebKit libs).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Client connects while the start screen is shown | `tools/list` works; document tools fail as in AC-09 |
| Tool call arrives during a pointer stroke by the user | Waits and runs after pointerup (AC-17) |
| Tool call arrives while an effect dialog is open | Fails at once: "editor busy: a dialog is open" (AC-18) |
| `doc.new` while the open document is dirty and the user cancels the prompt | Fails with "user cancelled"; document unchanged (AC-15) |
| Client stops sending requests without DELETE | Its session ends after 10 minutes idle; the indicator hides when no session is left (AC-12) |
| App restarts with the setting on | Same token and port; the client config keeps working (AC-03) |
| User undoes an agent's change, then the agent calls `doc.redo` | Redo re-applies it, as the editor's Redo would |
| The user switches the setting off mid-call | The call's result may still arrive or fail; later requests are refused (AC-04) |
| Port already in use | An OS-assigned port is bound and persisted; the dialog shows "Port changed — update your client config" (AC-21) |
| Request from another host on the LAN | Not reachable; the endpoint binds 127.0.0.1 only |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

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
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |

## Decisions

- The Tauri backend only forwards JSON; the tools run in the webview's wasm against the open document — documents live in the webview (§9.3), and no tool is re-implemented (plan, 2026-10-28).
- Live mode is desktop-only — a browser page cannot host a server (plan).
- The endpoint binds 127.0.0.1 only and requires a random bearer token — local agents only, no LAN exposure (plan; Decision Log entry 2026-10-29).
- Saving to a new path always asks the user — saving stays under the user's control (plan).
- 2026-10-29: Live MCP is desktop-only and off by default; Streamable HTTP on 127.0.0.1 with a random 256-bit bearer token; tools run in the webview's wasm via `agentCall`; WebMCP is the later option for the PWA — the Decisions here already stated all three, only the Decision Log row was missing; §15 row recorded with the 2026-10-29 Decision Log batch (owner accepted the recommendation).
- 2026-10-29: The endpoint runs on `hyper` 1.x (`server`, `http1`) plus `hyper-util` 0.1 (`tokio`) on Tauri's tokio runtime, one `/mcp` route with JSON responses and no SSE, reusing the in-house JSON-RPC dispatch — both crates are already in `Cargo.lock` through the updater plugin, so this adds a direct dependency but no new crate (MIT); dependency approved by the owner, §15 row recorded with the 2026-10-29 Decision Log batch (owner accepted the recommendation).
- 2026-10-29: `pincel-mcp --attach` is a follow-up spec; v1 documents `claude mcp add --transport http …` with the bearer header — Claude Code speaks HTTP natively, and the bridge only helps stdio-only clients and needs a token-file contract of its own (owner accepted the recommendation).
- 2026-10-29: The setting lives in View ▸ AI agents…, a small dialog with the toggle, the copyable command and the token (AC-02) — View is already the de-facto settings menu, and the token display needs a dialog, not a checkbox (owner accepted the recommendation).
- 2026-10-29: `doc.open` / `doc.new` with unsaved changes go through the shell's unsaved-changes prompt; a cancel fails the call with "user cancelled" (AC-15) — the user stays in control of their work and the existing flow is reused, as for `doc.save` (owner accepted the recommendation).
- 2026-10-29: `export.*` with a `path` opens the normal export / save dialog pre-filled with the file name; a cancel fails with "user cancelled", and nothing is written without the dialog (AC-16) — the same rule as `doc.save` to a new path: writing files stays under the user's control (owner accepted the recommendation).
- 2026-10-29: Tool calls during a user stroke are queued until pointerup; during a modal dialog they fail at once with "editor busy: a dialog is open"; every call times out after 30 s (AC-17 – AC-19) — strokes are short and safe to wait for, dialogs can stay open indefinitely and the agent can retry (owner accepted the recommendation).
- 2026-10-29: The token is generated when the setting is turned on and persisted across restarts in the app config dir; disconnect only ends the session, and a separate "Regenerate token" button rotates it (AC-03, AC-20) — a token that rotates on every start would break the user's client config at each launch (owner accepted the recommendation).
- 2026-10-29: A client counts as connected from a successful `initialize` (session id issued) until DELETE, user disconnect or 10 minutes without a request; the indicator shows while at least one session is open (AC-12) — Streamable HTTP defines sessions by `Mcp-Session-Id`, and the idle timeout clears clients that died (owner accepted the recommendation).
- 2026-10-29: The endpoint uses a fixed default port (39217); if it is taken, an OS-assigned port is bound, persisted and announced in the AI agents dialog with the updated command (AC-21) — the endpoint keeps working and the user gets the exact fix (owner accepted the recommendation).
