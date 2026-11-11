---
status: todo
created: 2026-11-06
size: M
mode: shell
design: [pincel.md §11, pincel.md §15 (2026-11-06 ComfyUI)]
---

# ComfyUI connection

## Context

The owner wants Pincel to generate images with a local image model through ComfyUI, both from a "Generate" panel and as an agent tool (`comfyui-generate`). Before anything can be generated, the desktop app must find, start and talk to a ComfyUI server. This spec covers the connection: finding a running server, starting an existing installation, a custom server URL, the status display and the list of installed models. `comfyui-install` adds a managed installation as a fourth server choice.

The connection runs in the Tauri backend (Decision Log 2026-11-06 "ComfyUI"), so the webview's CSP stays closed. Desktop only.

## Scope

### In scope

- View ▸ Image generation… dialog: server choice (Automatic, Existing installation, Custom URL; Managed once `comfyui-install` has installed one), status, model list, "Test connection"
- Automatic discovery of a running ComfyUI
- Starting an existing installation when a server is needed and stopping it when Pincel quits
- Checking that the server has the core node types Pincel's built-in workflows need
- Listing installed checkpoints and LoRAs for the Generate panel and `gen.models`
- English and German strings

### Out of scope

- Installing ComfyUI or models — because `comfyui-install` does that
- Generating images — because `comfyui-generate` does that
- Custom nodes and custom workflows — because v1 uses only ComfyUI's core nodes, which every stock ComfyUI has (backlog: custom workflows)
- Authentication against remote servers — because ComfyUI itself has none; a reverse proxy with auth is the user's setup, and Pincel reports its refusal (AC-08)
- Starting ComfyUI Desktop (the Electron app) — because it manages its own server and window; Pincel finds it once it runs
- The PWA — because a browser page cannot start processes and the webview's CSP blocks other origins

## Acceptance criteria

- [ ] AC-01 (option): Where the app runs in a browser (PWA), View ▸ Image generation… is not shown.
- [ ] AC-02 (always): The server choice is Automatic on a fresh install; the choice, the existing-installation folder and the custom URL persist across restarts.
- [ ] AC-03 (event): When a server is needed while Automatic is selected (the dialog opens, the Generate panel is expanded and visible, Generate, a `gen.*` call), then the app uses, in this order, the managed installation if one exists, else a ComfyUI answering on 127.0.0.1:8188, else on 127.0.0.1:8000; if none is available, it shows "No ComfyUI server found" with the buttons "Use an existing installation…" and "Install ComfyUI…".
- [ ] AC-04 (state): While a server is connected, the dialog shows its ComfyUI version and the name and VRAM of its first GPU, and lists its installed checkpoints and LoRAs.
- [ ] AC-05 (event): When the user chooses "Existing installation" and picks a folder, then the app accepts a ComfyUI installation it can start (a portable build, a folder with its own Python environment, or a folder whose `ComfyUI` subfolder is one) and otherwise says what is missing; a ComfyUI Desktop folder is refused with "Start ComfyUI Desktop — Pincel finds it automatically".
- [ ] AC-06 (event): When a server is needed while the chosen existing or managed installation does not answer, then the app starts it bound to 127.0.0.1 on a free port, shows "Starting ComfyUI…", and continues once it answers; requests arriving during the start wait for the same start instead of starting a second process.
- [ ] AC-07 (error): If a started server does not answer within the start timeout (120 s) or its process exits — while starting or later — then the status shows "ComfyUI stopped" with the last 50 lines of its output, and a running generation fails (`comfyui-generate` AC-15).
- [ ] AC-08 (event): When the user clicks "Test connection" for a custom URL (http or https, with or without a path prefix), then the dialog shows the AC-04 details on success, or one of "Not a valid URL", "Not reachable", "Not a ComfyUI server", "The server refused access (<status>)"; a URL whose host is not a loopback address shows the notice "Images and prompts are sent to this server".
- [ ] AC-09 (event): When the app quits, then a ComfyUI process the app started is ended; a server it did not start keeps running.
- [ ] AC-10 (event): When the app starts and a server it started in an earlier run still answers on the recorded port (the app was killed), then it uses that server instead of starting another, and ends it on quit.
- [ ] AC-11 (error): If the connected server lacks a node type the built-in workflows need, then the status shows "This ComfyUI is too old — update it" and names the missing node types.
- [ ] AC-12 (state): While the connected server has no checkpoint, the dialog shows "No model installed", the Generate panel's Generate is disabled and `gen.*` calls fail with "no model installed".
- [ ] AC-13 (error): If the configured existing-installation folder no longer exists, then the status shows "Installation not found at <path>" with "Choose another folder…".
- [ ] AC-14 (always): The dialog, status texts and errors are translated in English and German.

## Technical notes

