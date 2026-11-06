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

## Decisions

- 2026-11-06: One-click install in v1 (owner), modeled on krita-ai-diffusion's managed server without copying its GPL-3.0 code.
- 2026-11-06: v1 platforms: NVIDIA (CUDA) on Windows / Linux and Apple Silicon (MPS) (owner; reason in Context).
- 2026-11-06: Model catalog: SDXL base, SD 1.5, Flux.1 schnell, SDXL pixel-art LoRA; selectable like krita-ai-diffusion's workloads, license accepted per package (owner).
- 2026-11-06: The install goes into a "Pincel ComfyUI" subfolder of the chosen location, so Uninstall never deletes a folder the user picked (clarification pass).
- 2026-11-06: Python packages are pinned by a hashed lock file and installed with `uv`, and the Python build by its exact version and uv's checksums, so everything the install fetches is pinned, not only Pincel's own downloads (clarification pass).
