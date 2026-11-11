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
- Internal sessions for the in-app chat (`ai-chat-panel`): one token per chat, independent of the setting (revision 2026-11-06)
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
- Generative image models in this spec — because `comfyui-generate` adds them as desktop host tools (`gen.*`) on this endpoint
- The `pincel-mcp --attach` stdio bridge (a stdio client for stdio-only MCP clients, reading port and token from a file in the app's config dir) — follow-up spec, because Claude Code speaks HTTP natively and the bridge needs a token-file contract of its own; v1 documents `claude mcp add --transport http pincel http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"`

## Acceptance criteria

- [ ] AC-01 (always): On a fresh install, the desktop app runs no MCP endpoint until the user turns on "Allow AI agents to control Pincel" or sends the first message of a chat in the Assistant panel (AC-22).
- [ ] AC-02 (event): When the user turns the setting on in the dialog opened from View ▸ AI agents…, then the app listens for MCP Streamable HTTP on 127.0.0.1 only, with a newly generated random 256-bit bearer token, and the dialog shows the token and the exact client command (`claude mcp add --transport http pincel http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"`, address, port and token filled in) to copy.
- [ ] AC-03 (state): While the setting is on, it stays on across app restarts and the endpoint keeps the same bearer token (persisted in the app config dir) and port; while it is off and no chat holds a token (AC-22), no endpoint listens.
- [ ] AC-04 (event): When the user turns the setting off, then the endpoint stops, connected clients are disconnected, and later requests are refused.
- [ ] AC-05 (error): If a request carries no bearer token or a wrong one, then it is refused with an authorization error and no tool runs.
- [ ] AC-06 (event): When an external client calls `tools/list`, then it receives the same tool names and input schemas as the headless server, plus the desktop host tools other specs add (`gen.*`, `comfyui-generate`), listed whether or not those can run; a chat's internal session receives the list `ai-chat-panel` AC-17 and AC-26 define.
- [ ] AC-07 (event): When an external client calls a mutating tool, then the change appears on the document open on screen as after any editor command, and the editor's Undo reverts exactly that one call; a chat's calls form one step per answer instead (`ai-chat-panel` AC-08).
- [ ] AC-08 (event): When two tool calls are in flight at once, then each client receives the result of its own call.
- [ ] AC-09 (error): If a document tool is called while no document is open, then the call fails with `isError: true` saying that no document is open.
- [ ] AC-10 (event): When a client calls `doc.save` without `path` on a document that has a file, then the document is saved through the editor's normal Save; with a new `path`, the normal Save As dialog asks the user first.
- [ ] AC-11 (error): If the user cancels the Save As dialog opened by `doc.save`, then the call fails with `isError: true` saying the user cancelled, and nothing is written.
- [ ] AC-12 (state): While at least one external client session is open, the status bar shows "AI connected" with a disconnect button; clicking it ends the open sessions and the indicator disappears. A session opens with a successful `initialize` (session id issued) and ends when the client sends DELETE for it, the user disconnects, or 10 minutes pass without a request in that session.
- [ ] AC-13 (option): Where the app runs in a browser (PWA), the setting and the indicator are not shown.
- [ ] AC-14 (always): The setting, its help text, the indicator and the disconnect button are translated in English and German.
- [ ] AC-15 (event): When a client calls `doc.open` or `doc.new`, then the document opens in a new tab like the user's Open / New (no other tab is replaced); when it calls `doc.close` on a tab with unsaved changes, the usual unsaved-changes prompt is shown, and if the user cancels it, the call fails with `isError: true` and "user cancelled" and the tab stays open. A call without a `doc` argument acts on the active tab.
- [ ] AC-16 (event): When a client calls a tool that writes a file (every `export.*`, `palette.export`) with a `path`, then the normal export / save dialog opens with the file name from `path` filled in, and the file is written only where the user confirms; a tool that reads a file (`layer.import`, `palette.import`) opens the normal open dialog instead; if the user cancels, the call fails with `isError: true` and "user cancelled", and nothing is written or read.
- [ ] AC-17 (state): While the user is drawing a stroke (pointer down), an incoming tool call waits and runs after pointerup, and its result is returned then.
- [ ] AC-18 (error): If a tool call arrives while a modal dialog is open, then it fails at once with `isError: true` and "editor busy: a dialog is open", and the document is unchanged.
- [ ] AC-19 (error): If a tool call has no result 30 s after it arrived (5 minutes for `gen.*`), not counting time a dialog opened by that call waits for the user, then it fails with `isError: true` saying it timed out.
- [ ] AC-20 (event): When the user clicks "Regenerate token" in the AI agents dialog, then a new random token replaces the old one, the dialog shows the new token and command, and requests with the old token are refused as in AC-05; disconnecting alone never changes the token.
- [ ] AC-21 (error): If the default port is in use when the endpoint starts, then the app binds a free port assigned by the OS, persists it for later starts, and the AI agents dialog shows "Port changed — update your client config" with the updated command.
- [ ] AC-22 (event): When a chat in the Assistant panel sends its first message, then the endpoint (started if needed, on the same port rules) accepts a token issued for that chat, whether the setting is on or off; the token stops working when the chat ends (New chat, tab closed, CLI or model changed, app quit); while the setting is off, every other token is refused; turning the setting on or off does not affect chat tokens; chat sessions do not show the "AI connected" indicator and are not ended by its disconnect button.
- [ ] AC-23 (error): If an external client calls a tool while a chat answer runs, then the call fails at once with `isError: true` and "editor busy: the assistant is working", and the document is unchanged.

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
| `doc.close` on a dirty tab and the user cancels the prompt | Fails with "user cancelled"; the tab stays open (AC-15) |
| Client stops sending requests without DELETE | Its session ends after 10 minutes idle; the indicator hides when no session is left (AC-12) |
| App restarts with the setting on | Same token and port; the client config keeps working (AC-03) |
| User undoes an agent's change, then the agent calls `doc.redo` | Redo re-applies it, as the editor's Redo would |
| The user switches the setting off mid-call | The call's result may still arrive or fail; later requests are refused (AC-04) |
| Port already in use | An OS-assigned port is bound and persisted; the dialog shows "Port changed — update your client config" (AC-21) |
| Request from another host on the LAN | Not reachable; the endpoint binds 127.0.0.1 only |
| Setting off, chat running, external client tries the chat's port | Refused unless it presents the chat's token (AC-22) |
| A call times out while its work already runs in the editor | The work completes and stays a normal undo step (in a chat: part of the answer's step); the client got the timeout |
| App crashes while a chat holds a token | Tokens live in memory only; after a restart no old token works |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `src-tauri/src/mcp/tests.rs` `endpoint_listens_nowhere_until_enabled`; `ui/e2e/mcp-live-settings.spec.ts` "AC-01 a fresh install starts no endpoint" | built | |
| AC-02 | `src-tauri/src/mcp/tests.rs` `enable_listens_on_loopback_with_a_new_256_bit_token_and_the_client_command`; `src-tauri/src/mcp/secret.rs` `new_token_is_256_bit_hex_and_never_repeats`; `ui/e2e/mcp-live-settings.spec.ts` "AC-02 AC-03 AC-04 the switch …" | built | |
| AC-03 | `src-tauri/src/mcp/tests.rs` `endpoint_keeps_token_and_port_across_restarts`, `a_chat_that_cannot_store_the_port_leaves_nothing_listening`; `ui/e2e/mcp-live-settings.spec.ts` "AC-02 AC-03 AC-04 the switch …" | built | |
| AC-04 | `src-tauri/src/mcp/tests.rs` `disable_stops_the_endpoint_and_refuses_later_requests`; `ui/e2e/mcp-live-settings.spec.ts` "AC-02 AC-03 AC-04 the switch …" | built | |
| AC-05 | `src-tauri/src/mcp/tests.rs` `requests_without_or_with_a_wrong_token_are_refused_and_run_nothing` | built | |
| AC-06 | `src-tauri/src/mcp/tests.rs` `tools_list_matches_the_headless_server`, `a_chat_session_lists_and_runs_only_its_tools` | built | |
| AC-07 | `crates/pincel-agent/src/tests/live.rs` `call_pixel_draws_on_the_lent_document_as_one_undo_step`, `call_image_draws_on_the_lent_bus_as_one_undo_step`; `crates/pincel-wasm/src/agent.rs` `agent_call_draw_is_one_undo_step_and_queues_a_redraw`; `crates/pincel-image-wasm/src/agent.rs` `agent_call_stroke_is_one_undo_step_on_the_open_document`; `ui/e2e/mcp-live.spec.ts` "AC-07 a pixel / image tool call …" | built | |
| AC-08 | `src-tauri/src/mcp/tests.rs` `concurrent_calls_each_get_their_own_result` | built | |
| AC-09 | `ui/e2e/mcp-live.spec.ts` "AC-09 a document tool without an open document fails" | built | |
| AC-10 | `ui/e2e/mcp-live.spec.ts` "AC-10 AC-11 doc.save: …" | built | |
| AC-11 | `ui/e2e/mcp-live.spec.ts` "AC-10 AC-11 doc.save: …" | built | |
| AC-12 | `src-tauri/src/mcp/tests.rs` `sessions_open_on_initialize_and_end_on_delete_disconnect_or_idle`; `ui/e2e/mcp-live-settings.spec.ts` "AC-12 the status bar shows AI connected …" | built | |
| AC-13 | `ui/e2e/mcp-live-settings.spec.ts` "AC-13 the web app shows neither the setting nor the indicator" | built | |
| AC-14 | `ui/e2e/mcp-live-settings.spec.ts` "AC-14 the dialog and the indicator are German in the German UI" | built | |
| AC-15 | `ui/e2e/mcp-live.spec.ts` "AC-15 doc.new opens a new tab; doc.close of a dirty tab asks …", "AC-15 doc.open opens the picked file in a new tab and keeps the others" | built | |
| AC-16 | `ui/e2e/mcp-live.spec.ts` "AC-16 exports go through the save dialog …"; `crates/pincel-agent/src/tests/live.rs` `call_pixel_export_hands_the_file_to_the_host_instead_of_writing_it`, `call_pixel_reads_only_the_file_the_user_picked` | built | |
| AC-17 | `ui/e2e/mcp-live.spec.ts` "AC-17 a call during a stroke waits for pointerup" | built | |
| AC-18 | `ui/e2e/mcp-live.spec.ts` "AC-18 a call while a modal dialog is open fails at once …" | built | |
| AC-19 | `src-tauri/src/mcp/tests.rs` `a_call_without_answer_times_out_but_not_while_a_dialog_waits`, `generation_tools_get_five_minutes`; `ui/e2e/mcp-live.spec.ts` "AC-10 AC-11 doc.save: …" (the dialog pauses the clock) | built | |
| AC-20 | `src-tauri/src/mcp/tests.rs` `regenerate_token_refuses_the_old_one`, `switching_on_again_with_a_new_token_ends_the_old_sessions`, `sessions_open_on_initialize_and_end_on_delete_disconnect_or_idle` (disconnect keeps the token); `ui/e2e/mcp-live-settings.spec.ts` "AC-20 Regenerate token …" | built | |
| AC-21 | `src-tauri/src/mcp/tests.rs` `a_taken_default_port_binds_an_os_port_and_persists_it`, `an_unwritable_config_leaves_the_setting_off_and_nothing_listening`; `ui/e2e/mcp-live-settings.spec.ts` "AC-21 a changed port is announced …" | built | |
| AC-22 | `src-tauri/src/mcp/tests.rs` `a_chat_token_works_while_the_setting_is_off_and_ends_with_the_chat` | built | |
| AC-23 | `src-tauri/src/mcp/tests.rs` `an_external_call_during_a_chat_answer_is_refused_at_once` | built | |

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
- 2026-11-06: Revision for the in-app chat: internal sessions with a per-process token, independent of the setting (AC-22; AC-01 and AC-03 amended); `tools/list` adds desktop host tools such as `gen.*` (AC-06); `gen.*` calls time out after 5 minutes (AC-19); generative models leave this spec's out-of-scope list for `comfyui-generate` — the owner wants a chat and image generation in the app (Decision Log 2026-11-06, specs `ai-chat-panel`, `comfyui-generate`).
- 2026-11-06 (clarification pass): AC-15 follows the mixed document tabs (Decision Log 2026-11-04): opening never replaces a document, so the unsaved-changes prompt guards `doc.close` instead of `doc.open` / `doc.new`. Chat tokens are per chat, not per process, because one chat runs one CLI process per answer and keeps one MCP config; they live in memory only. Chat sessions do not show the indicator, which warns about control from outside the app, and the setting does not govern them. External calls during a chat answer are refused so the answer stays one undo step (owner). File-reading and file-writing tools added by the parity specs go through the open / save dialogs like `doc.save`; time spent in a dialog opened by the call does not count towards the timeout. `gen.*` gets 5 minutes because a first generation loads the model and takes minutes on mid-range GPUs.
- 2026-11-11 (spec-build): The endpoint is a small HTTP/1.1 server on `std::net` (one thread per connection, `Content-Length` and chunked bodies, keep-alive) instead of `hyper`: hyper 1.x needs tokio's `TcpListener` and `http-body-util` as further direct dependencies the approval does not cover, and one local route with JSON bodies needs no more; no dependency is added. Tokens and session ids come from the standard library's OS-seeded `RandomState` keys (a fresh thread per 64 bits) for the same reason. Requests with a foreign `Origin` are refused (DNS rebinding), `GET` answers 405 (no SSE stream).
- 2026-11-11 (spec-build): `pincel-tauri` depends on the workspace crates `pincel-agent` (the tool list) and `pincel-mcp` (its JSON-RPC routing, now `route` / `live_response` / `tool_error`), so `tools/list` is the headless server's by construction (AC-06).
- 2026-11-11 (spec-build): The chat side of AC-22 / AC-23 is the endpoint's commands `mcp_chat_start` / `mcp_chat_end` / `mcp_chat_answering` / `mcp_chat_image_access`; the Assistant panel that calls them comes with `ai-chat-panel`. Chat sessions get the list of `ai-chat-panel` AC-17 / AC-26 (hidden tools are refused too, "not available in the chat" / "image access is off").
- 2026-11-11 (spec-build): "Regenerate token" also ends the open external sessions — they cannot send another request with the old token anyway, and the indicator should not show clients that are cut off.
- 2026-11-11 (spec-build): A call that targets a tab makes it the active tab first, so the user sees every change; only `doc.list` reads the tabs in place. Calls run one after another in the webview; the dialog check of AC-18 happens on arrival and also counts the native file dialogs a call has open. Chat calls act on the active tab like external ones until `ai-chat-panel` binds them to the chat's tab.
- 2026-11-11 (spec-build): `doc.open` shows the open dialog starting at `path` (reading a file stays the user's choice, as for `layer.import`); `doc.save` with `path` runs the editor's Save As with all its dialogs (format, JPEG quality), so `flatten` / `quality` are left to those dialogs; `doc.new` with a `palette` other than db32 adds a `palette.load_preset` step; `doc.list` reports each tab's `doc.info` plus its name. A writing tool asks one save dialog per file (file name = the base name of its path), writes only after every dialog was confirmed, and adds `saved_to` to its result.
- 2026-11-11 (spec-build): With `pincel-agent` (and through it `pincel-image-core` / `pincel-core`) in both wasm modules, the release `.wasm` before wasm-opt grows from 1.7 to 5.2 MB (`pincel-wasm`) and from 4.1 to 6.1 MB (`pincel-image-wasm`); the dev builds pass 16 MiB, so the PWA precache limit is 32 MiB now. The PWA loads that code without using it — a candidate for a feature-gated build later.
- 2026-11-11 (spec-build): The e2e tests of AC-01 and AC-13 were green before the block was built: no endpoint and no setting is what the app did without this spec. Their proof that the new code keeps it so is the green run after the block; AC-01's Rust side (`endpoint_listens_nowhere_until_enabled`) went red first with the module missing.
- 2026-11-11 (spec-build): The setting is the IndexedDB pref `aiAgents`; at app start the UI asks the endpoint to start with the stored token (`mcp_enable` with `fresh: false`), so token and port come from `mcp.json` in the app config dir. The indicator sits in the editor frame's status bar, so it shows in both modes, and not on the start screen, which has no status bar.
- 2026-11-11 (spec-build): A Pixel tab in the background answers `doc.list` from its document in place instead of switching the editor to it. The e2e guard "doc.list reads background tabs without switching the shown document" was green against the previous code as well (the editor ended on a consistent tab in that run); it stays as a regression guard.
- 2026-11-11 (spec-build, verification round 1): The `doc.open` test of AC-15 was green at once — the code existed, only its test was missing. A chat whose port cannot be stored does not start and leaves nothing listening (red first: `a_chat_that_cannot_store_the_port_leaves_nothing_listening`).