- **API.** `GET /system_stats` (its `system.comfyui_version` field identifies ComfyUI; version, devices, VRAM) with a 2 s timeout per probe (a local server answers in milliseconds; discovery must not stall the dialog); `GET /object_info` for node types; checkpoints and LoRAs from the `object_info` of `CheckpointLoaderSimple` and `LoraLoader`.
- **Required node types.** `CheckpointLoaderSimple`, `LoraLoader`, `CLIPTextEncode`, `EmptyLatentImage`, `KSampler`, `VAEDecode`, `VAEEncode`, `VAEEncodeForInpaint`, `LoadImage`, `LoadImageMask`, `SetLatentNoiseMask`, `SaveImage`.
- **Start.** `<python> main.py --listen 127.0.0.1 --port <port>` through `std::process` (arguments, never a shell); Python is the portable build's embedded one or the folder's `venv` / `.venv`. The port and process id of a server Pincel started are recorded in the app config dir (AC-10). The start timeout is a constant tests can shorten.
- **Tests.** Rust tests against a mock HTTP server and a fake `main.py`; Playwright against the mocked Tauri bridge.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Both 8188 and 8000 answer | 8188 wins (AC-03): it is ComfyUI's own default, 8000 only ComfyUI Desktop's |
| ComfyUI Desktop (Electron app) is running | Found on 8000; Pincel neither starts nor stops it |
| Port 8188 is taken by something that is not ComfyUI | No `comfyui_version` in its answer; discovery continues with 8000 |
| Existing installation already runs from the user's own shortcut on 8188 | Automatic finds it; with "Existing installation" selected, Pincel starts its own copy on a free port only if the recorded port does not answer |
| A krita-ai-diffusion managed server runs | Found like any ComfyUI; its checkpoints are listed |
| A connected server stops between two generations | The next request finds it unreachable; an installation Pincel can start is started again (AC-06), otherwise "Not reachable" |
| Custom URL behind a proxy answering 401 | "The server refused access (401)" (AC-08) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| ComfyUI's default ports are 8188 (manual / portable) and 8000 (ComfyUI Desktop) | Research 2026-11-06 | A changed default needs a custom URL |
| 120 s start timeout | The first start of a fresh ComfyUI imports PyTorch and scans models, which takes over a minute on slow disks | A very slow first start needs a second try |
| The last 50 output lines | Enough to show a Python traceback or a CUDA error without flooding the dialog | Long tracebacks are cut |
| Only the first GPU is shown | ComfyUI generates on one device by default | Multi-GPU users see one device |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

- **HTTPS for custom URLs (AC-08) needs a new direct dependency.** `reqwest` (approved, Decision Log 2026-11-06) is compiled with `rustls-no-provider` through the updater plugin and panics when a client is built without a process-wide rustls crypto provider; installing one needs `rustls = { version = "0.23", default-features = false, features = ["ring"] }` as a direct dependency of `pincel-tauri` (already in `Cargo.lock`, no new crate) — not approved yet. Until then the connection speaks plain HTTP through a small `std::net` client (`src-tauri/src/comfyui/http.rs`) and an `https://` URL reports "HTTPS is not supported yet". Approve `rustls` (Decision Log entry) and the transport moves to `reqwest` in one file?

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
| AC-09 | `comfyui::manager::tests::shutdown_ends_the_started_server_and_no_other` (src-tauri/src/comfyui/manager/tests.rs) | built |        |
| AC-10 | `comfyui::manager::tests::a_new_run_adopts_the_server_an_earlier_run_left_running`, `a_record_whose_server_is_gone_is_forgotten` (src-tauri/src/comfyui/manager/tests.rs) | built |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |

## Decisions

- 2026-11-06: Pincel finds and starts existing ComfyUI installations and offers a managed install (`comfyui-install`) — the owner wants users without ComfyUI to get started from Pincel.
- 2026-11-06: The connection lives in the Tauri backend, not the webview — the CSP stays closed and the PWA has no way to start a server anyway (Decision Log 2026-11-06 "ComfyUI").
- 2026-11-06: Automatic prefers the managed installation, then 8188, then 8000 — a server Pincel installed is the one it can also start, update and stop (clarification pass).
- 2026-11-06: A server is started when something needs it (dialog, visible Generate panel, generation), not at app start — no GPU memory is taken while the user does not use generation (clarification pass).
- 2026-11-11: The settings (`comfyui.json`) and the record of a started server (`comfyui-run.json`) live in the app config dir, read and written by `pincel-tauri` — the backend resolves the server for the dialog, the Generate panel and `gen.*` calls alike, without asking the webview.
- 2026-11-11: Plain HTTP through a `std::net` client instead of `reqwest` for now — `reqwest` cannot build a client without a rustls crypto provider, and installing one needs an unapproved dependency (Open questions). Local ComfyUI servers speak HTTP, so only `https://` custom URLs wait for the answer.
- 2026-11-11: A ComfyUI Desktop folder is the Electron app's folder (`resources/app.asar`, `ComfyUI.exe`, `Contents/Resources/app.asar`) or its data folder (`.venv` and `models`, but no `main.py`) — the app keeps `main.py` inside its own resources.
- 2026-11-11: On Windows a started server is ended with `taskkill /T /F` — a venv's `python.exe` is a launcher that runs the interpreter as a child, so killing the launcher alone leaves the server running.
- 2026-11-11: Pincel owns at most one server: starting another installation ends the one it started before, so two servers never compete for the GPU.
- 2026-11-11: At app start the backend probes the recorded port once (2 s, off the start path); a server that answers is adopted and ended on quit even if nothing needs a server in this run (AC-10), a silent one's record is dropped.
- 2026-11-11: The Generate panel and the `gen.*` tools do not exist yet (`comfyui-generate` depends on this spec). The parts of AC-03 and AC-12 that name them are covered by the backend contract they will call — `Manager::ensure` / `ensure_usable` and the `comfyui_usable` command, which fails with "no model installed" — and tested there; the panel's disabled Generate and the tools' errors are `comfyui-generate` AC-02 and AC-19.
- 2026-11-11: The process tests run a fake `main.py` (a small Python HTTP server) through a `--without-pip` venv made from the Python 3 on `PATH`; the `pincel-tauri` CI job (Ubuntu) has `python3`.
