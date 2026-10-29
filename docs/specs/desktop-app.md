---
status: todo
created: 2026-10-29
size: M
mode: shell
design: [pincel.md §11, pincel.md §15 2026-10-26 desktop setup, pincel.md §15 2026-10-27 native menu macOS-only, docs/RELEASING.md "Self-updater (desktop)", docs/RELEASING.md "Setup and start-up (desktop)"]
---

# Desktop app (Tauri 2)

## Context

Pincel ships the same Svelte UI as a native desktop app built with Tauri 2 for Windows (NSIS setup and MSI), macOS (universal `.dmg`) and Linux (AppImage, `.deb`, `.rpm`). The desktop shell adds what a browser cannot: plain file-path reads and writes, OS file associations, a single running instance that receives files the OS hands it, a native menu bar on macOS, a per-user install on Windows and a self-install for the Linux AppImage, and a signed self-updater.

Already decided (Decision Log, `pincel.md` §15):

- **2026-10-26** — Setup and start-up as in amigo-blinzel, without resident mode or tray (closing ends the app); NSIS hooks register Capabilities / RegisteredApplications / OpenWithProgids; Linux `--install` / `--uninstall`; the window starts hidden and shows once the page has loaded (≤ 400 ms); a second start unminimizes and focuses the first.
- **2026-10-26 / 2026-10-27** — Releases via release-please; the updater checks a signed `latest.json` and verifies the minisign signature (key `3098504C1538B8F6`).
- **2026-10-27** — The native menu bar is macOS-only; Windows and Linux show the in-app menu bar alone, with a Help menu carrying About and the updater entries.

This spec records the current state of the desktop shell. It is reverse-engineered from the code.

## Scope

### In scope

- The native commands the renderer calls (`read_file_bytes`, `write_file_bytes`, `set_recent_menu`, `set_menu_language`, `updater_state`, `updater_check`, `updater_set_auto`)
- Files handed over by the OS: launch arguments, second instance, macOS open events
- File associations declared by the bundle, the Windows installer hooks and the Linux AppImage self-install
- The native macOS menu bar and its language
- The self-updater: cadence, settings, platform-specific install, Help-menu entries
- The first-launch file-association advisory dialog
- Capability detection `isTauri()` / `hasNativeMenu()`

### Out of scope

- How the editors open, save and export files (pickers, FSA, download fallback) — because that is the `file-access` spec; this spec stops at the native commands
- The in-app menu bar, Help ▸ About dialog content and keyboard map — because they belong to the `app-shell` spec and serve the web build too
- The recents list itself (IndexedDB) — because `file-access` / `app-shell` own it; this spec only covers mirroring it into the native menu
- Release pipeline, signing-key handling, winget manifests — because they are CI / release process (`docs/RELEASING.md`), not app behavior
- OS jump lists (Windows) and dock menu (macOS) for recent files — because they are named in `pincel.md` §11.3 but not built

## Acceptance criteria

