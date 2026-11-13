---
status: todo
created: 2026-11-06
size: L
mode: both
design: [pincel.md §9.3, pincel.md §11, pincel.md §13.7, pincel.md §15 (2026-11-06 PixelAssist)]
depends_on: [mcp-live-desktop]
---

# AI chat panel (PixelAssist)

## Context

The owner wants to chat with an agent that edits the open image for them ("PixelAssist"). Pincel already exposes every document-changing capability as an agent tool (`pincel-agent`, the `mcp-parity-*` specs), and `mcp-live-desktop` serves these tools on the document open on screen. What is missing is a chat inside the app.

Pincel itself contains no language model client, no API key and no account handling. Instead it starts an **agent CLI the user has installed** (this spec: Claude Code, `claude`; `ai-chat-clis` adds Codex CLI, Gemini CLI and opencode) as a child process. The CLI runs the model loop and reaches Pincel only through the live MCP endpoint, with a token issued for that chat (Decision Log 2026-11-06 "PixelAssist", `mcp-live-desktop` AC-22). Pincel streams the CLI's output into the chat and groups the agent's changes into undo steps.

Desktop only: the PWA cannot start processes.

## Scope

### In scope

- An "Assistant" panel in both editors (desktop app only): message list, input, Send / Stop, "New chat", CLI picker, model field, the "Agent may see the image" switch, and a menu entry to edit the system prompt
- One chat per document tab, kept until the tab closes
- Starting the selected CLI for each answer with only Pincel's tools and web search available, connected to the live endpoint with the chat's token (`mcp-live-desktop` AC-22)
- Streaming the agent's text and showing each tool call (name, result or error) as it happens
- One undo step per answered message
- An editor-context block in front of every user message
- The default system prompt (owner's PixelAssist template, adapted to Pincel's real tools) in English and German, editable and resettable
- The Claude Code adapter (`claude`)
- English and German strings

### Out of scope

- Codex CLI, Gemini CLI, opencode — because each needs its own adapter; spec `ai-chat-clis`
- A built-in LLM client, API keys or provider accounts in Pincel — because the CLI owns login and billing (Decision Log 2026-11-06)
- The chat in the PWA — because a browser page cannot start a process
- Keeping chats across app restarts — because a chat belongs to its tab's editing session, and old CLI sessions would point at documents that may have changed outside Pincel
- Opening, creating, closing documents and undo / redo from the chat — because a chat belongs to one tab and an answer is one undo step (AC-26)
- Object detection, segmentation and style transfer tools — because they need local vision models; backlog
- Image generation itself — because `comfyui-generate` adds it as `gen.*` tools, which the agent then sees like any other tool
- Attaching files or images to a chat message — because an attached file would bypass the image switch (AC-17) and the CLI's file restrictions (AC-16); a later spec if needed

## Acceptance criteria

