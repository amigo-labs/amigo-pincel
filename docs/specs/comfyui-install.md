---
status: todo
created: 2026-11-06
size: L
mode: shell
design: [pincel.md §11, pincel.md §15 (2026-11-06 ComfyUI managed install)]
depends_on: [comfyui-connect]
---

# ComfyUI managed install

## Context

Users without ComfyUI should get image generation from inside Pincel with one click. The owner pointed to krita-ai-diffusion, which installs a managed ComfyUI server: it fetches `uv`, lets `uv` create a Python environment, downloads a pinned ComfyUI release, installs PyTorch for the user's GPU and offers model packages ("workloads"). Pincel follows the same approach, written from scratch: krita-ai-diffusion and ComfyUI are GPL-3.0, Pincel is MIT OR Apache-2.0, so no code is copied and nothing of ComfyUI ships with Pincel; everything is downloaded at runtime and ComfyUI stays a separate program reached over HTTP (Decision Log 2026-11-06).

v1 supports NVIDIA GPUs on Windows and Linux (x86-64) and Apple Silicon Macs — the owner's choice, because those two backends cover most people running image models locally and every further backend is another PyTorch build to test; other GPUs connect a self-installed server (`comfyui-connect`).

## Scope

### In scope

- An "Install ComfyUI…" wizard opened from View ▸ Image generation…: hardware check, install location, package choice with sizes and licenses, installation with progress, cancel and retry
- Pinned versions of everything the install fetches; Pincel's own downloads checked against pinned SHA-256 hashes, Python packages against the hashes of a pinned lock file
- A model catalog: SDXL base, SD 1.5, Flux.1 schnell, a pixel-art LoRA for SDXL
- The managed installation as the server choice "Managed", started and stopped as in `comfyui-connect` AC-06, AC-07, AC-09, AC-10
- Adding and removing packages later, updating to a newer pinned ComfyUI with rollback, uninstalling
- English and German strings

### Out of scope

- AMD (ROCm), Intel (XPU), DirectML, CPU-only and ARM Windows / Linux installs — because v1 is limited to NVIDIA and Apple Silicon (Context); backlog
- Custom nodes — because the built-in workflows use core nodes only
- Arbitrary model downloads (Civitai, Hugging Face search) — because every package needs a pinned hash, a shown license and a download without login
- Bundling ComfyUI, Python or models with the Pincel installer — because of size (several GB) and the GPL-3.0 boundary

## Acceptance criteria