- [ ] AC-01 (event): When the renderer writes bytes to a file path through the desktop app, then that file holds exactly those bytes afterwards, replacing any earlier content, and reading the path back returns the same bytes.
- [ ] AC-02 (error): If the renderer reads a file path that does not exist, then the read fails with an error message that starts with `read `.
- [ ] AC-03 (event): When the app is started with command-line arguments, then the first argument after the program that does not start with `-` is the file to open; with no such argument, no file is opened.
- [ ] AC-04 (event): When the OS hands the running app a file (launch argument, a second start with a file, or a macOS Finder / Dock open), then the app reads it from its path and opens it in the editor mode its format calls for, asking for confirmation first if the current document has unsaved changes.
- [ ] AC-05 (event, manual: needs two OS processes of an installed build): When Pincel is started while it is already running, then no second window opens; the running window is unminimized, shown and focused, and a file passed to the second start opens in it.
- [ ] AC-06 (always): Every file type the app declares as a file association — `.aseprite`, `.ase`, `.ora`, `.png`, `.jpg`, `.jpeg`, `.webp`, `.bmp`, `.gif`, `.tif`, `.tiff` — is registered by the Windows installer under its association name, and the Linux desktop entry lists exactly the declared MIME types in the declared order.
- [ ] AC-07 (event, manual: needs a real Windows install): When the Windows setup runs interactively for the first time on a machine, then it opens Settings ▸ Default apps on Pincel's page, once; later installs and upgrades, silent (`/S`) and passive (`/P`, used by the updater) installs never open it.
- [ ] AC-08 (event): When the Linux AppImage is run with `--install`, then it copies itself to `<data>/pincel/pincel.AppImage` (mode 755), links `~/.local/bin/pincel` to that copy, writes a desktop entry whose `Exec` runs the copy with `%f`, an Aseprite MIME package and icons in four sizes; running `--install` again from a newer download replaces the copy, and running it from the installed copy keeps it unchanged.
- [ ] AC-09 (event): When `pincel --uninstall` runs, then everything `--install` created is removed — copy, its directory, link, desktop entry, MIME package, icons — the downloaded AppImage stays, and uninstalling with nothing installed succeeds.
- [ ] AC-10 (error): If `--install` is run from anything other than an AppImage, or `$HOME` is not set, then the process prints an error on stderr and exits with code 1 without opening a window.
- [ ] AC-11 (state): While automatic updates are on, a background update check is due on first start, then not again until 24 hours after the last check, and immediately if the clock went back before the last check; while they are off, no background check is due.
- [ ] AC-12 (always): Automatic updates are on by default; a missing updater settings file, missing fields in it, or a damaged file read as the defaults for whatever is missing.
- [ ] AC-13 (always): Concurrent changes to the updater settings — the background check recording its time while the user toggles automatic updates — never lose a change, and the settings file is never readable half-written.
- [ ] AC-14 (event, manual: needs signed release artifacts and real installs): When an update is found, then it is downloaded and installed only if its signature verifies against the app's public key; on macOS and the installed Linux AppImage the next start runs the new version, on Windows the installer runs passively when Pincel exits. Debug builds, `.deb` / `.rpm` installs and an AppImage not installed with `--install` never update themselves.
- [ ] AC-15 (state): While Pincel runs in a browser, the Help menu offers About Pincel and no update entries.
- [ ] AC-16 (event): When the user chooses Help ▸ Check for Updates… in a desktop copy that can update itself, then the About dialog reports "up to date", the downloaded version that runs from the next start, or the error; the Install Updates Automatically checkbox always shows the value actually stored after a toggle.
- [ ] AC-17 (state, manual: needs macOS hardware; CI builds the desktop shell on Linux only): While the desktop app runs on macOS, a native menu bar shows File (New, Open…, Open Recent, Save, Save As…, Export as PNG…, Export Sprite Sheet…, Quit), Edit (Undo, Redo, Cut, Copy, Paste), View (Zoom In, Zoom Out, Reset Zoom) and Help (About Pincel, plus the update entries where the copy can update itself), and the in-app menu bar has no Help menu; on Windows and Linux no native menu bar is shown.
- [ ] AC-18 (always): The native menu's labels are German when the UI language is `de` and English for every other language code.
- [ ] AC-19 (event): When the recent-files list changes, then the native Open Recent submenu lists the recent entries that have a file path, or a disabled "(no recent files)" placeholder when there are none; choosing an entry opens that file, and choosing a menu item the mounted editor does not serve does nothing.
- [ ] AC-20 (always): Capability detection reports the desktop app exactly when the page runs inside the Tauri runtime (Tauri 2 or Tauri 1 bridge present), and reports a browser — PWA and server-side rendering included — otherwise; the native-menu check is true only for the desktop app on macOS.
- [ ] AC-21 (event): When the desktop app starts and the file-association hint was never dismissed with "Don't show this again", then a dialog explains, for the detected OS, how to make Pincel the default app for `.aseprite` / `.ase`; dismissing it with "Don't show this again" keeps it from appearing on later starts, "Got it" alone does not.

## Technical notes

Hints that drift — the code is the truth.

