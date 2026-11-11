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

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | e2e `the PWA has no View ▸ Image generation… (AC-01)`, `both editors offer View ▸ Image generation… (AC-01)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-02 | `settings_default_to_automatic_and_persist_across_restarts` (src-tauri/src/comfyui/manager/tests.rs); `settings_default_to_automatic`, `settings_round_trip_through_the_file` (src-tauri/src/comfyui/settings.rs); e2e `the dialog shows the stored choice and stores a new one (AC-02)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-03 | `comfyui::manager::tests::config_default_tries_8188_then_8000_and_waits_120_s` (the defaults the app runs with); `ensure_automatic_prefers_the_first_port`, `ensure_automatic_skips_a_server_that_is_not_comfyui`, `ensure_automatic_without_a_server_is_not_found`, `ensure_automatic_uses_the_managed_installation_first` (src-tauri/src/comfyui/manager/tests.rs); e2e `opening the dialog looks for a server; none found offers the ways on (AC-03)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-04 | `ensure_reports_version_gpu_checkpoints_and_loras` (src-tauri/src/comfyui/manager/tests.rs); `fetch_info_reads_version_gpu_checkpoints_and_loras`, `parse_object_info_reads_v3_combo_options` (src-tauri/src/comfyui/api.rs); e2e `a connected server shows version, GPU and models (AC-04)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-05 | `choose_folder_accepts_an_installation_and_refuses_others` (src-tauri/src/comfyui/manager/tests.rs); `detect_*` (src-tauri/src/comfyui/install.rs); e2e `a picked folder is accepted or refused with the reason (AC-05)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-06 | `ensure_starts_the_existing_installation_bound_to_loopback`, `requests_during_a_start_wait_for_the_same_start`, `ensure_starts_again_after_the_server_stopped` (src-tauri/src/comfyui/manager/tests.rs); e2e `a start shows "Starting ComfyUI…" until the server answers (AC-06)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-07 | `comfyui::manager::tests::config_default_tries_8188_then_8000_and_waits_120_s` (the defaults the app runs with); `a_start_that_never_answers_stops_with_the_last_50_lines`, `a_process_that_exits_while_starting_stops_with_its_output`, `a_started_server_that_exits_later_reports_stopped` (src-tauri/src/comfyui/manager/tests.rs); e2e `a server that stops shows "ComfyUI stopped" with its output (AC-07)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-08 | `test_url_classifies_every_outcome`, `test_url_over_https_reaches_the_server_and_reports_failures` (src-tauri/src/comfyui/manager/tests.rs); `identify_classifies_refused_and_foreign_servers` (src-tauri/src/comfyui/api.rs); `parse_url_*`, `is_loopback_only_for_this_machine`, `get_https_speaks_tls_and_a_failed_handshake_is_unreachable`, `get_https_to_a_plain_http_server_is_unreachable`, `get_*` (src-tauri/src/comfyui/http.rs); e2e `Test connection reports each outcome; remote hosts get a notice (AC-08)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-09 | `comfyui::manager::tests::shutdown_ends_the_started_server_and_no_other` (src-tauri/src/comfyui/manager/tests.rs) | built |        |
| AC-10 | `comfyui::manager::tests::a_new_run_adopts_the_server_an_earlier_run_left_running`, `a_record_whose_server_is_gone_is_forgotten` (src-tauri/src/comfyui/manager/tests.rs) | built |        |
| AC-11 | `a_server_without_a_required_node_type_is_too_old` (src-tauri/src/comfyui/manager/tests.rs); `parse_object_info_names_missing_node_types` (src-tauri/src/comfyui/api.rs); e2e `a server lacking core node types is too old (AC-11)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-12 | `a_server_without_checkpoints_has_no_model_installed` (src-tauri/src/comfyui/manager/tests.rs); e2e `a server without checkpoints has no model installed (AC-12)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-13 | `a_missing_installation_folder_is_reported_with_its_path` (src-tauri/src/comfyui/manager/tests.rs); e2e `a vanished installation folder asks for another (AC-13)` (ui/e2e/comfyui-connect.spec.ts) | built |        |
| AC-14 | e2e `dialog, status texts and errors are translated (AC-14)` (ui/e2e/comfyui-connect.spec.ts); en/de key parity in `pnpm check` (`de: Record<MessageKey, string>`) and `scripts/ui-lint.mjs` | built |        |