- [ ] AC-01 (event): When the user starts "Install ComfyUI…", then the wizard shows the platform and the first supported GPU (name and VRAM) or Mac chip (name and memory) and whether the system is supported; an unsupported system (other GPU, no NVIDIA driver, Intel Mac, ARM Windows / Linux) gets the reason and the buttons "Use an existing installation…" and "Custom URL" instead of Install.
- [ ] AC-02 (state): While the NVIDIA GPU has less than 6 GB VRAM or the Mac less than 16 GB memory, the wizard shows "Generation will be slow or fail on this hardware" and still allows installing.
- [ ] AC-03 (event): When the user picks a location, then the installation goes into a subfolder "Pincel ComfyUI" of it; the wizard shows the free disk space and the required space (selected packages, base install, 5 GB reserve) and disables Install while free space is short or the location is not writable.
- [ ] AC-04 (always): The package list shows SDXL base, SD 1.5, Flux.1 schnell and the SDXL pixel-art LoRA, each with download size, license and minimum VRAM / memory, the latter highlighted when the hardware is below it; at least one checkpoint stays selected; selecting the LoRA selects SDXL base, deselecting SDXL base deselects the LoRA.
- [ ] AC-05 (event): When the user clicks Install, then the wizard shows the license of every selected package not accepted before and requires "I accept" for each before anything is downloaded, plus a notice naming ComfyUI (GPL-3.0) and PyTorch (BSD-3-Clause) as separate programs being installed; accepted licenses are remembered.
- [ ] AC-06 (state): While installing, the wizard shows the current step ("Preparing Python", "Downloading ComfyUI", "Installing PyTorch and dependencies", "Downloading <package>"), bytes downloaded and total for Pincel's own downloads, the latest output line for the other steps, and a Cancel button.
- [ ] AC-07 (error): If a file Pincel downloads does not match its pinned SHA-256, then the file is deleted and the install stops with "Download corrupted: <file>" and Retry.
- [ ] AC-08 (error): If a step fails (pinned URL gone: "Download no longer available — update Pincel"; no connection: "No internet connection"; disk full: "Disk full"; Python or package setup fails: its last 50 output lines), then the install stops with that message and Retry.
- [ ] AC-09 (event): When the user retries after a cancel, an error or closing Pincel mid-install, then completed steps and verified files are not repeated; a partly downloaded file continues where the server supports it and restarts otherwise.
- [ ] AC-10 (event): When the install finishes, then the server choice becomes Managed, the managed server starts and the dialog shows it connected with the installed models; if it fails to start, the install stays complete and the status shows `comfyui-connect` AC-07's message with "Retry start".
- [ ] AC-11 (event): When the user adds or removes packages later, then only those models are downloaded or deleted, AC-05 – AC-09 applying to downloads; the last remaining checkpoint cannot be removed, and removing SDXL base also removes the LoRA after a confirmation.
- [ ] AC-12 (event): When a Pincel update pins a newer ComfyUI, then the dialog offers "Update ComfyUI", which replaces ComfyUI and its Python packages with the new pins and keeps the models; if any step fails or the updated server does not start, the previous installation is restored and starts.
- [ ] AC-13 (event): When the user clicks Uninstall and confirms, then the "Pincel ComfyUI" folder with all models is deleted and the server choice falls back to Automatic.
- [ ] AC-14 (state): While a generation runs, package changes, update and uninstall are disabled; otherwise they stop the managed server first.
- [ ] AC-15 (error): If the managed folder no longer exists, then the status shows "Installation not found at <path>" with "Install again…" and "Choose another server".
- [ ] AC-16 (always): The wizard, steps, license notices and errors are translated in English and German.
- [ ] AC-17 (always, manual: needs the release build): Pincel's installer and app bundle contain no ComfyUI source, no Python interpreter or packages and no model files.
- [ ] AC-18 (state, manual: needs real hardware and about 6 GB download): On Windows with an NVIDIA GPU and on an Apple Silicon Mac, installing with SD 1.5 only ends with the managed server connected and listing that checkpoint.

## Technical notes