**Native commands** (`src-tauri/src/main.rs`, `src-tauri/src/updater.rs`), registered in `tauri::generate_handler!`:

```rust
async fn read_file_bytes(path: String) -> Result<Vec<u8>, String>          // "read {path}: {e}"
async fn write_file_bytes(path: String, bytes: Vec<u8>) -> Result<(), String> // "write {path}: {e}"
async fn set_recent_menu(app: AppHandle, items: Vec<RecentMenuItem>) -> Result<(), String> // no-op unless NATIVE_MENU
async fn set_menu_language(app: AppHandle, lang: String) -> Result<(), String>             // no-op unless NATIVE_MENU
pub async fn updater_state(app: AppHandle) -> UpdaterInfo   // { supported, autoUpdate, ready } (camelCase)
pub async fn updater_check(app: AppHandle) -> Result<Option<String>, String>  // None = up to date
pub async fn updater_set_auto(app: AppHandle, enabled: bool) -> bool         // returns the stored value
```

The paths are not scoped: both file commands accept any path the renderer passes (`std::fs::read` / `std::fs::write`). The open / save pickers come from `tauri-plugin-dialog` (`capabilities/default.json`: `core:default`, `dialog:default`). `pincel-core` is a declared dependency but not used yet.

**Events to the renderer:** `open-file` (payload: path string) from the launch argument (emitted in `setup()`, immediately), from `tauri-plugin-single-instance` and from `RunEvent::Opened` on macOS; `menu` (payload: menu item id `menu:<action>` or `recent:<id>`). `ui/src/lib/menu/index.ts` dispatches `menu:new` / `menu:open` / `recent:*` to the shell and the rest (`EditorMenuId`) to the mounted editor; `recent:_empty` is ignored. `updater:check` / `updater:auto` stay native.

**Native menu** is installed only when `const NATIVE_MENU: bool = cfg!(target_os = "macos")`. Accelerators: New `CmdOrCtrl+N`, Open `CmdOrCtrl+O`, Save `CmdOrCtrl+S`, Save As `CmdOrCtrl+Shift+S`, Export PNG `CmdOrCtrl+E`, Export Sprite Sheet `CmdOrCtrl+Shift+E`, Undo `CmdOrCtrl+Z`, Redo `CmdOrCtrl+Shift+Z`, Zoom In `CmdOrCtrl+Plus`, Zoom Out `CmdOrCtrl+-`, Reset Zoom `CmdOrCtrl+0`. `set_menu_language` rebuilds the menu and re-applies the Open Recent entries.

**`tauri.conf.json`** (verbatim where it matters): `identifier: "app.pincel.desktop"`; main window `1280×800`, min `800×600`, `"visible": false`, `"backgroundColor": "#1b1b1e"` — shown on `PageLoadEvent::Finished` or after `SHOW_DEADLINE = 400 ms`; `bundle.targets: "all"`; `bundle.windows.nsis`: `"installMode": "currentUser"`, languages English / German, `"installerHooks": "windows/hooks.nsh"`; `"createUpdaterArtifacts": true`; `plugins.updater.endpoints: ["https://github.com/amigo-labs/amigo-pincel/releases/latest/download/latest.json"]`, `plugins.updater.windows.installMode: "passive"`. File associations: Aseprite Sprite (`aseprite`, `ase`, `application/x-aseprite`), OpenRaster Image (`ora`, `image/openraster`), PNG, JPEG (`jpg`, `jpeg`), WebP, BMP, GIF, TIFF (`tif`, `tiff`), all `role: "Editor"`. The CSP allows `'wasm-unsafe-eval'` and `ipc:` / `http://ipc.localhost`.

**Updater** (`updater.rs`): `CHECK_INTERVAL_SECS = 24 * 60 * 60`, `START_DELAY = 10 s`, settings in `<app config dir>/updater.json` as `{ "auto_update": bool, "last_check": u64 }`, written to `updater.json.tmp` and renamed. A failed check counts as a check. `supported()` is false in debug builds and, on Linux, unless `$APPIMAGE` canonicalizes to the installed copy. Only one `fetch` runs at a time (async mutex); a version already downloaded in the session is reported again instead of re-downloading.