## Decisions

- 2026-11-06: Pincel finds and starts existing ComfyUI installations and offers a managed install (`comfyui-install`) — the owner wants users without ComfyUI to get started from Pincel.
- 2026-11-06: The connection lives in the Tauri backend, not the webview — the CSP stays closed and the PWA has no way to start a server anyway (Decision Log 2026-11-06 "ComfyUI").
- 2026-11-06: Automatic prefers the managed installation, then 8188, then 8000 — a server Pincel installed is the one it can also start, update and stop (clarification pass).
- 2026-11-06: A server is started when something needs it (dialog, visible Generate panel, generation), not at app start — no GPU memory is taken while the user does not use generation (clarification pass).
- 2026-11-11: The settings (`comfyui.json`) and the record of a started server (`comfyui-run.json`) live in the app config dir, read and written by `pincel-tauri` — the backend resolves the server for the dialog, the Generate panel and `gen.*` calls alike, without asking the webview.
- 2026-11-11: HTTP and HTTPS go through `reqwest` with `rustls` (ring) installed once as the process-wide crypto provider — approved by the owner on 2026-11-11 (Decision Log 2026-11-11 "rustls"), after a first cut had used a plain-HTTP `std::net` client because `reqwest` panics without a provider. Loopback probes bypass the system proxy; other hosts use it.
- 2026-11-11: A ComfyUI Desktop folder is the Electron app's folder (`resources/app.asar`, `ComfyUI.exe`, `Contents/Resources/app.asar`) or its data folder (`.venv` and `models`, but no `main.py`) — the app keeps `main.py` inside its own resources.
- 2026-11-11: On Windows a started server is ended with `taskkill /T /F` — a venv's `python.exe` is a launcher that runs the interpreter as a child, so killing the launcher alone leaves the server running.
- 2026-11-11: Pincel owns at most one server: starting another installation ends the one it started before, so two servers never compete for the GPU.
- 2026-11-11: At app start the backend probes the recorded port once (2 s, off the start path); a server that answers is adopted and ended on quit even if nothing needs a server in this run (AC-10), a silent one's record is dropped.
- 2026-11-11: The Generate panel and the `gen.*` tools do not exist yet (`comfyui-generate` depends on this spec). The parts of AC-03 and AC-12 that name them are covered by the backend contract they will call — `Manager::ensure` / `ensure_usable` and the `comfyui_usable` command, which fails with "no model installed" — and tested there; the panel's disabled Generate and the tools' errors are `comfyui-generate` AC-02 and AC-19.
- 2026-11-11: The process tests run a fake `main.py` (a small Python HTTP server) through a `--without-pip` venv made from the Python 3 on `PATH`; the `pincel-tauri` CI job (Ubuntu) has `python3`.
- 2026-11-11: The menu entry reads "Image Generation…" — Title Case like every other menu entry (View ▸ Touch Gestures, Help ▸ Check for Updates…); the dialog title is "Image generation".
- 2026-11-11: "Install ComfyUI…" (AC-03) is shown disabled with the tooltip "Installing ComfyUI from Pincel is not available yet" until `comfyui-install` gives it a dialog to open.
- 2026-11-11: Choosing Automatic or an installation connects at once; choosing Custom URL waits for "Test connection" (or Enter in the field), which stores the URL and probes it. The non-loopback notice follows the typed URL, before any test.
- 2026-11-11: AC-01's PWA half (no entry in a browser) held before the change, so its e2e test was green on the red run; the desktop half and every other e2e test were red (the menu entry did not exist).
- 2026-11-11: HTTPS is tested offline only: that an `https://` URL speaks TLS (the server receives a ClientHello) and that a failed handshake, a plain-HTTP server or a dead port reads "Not reachable". A successful handshake is not tested — a local TLS server needs a certificate generator and a TLS server crate (`rcgen`, `tokio-rustls`), neither approved; certificate checks are rustls-platform-verifier's (the OS trust store), as for the updater.