- **Steps.** (1) Download the pinned `uv` release and verify it. (2) `uv venv --python <pinned 3.12.x> <folder>/venv` — the exact Python version is in the pins table, and uv verifies the interpreter build it fetches against its own checksums. (3) Download the pinned ComfyUI release archive and extract it to `<folder>/ComfyUI`. (4) `uv pip sync --require-hashes` a lock file per platform shipped with Pincel (PyTorch from the CUDA index on Windows / Linux, the default index on macOS, plus ComfyUI's requirements), generated with `uv pip compile --generate-hashes`. (5) Write `extra_model_paths.yaml` pointing at `<folder>/models`. (6) Download the selected models into `<folder>/models/checkpoints` and `…/loras`. A `.version` file records Pincel's pin set and the backend; it is written last, so a folder without it is an incomplete install that Retry resumes. Accepted licenses are stored in the app config dir.
- **Python 3.12.** Every pinned PyTorch build exists for it on all three platforms, and ComfyUI recommends it.
- **Pins.** One table in the code: uv version, exact Python version, ComfyUI version, the lock files, the base install's size per platform (for AC-03), and per model package its URL (downloadable without login), size, SHA-256, license and minimum VRAM / memory.
- **GPU detection.** `nvidia-smi --query-gpu=name,memory.total --format=csv` on Windows / Linux (missing tool = no driver); `sysctl` and the architecture on macOS. No new crate.
- **Crates.** `reqwest` (download, HTTP range), `sha2`, `zip`, `flate2` + `tar` — all in `Cargo.lock` through the updater plugin (Decision Log 2026-11-06). Processes start through `std::process` with arguments, never a shell.
- **Tests.** Rust tests run the steps against a local mock file server and a fake `uv` (hash check, resume, rollback, cancel, error mapping); Playwright drives the wizard against the mocked Tauri bridge. AC-17 and AC-18 are manual.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Location already contains a complete "Pincel ComfyUI" | The wizard offers "Use this installation" instead of installing again |
| Location path with spaces or non-ASCII characters | Works |
| Network drive as location | Allowed; slow, no special handling |
| Pincel restarted after an interrupted install | View ▸ Image generation… shows "Installation incomplete" with "Resume" and "Delete"; the server choice stays as it was until the install finishes |
| NVIDIA driver too old for the pinned CUDA build | The server fails to start (`comfyui-connect` AC-07); when its output names the driver, the status adds "Update your NVIDIA driver" |
| Managed install exists and another ComfyUI runs on 8188 | The server choice decides; Automatic prefers the managed one (`comfyui-connect` AC-03) |
| Flux.1 schnell selected on an 8 GB GPU | Allowed; its minimum (12 GB) is highlighted (AC-04) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 6 GB VRAM / 16 GB Mac memory as the warning threshold | krita-ai-diffusion recommends ≥ 6 GB VRAM; SDXL on Apple Silicon needs about 16 GB | Larger models need more; each package shows its own minimum |
| 5 GB reserve on top of the download sizes | Extraction and uv's cache need room temporarily | Very full disks fail later with "Disk full" (AC-08) |
| Flux.1 schnell is offered as a single-file fp8 checkpoint | Loads with the core checkpoint loader like the others | Larger variants would need separate text-encoder files |
| The concrete pixel-art LoRA is picked when building, among SDXL LoRAs downloadable without login whose license allows commercial use; the pick and its license go into `## Decisions` | Pincel users make game assets they sell; the owner asked for "a pixel-art LoRA", not a specific one | The owner may prefer another; swapping it is a pin change |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `classify_nvidia_on_windows_and_linux_is_supported`, `classify_apple_silicon_is_supported_and_slow_below_16_gib`, `classify_names_the_reason_for_unsupported_systems`, `parse_nvidia_smi_reads_the_first_gpu` (src-tauri/src/comfyui/hardware.rs); e2e `the wizard shows the GPU and whether it is supported (AC-01)`, `an unsupported system gets the reason and the other ways in (AC-01)`, `an Intel Mac and a missing NVIDIA driver say why (AC-01)`, `a Mac shows its chip and memory (AC-01, AC-02)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-02 | `classify_nvidia_on_windows_and_linux_is_supported`, `classify_apple_silicon_is_supported_and_slow_below_16_gib` (src-tauri/src/comfyui/hardware.rs); e2e `a small GPU gets the warning and may still install (AC-02)`, `a Mac shows its chip and memory (AC-01, AC-02)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-03 | `free_space_reports_the_volume_of_a_not_yet_existing_folder`, `parse_df_reads_the_available_column`, `writable_is_false_for_a_missing_folder` (src-tauri/src/comfyui/hardware.rs); `info_and_check_location_give_the_wizard_its_numbers` (src-tauri/src/comfyui/managed/tests.rs); e2e `the location gets a subfolder, free and required space; short or read-only blocks Install (AC-03)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-04 | `catalog_lists_the_four_packages_with_size_license_and_minimum`, `select_needs_a_checkpoint_and_sdxl_for_the_lora` (src-tauri/src/comfyui/pins.rs); e2e `packages show size, license and minimum; the selection rules hold (AC-04)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-05 | `install_refuses_unaccepted_licenses_and_selections_without_a_checkpoint`, `accepted_licenses_are_remembered_across_restarts` (src-tauri/src/comfyui/managed/tests.rs); e2e `Install asks for each new license and names ComfyUI and PyTorch (AC-05)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-06 | `install_runs_every_step_and_writes_version_last` (src-tauri/src/comfyui/engine/tests.rs); `fetch_verifies_the_hash_and_moves_the_file_into_place` (src-tauri/src/comfyui/download.rs); e2e `installing shows the step, bytes or the latest line, and Cancel (AC-06)`, `reopening the dialog during an install shows its progress` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-07 | `fetch_with_a_wrong_hash_deletes_the_file_and_reports_it_corrupted` (src-tauri/src/comfyui/download.rs); `install_with_a_corrupted_model_stops_and_a_retry_skips_finished_steps` (src-tauri/src/comfyui/engine/tests.rs); e2e `a corrupted download stops with its name and Retry resumes (AC-07, AC-09)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-08 | `fetch_maps_a_missing_url_and_a_dead_server`, `fetch_after_a_dropped_transfer_is_offline_and_a_retry_continues`, `install_error_io_maps_a_full_disk` (src-tauri/src/comfyui/download.rs); `install_when_package_setup_fails_reports_its_last_50_lines`, `install_when_python_setup_fails_reports_its_output`, `install_when_the_disk_fills_up_says_disk_full`, `install_from_a_pinned_url_that_is_gone_says_so`, `mentions_full_disk_knows_uv_and_windows_wording` (src-tauri/src/comfyui/engine/tests.rs); e2e `each failing step has its message; setup failures show their output (AC-08)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-09 | `fetch_resumes_a_part_file_with_a_range_request`, `fetch_restarts_when_the_server_ignores_the_range`, `fetch_after_a_dropped_transfer_is_offline_and_a_retry_continues`, `fetch_stops_when_cancelled` (src-tauri/src/comfyui/download.rs); `install_with_a_corrupted_model_stops_and_a_retry_skips_finished_steps`, `install_when_package_setup_fails_reports_its_last_50_lines`, `install_cancelled_during_setup_ends_uv_and_a_retry_resumes` (src-tauri/src/comfyui/engine/tests.rs); `an_interrupted_install_is_incomplete_after_a_restart_and_resume_finishes_it` (src-tauri/src/comfyui/managed/tests.rs); e2e `a corrupted download stops with its name and Retry resumes (AC-07, AC-09)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-10 | `a_finished_install_selects_managed_and_starts_the_server`, `a_finished_install_whose_server_fails_stays_complete_and_reports_stopped` (src-tauri/src/comfyui/managed/tests.rs); e2e `a finished install shows the started managed server (AC-10)`, `a managed server that fails to start offers Retry start and the driver hint (AC-10, edge case)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-11 | `add_models_downloads_only_new_ones_and_remove_models_deletes_only_those` (src-tauri/src/comfyui/engine/tests.rs); `package_changes_touch_only_those_models_and_keep_a_checkpoint` (src-tauri/src/comfyui/managed/tests.rs); e2e `packages are added and removed later; SDXL takes the LoRA after asking (AC-11)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-12 | `update_replaces_comfyui_and_keeps_the_models`, `update_that_fails_restores_the_previous_installation`, `update_whose_server_does_not_start_restores_the_previous_installation` (src-tauri/src/comfyui/engine/tests.rs); `newer_pins_offer_an_update_that_keeps_the_models`, `a_failed_update_restores_and_starts_the_previous_installation` (src-tauri/src/comfyui/managed/tests.rs); e2e `a newer pinned ComfyUI is offered as Update ComfyUI (AC-12)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-13 | `uninstall_deletes_the_folder_and_falls_back_to_automatic`, `delete_of_an_incomplete_install_removes_the_folder` (src-tauri/src/comfyui/managed/tests.rs); e2e `Uninstall asks, deletes and falls back to Automatic (AC-13)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-14 | `a_running_generation_blocks_package_changes_update_and_uninstall` (src-tauri/src/comfyui/managed/tests.rs); e2e `a running generation disables package changes, update and uninstall (AC-14)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-15 | `a_vanished_managed_folder_is_reported_with_its_path` (src-tauri/src/comfyui/managed/tests.rs); e2e `a vanished managed folder offers Install again… and another server (AC-15)` (ui/e2e/comfyui-install.spec.ts) | built |        |
| AC-16 | e2e `wizard, steps, license notice and errors are translated (AC-16)` (ui/e2e/comfyui-install.spec.ts); key parity of all six dictionaries in `pnpm check` (`Record<MessageKey, string>`) and `scripts/ui-lint.mjs` | built |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |

## Decisions

- 2026-11-06: One-click install in v1 (owner), modeled on krita-ai-diffusion's managed server without copying its GPL-3.0 code.
- 2026-11-06: v1 platforms: NVIDIA (CUDA) on Windows / Linux and Apple Silicon (MPS) (owner; reason in Context).
- 2026-11-06: Model catalog: SDXL base, SD 1.5, Flux.1 schnell, SDXL pixel-art LoRA; selectable like krita-ai-diffusion's workloads, license accepted per package (owner).
- 2026-11-06: The install goes into a "Pincel ComfyUI" subfolder of the chosen location, so Uninstall never deletes a folder the user picked (clarification pass).
- 2026-11-06: Python packages are pinned by a hashed lock file and installed with `uv`, and the Python build by its exact version and uv's checksums, so everything the install fetches is pinned, not only Pincel's own downloads (clarification pass).
- 2026-11-12: Pins, researched with a two-week cutoff (2026-10-29): uv 0.13.6, Python 3.12.16 (python-build-standalone 20261026), ComfyUI v0.43.0 (GitHub tag source archive), torch 2.14.1+cu130 on Windows / Linux (ComfyUI's README installs cu130 for NVIDIA; needs driver R580 or newer and a Turing or newer GPU) and torch 2.15.0 on macOS (the cu130 index has no 2.15; the macOS wheels need macOS 14). One `PIN_SET` id in `pins.rs` names the set; changing any pin means bumping it, which offers "Update ComfyUI" (AC-12).
- 2026-11-12: The lock files (`src-tauri/comfyui/locks/`, generated with `uv pip compile --generate-hashes --torch-backend cu130`, commands in their headers) carry `--find-links` lines for the PyTorch index, because uv does not write the `--torch-backend` index into the file and `--index-strategy` cannot go into a requirements file; `uv pip sync --require-hashes <file>` then needs no extra flags. On Linux, triton is the PyTorch-index build (its hash differs from PyPI's). `uv pip sync --dry-run` accepted all three files; a full install of them is AC-18's manual check.
- 2026-11-12: Model packages, all from Hugging Face repositories that are not gated, pinned to a commit with the LFS SHA-256: SDXL base 1.0 (`stabilityai/stable-diffusion-xl-base-1.0`, CreativeML Open RAIL++-M), SD 1.5 as Comfy-Org's fp16 single file (`Comfy-Org/stable-diffusion-1.5`, CreativeML Open RAIL-M; the official repository has no fp16 single file), Flux.1 schnell fp8 (`Comfy-Org/Flux-1-Schnell`, Apache-2.0; Black Forest Labs' own repository is gated).
- 2026-11-12: The pixel-art LoRA is `nerijs/pixel-art-xl` (`pixel-art-xl.safetensors`, 171 MB, CreativeML Open RAIL-M): downloadable without login, and its license grants commercial use (with the license's use-based restrictions); no trigger word is needed.
- 2026-11-12: Minimum VRAM / Mac memory per package: SD 1.5 4 / 8 GB, SDXL base and the LoRA 6 / 16 GB, Flux.1 schnell 12 / 32 GB — estimates from the checkpoint sizes and ComfyUI's offloading, not measurements.
- 2026-11-12: The base install size per platform (Windows 7.2 GB, Linux 9.8 GB, macOS 2.3 GB) is estimated as 2.5 × the locked wheels' download size + Python + 3 × the ComfyUI archive; the 5 GB reserve covers uv's cache.
- 2026-11-12: Free space comes from `df -Pk` (Linux, macOS) and from PowerShell's `DriveInfo` on Windows, the path passed in an environment variable, never in the command text; where it is unknown (a UNC path) the wizard says "Free: unknown" and does not block Install.
- 2026-11-12: Unsupported reasons: another GPU (named on Windows through `Win32_VideoController`), no NVIDIA driver (`nvidia-smi` missing or failing, including an NVIDIA adapter without a working driver), Intel Mac (`machdep.cpu.brand_string` not "Apple…", so an x86-64 build under Rosetta still counts as Apple Silicon), ARM Windows / Linux, any other OS.
- 2026-11-12: uv runs with `UV_PYTHON_INSTALL_DIR` and `UV_CACHE_DIR` inside the install folder, `--no-config` and `--python-preference only-managed`, so the install uses its pinned Python, ignores the user's uv configuration, and Uninstall removes everything.
- 2026-11-12: Downloads go to `<file>.part` and land under their name only after the hash matched, so an existing file counts as verified (AC-09); finished base steps are listed in `.install-state.json`. The uv and ComfyUI archives are deleted after extraction; models download straight into `models/<kind>/`.
- 2026-11-12: Update (AC-12) moves `uv`, `venv`, `ComfyUI`, the lock file and the state files into `rollback/`, installs the new pins, starts the server and restores `rollback/` on any failure, a cancel or a server that does not answer; the models and uv's Python builds stay. A `rollback/` left by a crash makes the next update resume rather than move again.
- 2026-11-12: The managed folder and its packages are recorded in `comfyui-managed.json`, accepted licenses (per package id) in `comfyui-licenses.json`, both in the app config dir; the server choice changes only when the install finishes (AC-10, edge case).
- 2026-11-12: AC-14: generation does not exist yet. The backend keeps a "generation running" flag that blocks package changes, update and uninstall; comfyui-generate sets it through the command `comfyui_set_generating` (or `Service::set_generating` from the backend). Until then nothing sets it.
- 2026-11-12: AC-13 sets the server choice to Automatic only when it was Managed; another choice stays.
- 2026-11-12: AC-15 shows when the server choice is Managed. With Automatic a vanished managed folder falls through to discovery (comfyui-connect AC-03), and the dialog's managed section knows it as missing. "Choose another server" switches to Automatic and keeps the record, so a drive that comes back works again; "Install again…" opens the wizard at the old location.
- 2026-11-12: Uninstall and Delete only ever remove a folder named "Pincel ComfyUI" (the subfolder Pincel made), never the location the user picked.
- 2026-11-12: The wizard preselects SD 1.5, the smallest checkpoint. The licenses page always shows on Install, with the ComfyUI / PyTorch notice, even when every package license was accepted before; for package changes it lists only the new licenses.
- 2026-11-12: The wizard takes the place of the Image generation dialog while open; closing it leaves a running job running, and reopening the dialog shows its progress.
- 2026-11-12: The NVIDIA driver hint (edge case) shows when the stopped server's output matches "nvidia driver", "driver version is insufficient" or "cuda driver"; "Retry start" shows for the Managed choice.
- 2026-11-12: Job commands reject with the `InstallError` object; errors of a running job arrive as the `comfyui:install` event.
- 2026-11-12: The Rust tests were written with each module; their red runs were compile failures, then `download` tests panicking with "there is no reactor running" (requests built outside Tauri's runtime) and a wrong step-order expectation, both fixed. The e2e red run used the UI before this change (`ui/src` stashed): 21 of 21 failed.
- 2026-11-12: AC-16 says English and German; the app has six UI languages since `more-languages`, and the dictionaries are typed against `en.ts`, so every string is in all six.
- 2026-11-12: Risk: GitHub does not promise byte-stable tag archives. A changed ComfyUI archive fails with "Download corrupted: ComfyUI" (AC-07) until the pin is updated.