**Windows installer hooks** (`src-tauri/windows/hooks.nsh`): writes `Capabilities\FileAssociations`, `OpenWithProgids` per extension and `RegisteredApplications`; opens `ms-settings:defaultapps?registeredAppUser=Pincel` once, remembered in `DefaultAppsOffered`; an update-mode uninstall keeps the registration.

**Linux install** (`linux_install.rs`): `<data>` is `$XDG_DATA_HOME` if absolute, else `~/.local/share`; icons at `icons/hicolor/{32x32,128x128,256x256,512x512}/apps/pincel.png`; MIME package `mime/packages/pincel.xml` defines `application/x-aseprite` for `*.aseprite` / `*.ase`; `update-mime-database` / `update-desktop-database` run best-effort.

A unit test (`bundle_icons_are_rgba_pngs`) keeps every bundle icon an RGBA PNG, which Tauri's codegen requires.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Launch argument is a flag (`--debug foo.aseprite`) | Flags are skipped; `foo.aseprite` opens |
| Second start without a file | The running window comes to the front; nothing opens |
| OS hands a file while the document has unsaved changes | Discard confirmation first; the file opens only after confirming |
| Native Open Recent entry whose file is gone from the recents list | Nothing opens |
| `set_menu_language` / `set_recent_menu` on Windows or Linux | Succeed without doing anything |
| Updater settings file is damaged | Read as defaults — automatic updates on |
| Manual check while a background check is downloading | Waits for it and reports its result; no second download |
| App exits while a check is still running (Windows) | Nothing is installed; the next start checks again when due |
| `updater_check` in a copy that cannot update itself | Returns "up to date" (the menu hides the entry there) |
| `--install` run from the installed copy | Keeps the copy, rewrites link, desktop entry, MIME package and icons |
| `--uninstall` with nothing installed | Succeeds (exit 0) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The renderer's `open-file` listener is armed before the launch-argument event arrives | `setup()` emits it immediately; the code comment relies on the listener arming before first paint | A file opened by double-click on a cold start may be dropped |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `read_write_round_trip`, `write_then_overwrite` (src-tauri/src/main.rs) | built |        |
| AC-02 | `read_missing_path_errors` (src-tauri/src/main.rs) | built |        |
| AC-03 | `first_file_arg_skips_binary`, `first_file_arg_skips_flags`, `first_file_arg_none_when_only_binary` (src-tauri/src/main.rs) | built |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 | `nsis_hooks_cover_every_file_association` (src-tauri/src/main.rs), `mime_types_match_tauri_conf` (src-tauri/src/linux_install.rs) | built |        |
| AC-07 |      | open   |        |
| AC-08 | `install_creates_every_part`, `install_again_replaces_the_copy`, `install_from_the_installed_copy_keeps_it` (src-tauri/src/linux_install.rs) | built |        |
| AC-09 | `uninstall_removes_every_part`, `uninstall_without_install_is_fine` (src-tauri/src/linux_install.rs) | built |        |
| AC-10 |      | open   |        |
| AC-11 | `is_due_first_start`, `is_due_once_a_day`, `is_due_clock_went_back`, `is_due_never_when_off` (src-tauri/src/updater.rs) | built |        |
| AC-12 | `parse_settings_missing_fields_use_defaults`, `parse_settings_damaged_file_is_default` (src-tauri/src/updater.rs) | built |        |
| AC-13 | `update_settings_keeps_concurrent_toggle` (src-tauri/src/updater.rs) | built |        |
| AC-14 |      | open   |        |
| AC-15 | `the web build offers no updater entries` (ui/e2e/help.spec.ts) | built |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 | `labels_for_german_and_fallback` (src-tauri/src/main.rs) | built |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |

## Decisions

- 2026-10-29: Extracted from the code (reverse engineering). AC-15 covers only the browser; whether a desktop copy that cannot update itself hides the entries is part of the manual AC-14 because it needs a debug or package-managed build. AC-11 leaves out the 10 s start delay, which no test pins (see Technical notes).