- [ ] AC-01 (option): Where the app runs in a browser (PWA), the Assistant panel is not shown in either editor.
- [ ] AC-02 (always): In the desktop app, both editors list an "Assistant" panel; its collapsed state persists like every other panel's.
- [ ] AC-03 (state): While no supported CLI is installed, the panel shows "No supported AI command-line tool found" with the name and install page of each supported CLI, and the message input is disabled. A CLI counts as installed when the user's terminal would find it (the login shell's `PATH` on macOS / Linux; `PATH` including `.cmd` / `.exe` shims on Windows); the check repeats whenever the panel or the picker opens.
- [ ] AC-04 (always): The CLI picker lists the installed supported CLIs; the model field shows "Default" (the CLI's own default model) as placeholder; the last chosen CLI and, per CLI, the last model persist across restarts.
- [ ] AC-05 (event): When the user sends a message, then it appears in the chat, the selected CLI starts, and the agent's reply text appears progressively while it streams; Send is disabled while the input is empty or whitespace only.
- [ ] AC-06 (event): When the agent calls a tool (a Pincel tool or web search), then the chat shows a row with the tool name that turns into success or error when the call returns; expanding the row shows the arguments and the result text, an image result as "image (W×H)".
- [ ] AC-07 (event): When the agent changes the document while answering, then the change shows on the canvas as soon as the call returns.
- [ ] AC-08 (event): When an answer that changed the document ends (finished, stopped or failed), then the editor's Undo reverts every change of that answer in one step and Redo re-applies them in one step; the History panel names the step "Assistant".
- [ ] AC-09 (event): When an answer ends without changing the document, then no undo step is added.
- [ ] AC-10 (state): While an answer runs, the panel shows Stop instead of Send; drawing, document-changing menus and shortcuts, tab switching and closing, the Generate panel's Generate and Apply, "New chat", the CLI picker, the model field, the image switch and the system-prompt editor are inert; zoom, pan and scrolling the chat stay available.
- [ ] AC-11 (event): When the user clicks Stop, then the CLI process ends within 2 s, a running `gen.*` call is cancelled on the server without adding anything, any other tool call already running in the editor completes and counts towards the answer, open tool rows and the answer are marked "Stopped", and AC-08 applies once those calls are done.
- [ ] AC-12 (event): When the user sends a follow-up message in the same chat, then the CLI continues this chat's session, so the agent has the earlier messages as context.
- [ ] AC-13 (event): When the user clicks "New chat", then the message list empties, the chat's token ends and the next message starts a new session; the document and its undo history are unchanged.
- [ ] AC-14 (state): While several tabs are open, each tab has its own chat; switching tabs shows that tab's chat, and closing a tab ends its chat.
- [ ] AC-15 (always): Every user message the agent receives starts with an editor-context block stating the document name, mode, canvas size, the selection bounds (or "none"), up to 50 layer names with the active layer marked ("… and N more" beyond), and the names of the last five undo steps in the app's language; names are escaped so they cannot end the block early.
- [ ] AC-16 (always): Pincel starts the CLI so that its only tools are the chat's Pincel session and web search: no other MCP server from the user's configuration, no shell, no file reading or writing, no web fetch.
- [ ] AC-17 (state): While "Agent may see the image" is off, the tools that return pixel data (`doc.preview`, `doc.read_pixels`, `tile.read_pixels`) are not listed to the agent, and a call to one fails with "image access is off"; the switch is off for every new chat with a cloud model, and turning it on shows the notice "The image is sent to the model's provider".
- [ ] AC-18 (always): While the user has not edited the system prompt, the chat uses the default PixelAssist prompt in the app's language: the German text of the Technical notes, or its English translation.
- [ ] AC-19 (event): When the user saves an edited system prompt, then every chat uses it from its next answer on, in either language, also after a restart; Save is disabled while the text is empty; "Reset to default" restores the default of AC-18.
- [ ] AC-20 (error): If the CLI exits with an error or writes output Pincel cannot read, then the chat shows the CLI's error text and a hint for the recognised cases (not signed in: "Run `<CLI>` once in a terminal to sign in"; no network: "No internet connection"; unknown model: "<CLI> does not know the model <model>"), else "<CLI> stopped unexpectedly"; open tool rows are marked "Failed", and AC-08 applies.
- [ ] AC-21 (always): Agent text is shown as plain text with line breaks, inline code and code blocks in monospace; HTML or script in the agent's text is displayed, never executed.
- [ ] AC-22 (event): When the user closes the window or quits while an answer runs, then the answer is stopped as in AC-11 before the usual close flow, and no CLI process started by Pincel remains running; temporary chat folders left by a crashed run are removed at the next start.
- [ ] AC-23 (always): The panel, its messages, buttons, notices and the default prompt are translated in English and German.
- [ ] AC-24 (state, manual: needs a signed-in Claude Code and a real model): With Claude Code, on a new 64×64 Image document, "Draw a red 10×10 rectangle at the top left" draws it and one Undo removes it; after Redo, "Now make it blue" recolors it; "Run `dir` in a shell" is refused by the CLI.
- [ ] AC-25 (event): When the user changes the CLI or the model while the chat has messages, then the app asks "Start a new chat?"; confirming starts a new chat with the new choice, cancelling keeps the previous choice.
- [ ] AC-26 (always): The chat's agent is not offered `doc.open`, `doc.new`, `doc.close`, `doc.list`, `doc.undo` and `doc.redo`; a call to one fails with "not available in the chat"; every tool call of the chat acts on the chat's tab, and a `doc` argument naming another document fails the same way.
- [ ] AC-27 (error): If the CLI waits for interactive input (a permission prompt), then it gets none, and the answer fails as in AC-20 with "<CLI> asked for a permission Pincel does not grant".
- [ ] AC-28 (error): If the chat cannot start (the endpoint cannot bind a port, the temporary folder cannot be written), then the chat shows the reason and nothing is started.

## Technical notes

**Process model.** One CLI process per answer, stdin closed; follow-up messages continue the CLI's own session (`claude -p --resume <session-id>`), so Pincel keeps only the session id per chat. Pincel writes an MCP config with the internal session's URL and the chat's token into an empty per-chat temp directory, used as the working directory, and deletes it when the chat ends. Claude Code flags (as of 2026-11): `-p`, `--output-format stream-json --verbose`, `--mcp-config <file> --strict-mcp-config`, `--allowedTools "mcp__pincel__*,WebSearch"`, `--disallowedTools` for its built-in shell, file and fetch tools, `--append-system-prompt <prompt>`, `--model <model>`. Re-check the flags against `claude --help` when building. Unknown stream-json event types are skipped; lines that are not JSON count as unreadable output (AC-20).

**Finding CLIs.** macOS / Linux: the `PATH` printed by the user's login shell, because apps started from the Dock or a launcher do not inherit the terminal's `PATH`. Windows: `PATH` with `PATHEXT`, because npm installs CLIs as `.cmd` shims. Install page for Claude Code: `https://docs.claude.com/en/docs/claude-code`.

**Tests without a real model.** A fake CLI (a small script on a test `PATH`, named like the real CLI) records its arguments and config, emits recorded `stream-json` lines and calls the live endpoint itself; it covers every AC except AC-24 in Rust tests (adapter: arguments, config, stream parsing, process supervision) and Playwright against the mocked Tauri bridge (`ui/e2e/tauri-mock.ts`). AC-12 and AC-16 are proven by the arguments the fake CLI receives, plus AC-24 with the real CLI.

**Undo grouping.** The chat's session opens a group when an answer starts and closes it when the CLI exits; both cores fold the calls in between into one step (Pixel `CommandBatch`, Image undo stack). The History panel kind is `assistant` (Decision Log 2026-11-05 "History entries carry a kind").

**Editor-context block.** Prepended to the user's text, e.g.:

```text
<editor-context>
document: sky.png (Image), 1920×1080
selection: 400,120 300×200
layers: Background, *Clouds (active), Text
recent steps: Brush, Brush, Fill, Gaussian Blur, Assistant
</editor-context>
```

**Default system prompt (German).** The English default is a faithful translation shipped beside it. Adapted from the owner's PixelAssist template of 2026-11-06: invented tool names replaced by Pincel's tools, `{{TOOL_LIST}}` replaced by MCP's own tool listing, the per-message context moved into the editor-context block.

```markdown
Du bist „PixelAssist“, der Chat-Agent im Bildeditor Pincel.

## Rolle und Architektur
- Du bist das Sprach- und Planungsmodell. Du verstehst Chat-Anfragen, stellst Rückfragen und planst Bearbeitungsschritte.
- Du führst Bildoperationen nicht selbst aus und erzeugst keine Pixel. Alle Änderungen laufen lokal über die Tools des MCP-Servers `pincel`.
- Bildkontext holst du über `doc.info`, `layer.list` und `select.get`. `doc.preview` und `doc.read_pixels` stehen nur zur Verfügung, wenn der Nutzer dir erlaubt hat, das Bild zu sehen.
- Fordere keine Bild-Uploads an und sende keine Bilddaten an andere Dienste.
- Erfinde keine Tools. Nutze nur die Tools, die dir der Server `pincel` anbietet, mit ihren dokumentierten Parametern. Die Ressource `pincel://guide` erklärt Koordinaten, Farben und Arbeitsweise; `pincel://effects` listet die Effekte und Anpassungen.

## Editor-Kontext
Jede Nutzernachricht beginnt mit einem Block `<editor-context>`: Dokument, Modus, Größe, Auswahl, Ebenen (aktive mit *) und die letzten Schritte.

## Arbeitsweise
1. Verstehe die Anfrage.
2. Prüfe, ob Ziel, Region, Stil, Stärke oder Ausgabeformat fehlen. Frage nur nach, wenn die Änderung sonst wahrscheinlich falsch wird.
3. Erstelle einen kurzen internen Plan: welche Schritte, welche Tools, welche Parameter.
4. Rufe die passenden Tools mit gültigen Parametern auf. Bevorzuge das spezifischste Tool.
5. Prüfe das Ergebnis (`doc.info`, und `doc.preview`, wenn erlaubt). Bei Fehlern: erkläre kurz das Problem und schlage eine Alternative vor.
6. Fasse am Ende kurz zusammen, was geändert wurde und welche Tools liefen.

## Regeln für Bildänderungen
- Regionen: Es gibt keine automatische Objekterkennung. Nutze die Auswahl des Nutzers (`select.get`) oder – wenn du das Bild sehen darfst – bestimme die Region aus `doc.preview` und setze sie mit `select.rect`, `select.ellipse`, `select.polygon` oder `select.wand`. Ist die Region unklar, frage nach.
- „Entferne X“: Region auswählen, dann mit `gen.image` (Inpainting, der Prompt beschreibt den Hintergrund) füllen. Ist keine Bildgenerierung verfügbar, sag es offen.
- „Ändere Farbe/Helligkeit/Kontrast“: `adjust.apply`, bei Bedarf vorher eine Auswahl. Effekte (Weichzeichnen, Schärfen, Rauschen, …): `effect.apply`.
- „Zeichne X“: Formen, Linien, Text und Verläufe mit `draw.shape`, `draw.line`, `draw.rect`, `draw.ellipse`, `draw.text`, `draw.gradient`. „Generiere X“: `gen.image` mit Prompt, Negativ-Prompt, Seed und Stärke; die Position bestimmt die Auswahl.
- „Mach es wie Stil Y“: `gen.image` mit Stärke unter 1 und dem Stil im Prompt, oder passende Effekte. Einen eigenen Stil-Transfer gibt es nicht.
- Mehrere Schritte: plane eine sinnvolle Reihenfolge und führe sie nacheinander aus. Zerstörerische Schritte – `layer.remove`, `layer.flatten`, `layer.merge_*`, `draw.clear`, `select.delete`, `image.crop_to_selection`, `canvas.resize`, `export.*`, `doc.save` – erst nach Bestätigung.
- Alles, was du auf eine Nachricht hin änderst, macht der Nutzer mit einem Rückgängig-Schritt ungeschehen.
- Erfinde keine Parameter. Nutze Defaults nur, wenn sie im Tool-Schema dokumentiert sind.
- Fehlt ein Tool oder ist die Anfrage nicht umsetzbar, sag das offen und schlage die nächste mögliche Alternative vor.

## Antwortstil
- Antworte auf Deutsch, knapp und konkret.
- Keine langen technischen Erklärungen, außer der Nutzer fragt danach.
- Bei erfolgreicher Aktion: „Erledigt: …“ plus 1–3 Stichpunkte zu den Änderungen.
- Bei Rückfragen: höchstens 1–2 präzise Fragen.
- Bei Unsicherheit lieber nachfragen als raten.

## Ausgabeformat
- Tool-Aufrufe gemäß MCP-Schema.
- Nach Abschluss: kurze Statusmeldung an den Nutzer.
- Wenn du nicht handeln kannst: kurze Erklärung und ein konkreter Vorschlag.

## Beispiele
Nutzer: „Mach den Himmel blauer.“
Du: Wenn du das Bild sehen darfst: `doc.preview`, Himmel bestimmen, `select.wand` oder `select.polygon`, dann `adjust.apply` (Farbton/Sättigung). Sonst fragen, ob der Nutzer den Himmel auswählt. Antwort: „Erledigt: Himmel blauer und etwas satter.“

Nutzer: „Entferne die Person rechts.“
Du: Region aus der Auswahl oder aus `doc.preview` → `select.polygon` → `gen.image` (Inpainting, Prompt beschreibt den Hintergrund). Bei unklarer Region nachfragen.

Nutzer: „Zeichne links unten eine kleine Katze im Aquarellstil.“
Du: `select.rect` links unten → `gen.image` mit Prompt „kleine Katze, Aquarellstil“ und Seed. Ergebnis kurz bestätigen.

Nutzer: „Mach das ganze Bild heller.“
Du: `adjust.apply` (Helligkeit) auf die Ebene. Bei starker Änderung kurz bestätigen lassen.
```

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Agent calls `doc.save` with a new path | The normal Save As dialog asks (`mcp-live-desktop` AC-10); the call's timeout pauses while the dialog is open |
| CLI hangs without output | Stop ends it (AC-11); a whole answer has no timeout, because a long multi-step edit is legitimate and each tool call keeps its own timeout |
| User undoes the agent's step, then asks "do it again but stronger" | The editor-context block shows the undone state; the agent works from the current document |
| Two tabs, an answer runs in tab A | Tab switching is inert until the answer ends or is stopped (AC-10) |
| Agent reply contains `<img src=x onerror=…>` | Shown as text (AC-21) |
| Layer named `</editor-context>` | Escaped in the block (AC-15) |
| CLI installed while the app runs | Listed the next time the panel or picker opens (AC-03) |
| An external MCP client calls a tool during an answer | Refused with "editor busy: the assistant is working" (`mcp-live-desktop` AC-23) |
| External "Allow AI agents" setting is off | The chat still works through its own token (`mcp-live-desktop` AC-22) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The editor is inert for document changes while an answer runs | One undo step per answer cannot interleave the user's own edits; the effect dialog sets the precedent | Users wanting to keep drawing during long answers must wait or press Stop |
| One process per answer with the CLI's session resume, not a long-lived process | Stateless per answer, simpler crash handling; Claude Code resumes sessions (`--resume`); `ai-chat-clis` checks the same for each CLI it adds | A CLI that drops resume would lose context |
| Claude Code counts as a cloud model | It has no local-model mode | none |
| "Recent steps" are the last five undo-step names | The owner's template asked for "recently used tools"; undo names cover tools, menus and the agent alike; five show what just happened without crowding the block | The agent may misread them as its own actions |
| Stop ends the process within 2 s | Long enough for a graceful exit, short enough to feel immediate | A CLI ignoring the signal is killed |
| Up to 50 layer names in the context block | Covers almost every real document while keeping the block short; the agent can call `layer.list` for the rest | none |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/assistant-panel.spec.ts` "AC-01 the browser app shows no Assistant panel in either editor" | built  |        |
| AC-02 | `ui/e2e/assistant-panel.spec.ts` "AC-02 AC-03 AC-04 both editors list the panel; …" | built  |        |
| AC-03 | `ui/e2e/assistant-panel.spec.ts` "AC-02 AC-03 AC-04 …"; `src-tauri/src/assistant/tests.rs` `find_in_takes_pathext_shims_on_windows_and_executables_elsewhere` | built  |        |
| AC-04 | `ui/e2e/assistant-panel.spec.ts` "AC-02 AC-03 AC-04 …" | built  |        |
| AC-05 | `ui/e2e/assistant-panel.spec.ts` "AC-05 – AC-08 AC-12 AC-15 AC-21 a pixel / image answer streams, …"; `src-tauri/src/assistant/tests.rs` `stream_json_becomes_session_text_and_tool_events`, `an_answer_streams_events_records_its_inputs_and_finishes` | built  |        |
| AC-06 | `ui/e2e/assistant-panel.spec.ts` "AC-05 – AC-08 …"; `src-tauri/src/assistant/tests.rs` `stream_json_becomes_session_text_and_tool_events` | built  |        |
| AC-07 | `ui/e2e/assistant-panel.spec.ts` "AC-05 – AC-08 …" | built  |        |
| AC-08 | `ui/e2e/assistant-panel.spec.ts` "AC-05 – AC-08 …" (both modes); `crates/pincel-core/src/command/bus.rs` `end_group_folds_the_group_into_one_step_that_undoes_and_redoes_whole`, `end_group_of_one_step_names_it_and_survives_dropped_old_steps`; `crates/pincel-image-core/src/command/mod.rs` `end_group_folds_the_group_into_one_assistant_step`; `crates/pincel-wasm/src/agent.rs` / `crates/pincel-image-wasm/src/agent.rs` `assistant_answer_of_several_calls_is_one_assistant_step`; `ui/e2e/assistant-generate.spec.ts` "AC-08 a generation of the answer lands in its one Assistant step" | built  |        |
| AC-09 | `ui/e2e/assistant-panel.spec.ts` "AC-09 an answer that changes nothing adds no undo step"; `end_group_without_a_change_adds_no_step` in both cores | built  |        |
| AC-10 | `ui/e2e/assistant-chat.spec.ts` "AC-10 while an answer runs only zoom, pan and the chat stay usable"; `ui/e2e/assistant-generate.spec.ts` "AC-10 AC-11 the Generate panel is inert during an answer; Stop cancels its generation" | built  |        |
| AC-11 | `ui/e2e/assistant-panel.spec.ts` "AC-11 Stop ends the answer: …"; `src-tauri/src/assistant/tests.rs` `stop_ends_the_cli_within_two_seconds`; `ui/e2e/assistant-generate.spec.ts` "AC-10 AC-11 the Generate panel is inert …" (a running `gen.image` is cancelled, no layer, no step) | built  |        |
| AC-12 | `ui/e2e/assistant-panel.spec.ts` "AC-05 – AC-08 AC-12 …"; `src-tauri/src/assistant/tests.rs` `claude_args_allow_only_pincel_and_web_search_and_resume_the_session`, `an_answer_streams_events_records_its_inputs_and_finishes` | built  |        |
| AC-13 | `ui/e2e/assistant-chat.spec.ts` "AC-13 AC-14 each tab has its own chat; …" | built  |        |
| AC-14 | `ui/e2e/assistant-chat.spec.ts` "AC-13 AC-14 …" | built  |        |
| AC-15 | `ui/e2e/assistant-panel.spec.ts` "AC-05 – AC-08 AC-12 AC-15 …" (block in the sent prompt, recent steps); `ui/e2e/assistant-context.spec.ts` "the editor-context block states …", "the block lists 50 layers at most …" | built  |        |
| AC-16 | `src-tauri/src/assistant/tests.rs` `claude_args_allow_only_pincel_and_web_search_and_resume_the_session`, `an_answer_streams_events_records_its_inputs_and_finishes`; `src-tauri/src/mcp/tests.rs` `a_chat_session_lists_and_runs_only_its_tools` | built  |        |
| AC-17 | `ui/e2e/assistant-chat.spec.ts` "AC-17 the image switch is off for a new chat and warns when turned on"; `src-tauri/src/mcp/tests.rs` `a_chat_session_lists_and_runs_only_its_tools` | built  |        |
| AC-18 | `ui/e2e/assistant-chat.spec.ts` "AC-18 AC-19 AC-23 the system prompt: …"; `ui/e2e/assistant-context.spec.ts` "the default system prompt is …" | built  |        |
| AC-19 | `ui/e2e/assistant-chat.spec.ts` "AC-18 AC-19 AC-23 …" | built  |        |
| AC-20 | `ui/e2e/assistant-panel.spec.ts` "AC-20 AC-27 a failed answer shows the CLI error, …"; `src-tauri/src/assistant/tests.rs` `a_failing_cli_reports_its_error_text_and_hint`, `an_error_result_fails_with_the_cli_text`, `unreadable_output_fails_the_answer`, `errors_get_the_hint_of_their_cause` | built  |        |
| AC-21 | `ui/e2e/assistant-panel.spec.ts` "AC-05 – AC-08 AC-12 AC-15 AC-21 …"; `ui/e2e/assistant-context.spec.ts` "agent text splits into …" | built  |        |
| AC-22 | `ui/e2e/assistant-chat.spec.ts` "AC-22 closing the window during an answer stops it before the close flow"; `src-tauri/src/assistant/tests.rs` `purge_removes_folders_left_by_a_crashed_run` | built  |        |
| AC-23 | `ui/e2e/assistant-chat.spec.ts` "AC-18 AC-19 AC-23 …" (German panel and prompt); every key in all six dictionaries (`pnpm check`) | built  |        |
| AC-24 |      | open   |        |
| AC-25 | `ui/e2e/assistant-chat.spec.ts` "AC-25 changing the model of a chat with messages asks first"; `ui/e2e/assistant-panel.spec.ts` "AC-20 AC-27 …" (confirmed change) | built  |        |
| AC-26 | `ui/e2e/assistant-chat.spec.ts` "AC-26 the chat gets no document-level tools and acts on its own tab only"; `ui/e2e/assistant-generate.spec.ts` "AC-26 a chat gen.image runs on the chat tab and refuses another document"; `src-tauri/src/mcp/tests.rs` `a_chat_session_lists_and_runs_only_its_tools` | built  |        |
| AC-27 | `ui/e2e/assistant-panel.spec.ts` "AC-20 AC-27 …"; `src-tauri/src/assistant/tests.rs` `a_permission_prompt_fails_the_answer` | built  |        |
| AC-28 | `ui/e2e/assistant-panel.spec.ts` "AC-28 a chat that cannot start shows the reason and adds nothing"; `src-tauri/src/assistant/tests.rs` `a_chat_folder_that_cannot_be_written_starts_nothing`, `a_linked_chat_folder_is_refused_and_nothing_is_written_through_it`; `src-tauri/src/mcp/tests.rs` `a_chat_that_cannot_store_the_port_leaves_nothing_listening` | built  |        |

## Decisions

- 2026-11-06: The chat drives an installed agent CLI through Pincel's live MCP endpoint instead of a built-in LLM client — the CLI already handles the model loop, login and model choice, and the tools reach the agent through MCP unchanged (owner, Decision Log 2026-11-06).
- 2026-11-06: The agent may use Pincel's tools plus web search, nothing else — web search helps with references, while shell and file access could change the user's system outside the document (owner).
- 2026-11-06: One undo step per answered message — the user asked for one change and undoes it as one (owner).
- 2026-11-06: One chat per document tab, ended when the tab closes — the chat is about that document (owner).
- 2026-11-06: The system prompt is built in and editable with a reset (owner). The owner's PixelAssist template is adapted to Pincel's real tools; detection, segmentation and style transfer are named as unavailable (owner).
- 2026-11-06: Pixel-returning tools are hidden from cloud models until the user turns on "Agent may see the image" per chat; Pincel enforces it, because a prompt rule is only a request to the model (owner).
- 2026-11-06: The chat works independently of the external "Allow AI agents" setting through its own token (owner).
- 2026-11-06 (clarification pass): Document-level tools (`doc.open`, `doc.new`, `doc.close`, `doc.list`, `doc.undo`, `doc.redo`) are hidden from the chat (owner). Changing the CLI or model starts a new chat, so a history with image data never moves to another model (owner). Web fetch dropped: the owner allowed web search only.
- 2026-11-12 (spec-build): Flags re-checked against `claude --help` (Claude Code 2.1.322): besides the spec's list the CLI gets `--include-partial-messages` (text deltas for AC-05) and `--tools WebSearch` (its only built-in tool at all, AC-16); the prompt goes to stdin, which then closes, and the system prompt goes to `--append-system-prompt-file`, because Rust starts an npm `.cmd` shim through `cmd.exe`, which cannot carry a line break in an argument. A `.cmd` shim is started by `std::process::Command` directly (no shell string is built); Stop ends the process tree (`taskkill /T /F` on Windows, the process group on Unix).
- 2026-11-12 (spec-build): A permission the CLI denied shows up in its `result` line (`permission_denials`); that fails the answer as AC-27. Tool names come back from Claude Code's wire form (`mcp__pincel__draw_rect`) to the real names through `pincel-agent`'s tool list.
- 2026-11-12 (spec-build): Undo grouping lives in the cores as `begin_group` / `end_group(kind)` on Pixel's `Bus` and Image's `UndoStack` (exposed as `assistantBegin` / `assistantEnd` in both wasm crates); the group start follows steps the history cap drops, and an answer with no change adds no step. New history kind `assistant` in both cores.
- 2026-11-12 (spec-build): Chat folders live in the user's app data folder (`<app_local_data_dir>/assistant/<chat>`), created owner-only and refused when a link stands in their place; the MCP config with the chat's token is a new owner-only file per answer and is removed when the answer ends (security review). The Rust tests use a fake CLI built as the example `fake_claude`.
- 2026-11-12 (spec-build): The Node-side tests of the context block, the default prompts and the text segments (`ui/e2e/assistant-context.spec.ts`) were written right after the modules and were green at once; their red run is still owed when the panel wiring lands.
- 2026-11-12 (spec-build): The system-prompt editor and New chat are icon buttons in the panel header — the panel has no menu of its own, and the editors' menus are inert while an answer runs. "Reset to default" applies at once; saving the unchanged default text stores nothing, so the default keeps following the app's language (AC-18).
- 2026-11-12 (spec-build): The CLI and the model per CLI are app-wide prefs (`assistantCli`, `assistantModels`, `assistantPrompt` in IndexedDB); a chat with messages keeps the CLI and model its messages went to, and a change there asks and starts a new chat for that tab only (AC-25).
- 2026-11-12 (spec-build): AC-10's inert editor also covers every panel but the Assistant (Layers, History, Timeline change the document), file drops, pastes, File ▸ New / Open from the native menu, and native editor menu items other than zoom and the shortcut overview; the keys that pass are zoom, fit, actual size, the grids, rulers, Toggle UI and the shortcut overview.
- 2026-11-12 (spec-build): Closing or restarting the app waits at most 5 s for stopped answers to end (the CLI itself ends within 2 s, AC-11) before the unsaved-changes flow. The UI refuses the chat-hidden tools and a foreign `doc` itself as well as the endpoint, so a chat's call can never reach another tab.
- 2026-11-12 (spec-build): Red run of the UI blocks: `ui/e2e/assistant-panel.spec.ts` and `ui/e2e/assistant-chat.spec.ts` against a bundle built from the previous head (UI sources stashed): 14 of 15 failed for the missing panel; AC-01 passed at once, because the browser build had no panel before — it guards that the new panel stays desktop-only. The Node-side tests of `ui/e2e/assistant-context.spec.ts` stay green-at-once as recorded above; the e2e specs now cover their wiring.
- 2026-11-12 (spec-build): `gen.*` host tools and the Generate panel do not exist yet (`comfyui-generate` is `todo`), so AC-10's Generate / Apply and AC-11's cancelled `gen.*` call have nothing to act on in this build. Superseded by the 2026-11-13 entry below, now that `comfyui-generate` is built.
- 2026-11-13 (fix): A chat's `gen.*` call runs beside the call chain like any generation (comfyui-generate, Decisions) but is bound to the chat's tab as every chat call (AC-26): no `doc` targets the chat's tab, a foreign `doc` fails with "not available in the chat". The end of an answer waits for the chat's running generations as well as for the chain, so a generated layer lands inside the one "Assistant" step (AC-08); another client's generation does not hold an answer up. Stop — and so closing the tab or the app (AC-14, AC-22) — aborts the chat's running generations through the same path as a timed-out call (comfyui-generate AC-20): the job is cancelled on the server and nothing is added (AC-11). The Generate panel is one of the panels AC-10 makes inert; its Generate and Apply are buttons only, no key reaches them, and the canvas shows a variant but cannot apply it. Red run: `ui/e2e/assistant-generate.spec.ts` against the build before each fix, 3 of 3 failed (the generation ran on the shown tab; the answer ended before its generation; Stop sent no cancel); its AC-10 inert check passed before the fix.
