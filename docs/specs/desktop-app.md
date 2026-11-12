---
status: todo
created: 2026-10-29
size: L
mode: shell
design: [pincel.md §11, pincel.md §15 2026-10-26 desktop setup, pincel.md §15 2026-11-11 update choice, pincel.md §15 2026-10-27 native menu macOS-only, docs/RELEASING.md "Self-updater (desktop)", docs/RELEASING.md "Setup and start-up (desktop)"]
---

# Desktop app (Tauri 2)

## Context

Pincel ships the same Svelte UI as a native desktop app built with Tauri 2 for Windows (NSIS setup and MSI), macOS (universal `.dmg`) and Linux (AppImage, `.deb`, `.rpm`). The desktop shell adds what a browser cannot: plain file-path reads and writes, OS file associations, a single running instance that receives files the OS hands it, a native menu bar on macOS, a per-user install on Windows and a self-install for the Linux AppImage, and a signed self-updater.

Already decided (Decision Log, `pincel.md` §15):

- **2026-10-26** — Setup and start-up as in amigo-blinzel, without resident mode or tray (closing ends the app); NSIS hooks register Capabilities / RegisteredApplications / OpenWithProgids; Linux `--install` / `--uninstall`; the window starts hidden and shows once the page has loaded (≤ 400 ms); a second start unminimizes and focuses the first.
- **2026-10-26 / 2026-10-27** — Releases via release-please; the updater checks a signed `latest.json` and verifies the minisign signature (key `3098504C1538B8F6`).
- **2026-10-27** — The native menu bar is macOS-only; Windows and Linux show the in-app menu bar alone, with a Help menu carrying About and the updater entries.

This spec records the current state of the desktop shell. It is reverse-engineered from the code.

**Revision 2026-11-11 — update choice.** The owner wants the user to decide how updates are handled, the same way in all amigo-labs desktop apps (amigo-blinzel `updates`, amigo-krake `desktop-app`): on the first start of a copy that can update itself, a question offers three choices — install automatically, only notify, never check. The choice can be changed later in the About dialog, which shows the version and every update option; the Install Updates Automatically checkbox in the Help menu goes away. A copy **can update itself** when it is a release build installed by the Windows setup (NSIS or MSI), a macOS app bundle, or on Linux the AppImage installed with `--install` (AC-14); debug builds, `.deb` / `.rpm` installs, an AppImage not installed and the browser build cannot. The **update choice** that applies is, in this order: the one the user confirmed in the first-start question or selected in the About dialog in this run (even if storing it failed; a selection in the question closed without Confirm is no choice, AC-23), else the stored one, else "install automatically". A later successful write of the updater settings in the same run stores it too. Decision Log `pincel.md` §15, 2026-11-11. AC-11, AC-12, AC-13, AC-15, AC-16 and AC-17 changed; AC-22 to AC-30 are new. Revision 2026-11-12 adds AC-31 to AC-33: the About dialog checks when it opens and offers Restart Now.

## Scope

### In scope

- The native commands the renderer calls (`read_file_bytes`, `write_file_bytes`, `set_recent_menu`, `set_menu_language`, `updater_state`, `updater_check`, `updater_set_auto` — which the update choice replaces with a command that stores the choice)
- Files handed over by the OS: launch arguments, second instance, macOS open events
- File associations declared by the bundle, the Windows installer hooks and the Linux AppImage self-install
- The native macOS menu bar and its language
- The self-updater: cadence, settings, platform-specific install, Help-menu entries
- The update choice: the first-start question, the "notify" announcement, and the update options in the About dialog, including the check when it opens and Restart Now
- The first-launch file-association advisory dialog
- Capability detection `isTauri()` / `hasNativeMenu()`

### Out of scope

- How the editors open, save and export files (pickers, FSA, download fallback) — because that is the `file-access` spec; this spec stops at the native commands
- The in-app menu bar, the About dialog's general content (title, tagline) and keyboard map — because they belong to the `app-shell` spec and serve the web build too; the update part of the About dialog is in scope here
- The recents list itself (IndexedDB) — because `file-access` / `app-shell` own it; this spec only covers mirroring it into the native menu
- Release pipeline, signing-key handling, winget manifests — because they are CI / release process (`docs/RELEASING.md`), not app behavior
- OS jump lists (Windows) and dock menu (macOS) for recent files — because they are named in `pincel.md` §11.3 but not built
- An update question inside the Windows installer — because macOS has no installer and Linux installs from the command line; one first-start question serves every platform alike (owner, 2026-11-11)
- Asking copies that already ran an earlier version — because they made their choice with the old checkbox; their stored on / off is kept (owner, 2026-11-11)
- Release channels, skipping a version for good, or postponing for a time span — because "notify" announces each version once and "Check for Updates…" offers it again at any time

## Acceptance criteria

- [ ] AC-01 (event): When the renderer writes bytes to a file path through the desktop app, then that file holds exactly those bytes afterwards, replacing any earlier content, and reading the path back returns the same bytes.
- [ ] AC-02 (error): If the renderer reads a file path that does not exist, then the read fails with an error message that starts with `read `.
- [ ] AC-03 (event): When the app is started with command-line arguments, then the first argument after the program that does not start with `-` is the file to open; with no such argument, no file is opened.
- [ ] AC-04 (event): When the OS hands the running app a file (launch argument, a second start with a file, or a macOS Finder / Dock open), then the app reads it from its path and opens it as a new tab of the editor mode its format calls for, without asking, also beside a tab with unsaved changes; the file of an open tab activates that tab (`document-tabs` AC-02, AC-03).
- [ ] AC-05 (event, manual: needs two OS processes of an installed build): When Pincel is started while it is already running, then no second window opens; the running window is unminimized, shown and focused, and a file passed to the second start opens in it.
- [ ] AC-06 (always): Every file type the app declares as a file association — `.aseprite`, `.ase`, `.ora`, `.png`, `.jpg`, `.jpeg`, `.webp`, `.bmp`, `.gif`, `.tif`, `.tiff` — is registered by the Windows installer under its association name, and the Linux desktop entry lists exactly the declared MIME types in the declared order.
- [ ] AC-07 (event, manual: needs a real Windows install): When the Windows setup runs interactively for the first time on a machine, then it opens Settings ▸ Default apps on Pincel's page, once; later installs and upgrades, silent (`/S`) and passive (`/P`, used by the updater) installs never open it.
- [ ] AC-08 (event): When the Linux AppImage is run with `--install`, then it copies itself to `<data>/pincel/pincel.AppImage` (mode 755), links `~/.local/bin/pincel` to that copy, writes a desktop entry whose `Exec` runs the copy with `%f`, an Aseprite MIME package and icons in four sizes; running `--install` again from a newer download replaces the copy, and running it from the installed copy keeps it unchanged.
- [ ] AC-09 (event): When `pincel --uninstall` runs, then everything `--install` created is removed — copy, its directory, link, desktop entry, MIME package, icons — the downloaded AppImage stays, and uninstalling with nothing installed succeeds.
- [ ] AC-10 (error): If `--install` is run from anything other than an AppImage, or `$HOME` is not set, then the process prints an error on stderr and exits with code 1 without opening a window.
- [ ] AC-11 (state): While the update choice that applies is "install automatically" or "notify", a background update check is due on first start, then not again until 24 hours after the last check, and immediately if the clock went back before the last check; while it is "never check", no background check is due.
- [ ] AC-12 (always): Until the user has chosen, the update choice is "install automatically"; a missing updater settings file, missing fields in it, or a damaged file read as the defaults for whatever is missing. Settings written by a version before the update choice keep their meaning: automatic updates on read as "install automatically", off as "never check".
- [ ] AC-13 (always): Concurrent changes to the updater settings — the background check recording its time or an announced version while the user changes the update choice — never lose a change, and the settings file is never readable half-written.
- [ ] AC-14 (event, manual: needs signed release artifacts and real installs): When an update is found, then it is downloaded and installed only if its signature verifies against the app's public key; on macOS and the installed Linux AppImage the next start runs the new version, on Windows the installer runs passively when Pincel exits. Debug builds, `.deb` / `.rpm` installs and an AppImage not installed with `--install` never update themselves.
- [ ] AC-15 (state): While Pincel runs in a browser, the Help menu offers About Pincel and no update entries, and the About dialog shows the version without any update option.
- [ ] AC-16 (event): When the user chooses Help ▸ Check for Updates… in a desktop copy that can update itself, whatever the update choice, then the About dialog opens and reports "up to date", the downloaded version that runs from the next start, or the error; the Help menu has no Install Updates Automatically entry.
- [ ] AC-17 (state, manual: needs macOS hardware; CI builds the desktop shell on Linux only): While the desktop app runs on macOS, a native menu bar shows File (New, Open…, Open Recent, Save, Save As…, Export as PNG…, Export Sprite Sheet…, Quit), Edit (Undo, Redo, Cut, Copy, Paste), View (Zoom In, Zoom Out, Reset Zoom) and Help (About Pincel, which opens the same About dialog as the in-app Help menu elsewhere, plus Check for Updates… where the copy can update itself), and the in-app menu bar has no Help menu; on Windows and Linux no native menu bar is shown.
- [ ] AC-18 (always): The native menu's labels are German when the UI language is `de` and English for every other language code.
- [ ] AC-19 (event): When the recent-files list changes, then the native Open Recent submenu lists the recent entries that have a file path, or a disabled "(no recent files)" placeholder when there are none; choosing an entry opens that file, and choosing a menu item the mounted editor does not serve does nothing.
- [ ] AC-20 (always): Capability detection reports the desktop app exactly when the page runs inside the Tauri runtime (Tauri 2 or Tauri 1 bridge present), and reports a browser — PWA and server-side rendering included — otherwise; the native-menu check is true only for the desktop app on macOS.
- [ ] AC-21 (event): When the desktop app starts and the file-association hint was never dismissed with "Don't show this again", then a dialog explains, for the detected OS, how to make Pincel the default app for `.aseprite` / `.ase`; dismissing it with "Don't show this again" keeps it from appearing on later starts, "Got it" alone does not.
- [ ] AC-22 (event): When a desktop copy that can update itself starts and no update choice is stored, then once the window shows — after the file-association hint (AC-21) is closed, if it shows — a question asks how Pincel should handle updates, with the choices "Install automatically (recommended)", "Notify me when an update is available" and "Never check for updates", the first preselected, and a Confirm button.
- [ ] AC-23 (event): When the user confirms the question, then the chosen option is stored and the question never shows again; when the user closes it without confirming (Escape or the close button), nothing is stored, "install automatically" applies for this run, and the question shows again at the next start.
- [ ] AC-24 (state): While the first-start question is open, no background update check starts; once it is closed, a check that came due meanwhile starts at once.
- [ ] AC-25 (always): A copy that cannot update itself — the browser build, a debug build, a `.deb` / `.rpm` install, an AppImage not installed with `--install` — never shows the first-start question, and neither does a copy whose updater settings were written by a version before the update choice (AC-12).
- [ ] AC-26 (event): When a background check finds a newer version while the update choice that applies is "notify" and that version was not announced before, then nothing is downloaded and a notice says "Pincel <version> is available" with Install and Later — a card at the bottom of the window that leaves the editor usable and takes no keyboard focus away from it; Install downloads, verifies and installs it like a manual check (AC-14) and reports the outcome in the About dialog, which opens; closing the notice with its close button, or with Escape while the focus is in the notice, counts as Later; Later, or an Install that succeeds — from the notice or from the About dialog (AC-31), which also closes the notice — marks that version as announced, so later checks do not announce it again, while a still newer version is announced. An Install that fails leaves the version unannounced, so the next due check announces it again.
- [ ] AC-27 (state): While the update choice that applies is "never check", no background check runs and nothing is announced; Help ▸ Check for Updates… still checks, downloads and installs (AC-16).
- [ ] AC-28 (state): While the About dialog is open in a desktop copy that can update itself, it shows the running version, the version that is downloaded and runs from the next start (if any), the three update choices with the one that applies selected, and a Check for Updates button; selecting a choice stores it at once and takes effect without a restart: after "never check" no background check runs in this run, after "notify" the next due check announces instead of installing, after "install automatically" a due check starts at once. On Windows the downloaded version is installed when Pincel exits rather than "from the next start", and the dialog says so. In a desktop copy that cannot update itself it shows the version and the reason instead of the choices ("Updates come from your package manager" for `.deb` / `.rpm`, "Install with `--install` to receive updates" for an AppImage not installed, nothing for a debug build, because only developers run one), and the Help menu has no Check for Updates….
- [ ] AC-29 (event, manual: needs a real Windows install): When the Windows setup runs silently (`/S`, as winget does) on a machine where Pincel has no updater settings yet, then "install automatically" is stored and the first start shows no question; interactive and passive (`/P`, the updater) installs store nothing.
- [ ] AC-30 (always): Every text of the first-start question, the notice and the About dialog's update part exists in English and German.
- [ ] AC-31 (event): When the user opens the About dialog through About Pincel — the start screen's button, the in-app Help menu's entry or the macOS menu's entry, not through Check for Updates… (AC-16) or the notice's Install (AC-26) — in a desktop copy that can update itself, while no check is running and the first-start question is not open, then a check starts at once and the dialog shows that it is checking: while the update choice that applies is "install automatically", the check downloads, verifies and installs a newer version and reports like AC-16; while it is "notify", nothing is downloaded and the dialog names the newer version with an Install button that does what Install in the notice does (AC-26) — naming it there does not count as announcing it, and closing the dialog is not Later; while it is "never check", no check starts and the Check for Updates button stays the only way to check. If a check is already running, the dialog reports that check's outcome instead of starting a second one, as its own check would under the choice that applies (a background check under "notify" still shows its notice as AC-26 says). A version already downloaded in this run is reported again at once, without asking the server. Such a check counts as a check for the 24-hour interval (AC-11), as a manual check does.
- [ ] AC-32 (event): When the About dialog shows a version that is downloaded and verified in this run and no check is running, then it offers a Restart Now button; when the user clicks it and no tab has unsaved changes, Pincel asks the desktop shell to start again after closing and closes; if a tab has unsaved changes, the unsaved-changes prompt asks first, as when the window is closed — cancelling it, or a Save in it that fails or is cancelled, leaves Pincel running with the downloaded version still waiting, and no restart is requested. A save that is running when the user clicks finishes before Pincel closes; if it fails, Pincel behaves as when the window is closed during a failing save. A check that starts while the prompt is open is waited for, and the restart then runs the version already downloaded. A second click while the prompt is open does nothing more. After the restart Pincel starts as after any quit (start screen, or the files the OS hands it).
- [ ] AC-33 (event, manual: needs signed release artifacts and real installs): When the user restarts with Restart Now (AC-32), then the window that opens runs the new version — on Windows after the installer has run passively, without the user starting Pincel by hand.

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

Restart Now (AC-32, AC-33), proposed shape: a command `updater_arm_restart` marks the downloaded update for a restart (an error when none is downloaded); the renderer then closes through the same path as the window's close request (`guardUnsaved`, then the window is destroyed). At `RunEvent::Exit` the Windows installer held for exit runs with `Update::restart_after_install(true)` (NSIS `/R`, which starts Pincel again after installing; without the mark it stays `false`), and on macOS / the AppImage, whose bundle is already replaced, `tauri::process::restart` starts the new one. No new dependency (`tauri-plugin-process` is not needed).

The paths are not scoped: both file commands accept any path the renderer passes (`std::fs::read` / `std::fs::write`). The open / save pickers come from `tauri-plugin-dialog` (`capabilities/default.json`: `core:default`, `dialog:default`). `pincel-core` is a declared dependency but not used yet.

**Events to the renderer:** `open-file` (payload: path string) from the launch argument, from `tauri-plugin-single-instance` and from `RunEvent::Opened` on macOS. An event emitted before the page listens is lost, so `src-tauri/src/open_files.rs` holds every such path until the renderer, its listener armed, calls `take_open_files`, and emits directly from then on; a page load resets that. The shell waits for those paths before it decides on the empty startup document; `menu` (payload: menu item id `menu:<action>` or `recent:<id>`). `ui/src/lib/menu/index.ts` dispatches `menu:new` / `menu:open` / `recent:*` to the shell and the rest (`EditorMenuId`) to the mounted editor; `recent:_empty` is ignored. `updater:check` / `updater:auto` stay native (`updater:auto` goes away with the update choice, AC-16).

**Native menu** is installed only when `const NATIVE_MENU: bool = cfg!(target_os = "macos")`. Accelerators: New `CmdOrCtrl+N`, Open `CmdOrCtrl+O`, Save `CmdOrCtrl+S`, Save As `CmdOrCtrl+Shift+S`, Export PNG `CmdOrCtrl+E`, Export Sprite Sheet `CmdOrCtrl+Alt+E`, Undo `CmdOrCtrl+Z`, Redo `CmdOrCtrl+Shift+Z`, Zoom In `CmdOrCtrl+Plus`, Zoom Out `CmdOrCtrl+-`, Reset Zoom `CmdOrCtrl+0`. `set_menu_language` rebuilds the menu and re-applies the Open Recent entries.

**`tauri.conf.json`** (verbatim where it matters): `identifier: "app.pincel.desktop"`; main window `1280×800`, min `800×600`, `"visible": false`, `"backgroundColor": "#1b1b1e"` — shown on `PageLoadEvent::Finished` or after `SHOW_DEADLINE = 400 ms`; `bundle.targets: "all"`; `bundle.windows.nsis`: `"installMode": "currentUser"`, languages English / German, `"installerHooks": "windows/hooks.nsh"`; `"createUpdaterArtifacts": true`; `plugins.updater.endpoints: ["https://github.com/amigo-labs/amigo-pincel/releases/latest/download/latest.json"]`, `plugins.updater.windows.installMode: "passive"`. File associations: Aseprite Sprite (`aseprite`, `ase`, `application/x-aseprite`), OpenRaster Image (`ora`, `image/openraster`), PNG, JPEG (`jpg`, `jpeg`), WebP, BMP, GIF, TIFF (`tif`, `tiff`), all `role: "Editor"`. The CSP allows `'wasm-unsafe-eval'` and `ipc:` / `http://ipc.localhost`.

**Updater** (`updater.rs`): `CHECK_INTERVAL_SECS = 24 * 60 * 60`, `START_DELAY = 10 s`, settings in `<app config dir>/updater.json` as `{ "auto_update": bool, "last_check": u64 }`, written to `updater.json.tmp` and renamed. Update choice (2026-11-11, proposed shape): a field `mode` (`"auto"` / `"notify"` / `"off"`) written only when the user confirms or changes the choice, and `announced` (the last announced version); the new version never writes `auto_update`, so a file holding `auto_update` comes from an earlier version (AC-12, AC-25), while a file with only `last_check` comes from a run whose question was closed (AC-23). `updater_state` reports the mode, whether it was ever chosen and the downloaded version; `updater_set_auto` gives way to a command that stores the mode. The silent-install case (AC-29) can be an `installerHooks` step that writes `{"mode":"auto"}` to `%APPDATA%\app.pincel.desktop\updater.json` when `$SILENT` is set and no file exists; AC-29's "no stored update choice" therefore covers a machine without the file, and a silent reinstall over a copy whose question was closed leaves the question in place. A failed check counts as a check. `supported()` is false in debug builds and, on Linux, unless `$APPIMAGE` canonicalizes to the installed copy. Only one `fetch` runs at a time (async mutex); a version already downloaded in the session is reported again instead of re-downloading.

**Windows installer hooks** (`src-tauri/windows/hooks.nsh`): writes `Capabilities\FileAssociations`, `OpenWithProgids` per extension and `RegisteredApplications`; opens `ms-settings:defaultapps?registeredAppUser=Pincel` once, remembered in `DefaultAppsOffered`; an update-mode uninstall keeps the registration.

**Linux install** (`linux_install.rs`): `<data>` is `$XDG_DATA_HOME` if absolute, else `~/.local/share`; icons at `icons/hicolor/{32x32,128x128,256x256,512x512}/apps/pincel.png`; MIME package `mime/packages/pincel.xml` defines `application/x-aseprite` for `*.aseprite` / `*.ase`; `update-mime-database` / `update-desktop-database` run best-effort.

A unit test (`bundle_icons_are_rgba_pngs`) keeps every bundle icon an RGBA PNG, which Tauri's codegen requires.

**e2e:** `ui/e2e/desktop.spec.ts` runs the shared UI on a stand-in Tauri 2 bridge (`window.__TAURI_INTERNALS__`) that answers the native commands, records the calls and emits `open-file` / `menu`; the user agent picks the OS.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Launch argument is a flag (`--debug foo.aseprite`) | Flags are skipped; `foo.aseprite` opens |
| Second start without a file | The running window comes to the front; nothing opens |
| OS hands a file while the document has unsaved changes | The file opens as a new tab without a prompt (AC-04) |
| Native Open Recent entry whose file is gone from the recents list | Nothing opens |
| `set_menu_language` / `set_recent_menu` on Windows or Linux | Succeed without doing anything |
| Updater settings file is damaged | Read as defaults — "install automatically"; the question shows on the start that reads the damaged file (AC-22), because no choice is stored |
| First start opened by double-clicking a file | The file opens; the question shows over the editor (after the file-association hint) |
| User picks "notify" in the question while a version is already out | The next due check announces it |
| Notice is open when Pincel exits | That version counts as not announced; the next due check announces it again |
| User switches from "never check" to "install automatically" in the About dialog | A check that is due (24 h passed) starts without a restart |
| Second start (single instance) while the question is open | The running window comes to the front with the question still open; no second question |
| Settings file not writable when the user confirms | The choice applies for this run; the question shows again at the next start unless a later write in this run succeeds and stores it |
| Settings file not writable when the choice changes in the About dialog | The choice applies for this run — it wins over the stored value until Pincel exits — and the dialog says it could not be saved |
| A newer version comes out while the notice for the previous one is open | The notice names the newest version; Later marks that one as announced |
| Help ▸ Check for Updates… while the notice is open | The notice closes; the About dialog reports the check; the version counts as announced if it was installed, and stays unannounced if installing failed, as for Install (AC-26) |
| User leaves "install automatically" while an update downloads or waits | That update still completes and installs as before (Windows: when Pincel exits); the new choice applies from the next check |
| Manual check (Help ▸ Check for Updates…, or Install in the notice) while a background check runs | Both share one check; if the background check did not download ("notify"), the manual check then downloads, verifies and installs itself; concurrent installs of the same version share one download. If the shared check, the download or the signature fails, the About dialog reports that error (AC-16) and nothing is installed |
| App exits while a check is still running (Windows) | Nothing is installed; the next start checks again when due |
| `updater_check` in a copy that cannot update itself | Returns "up to date" (the menu hides the entry there) |
| `--install` run from the installed copy | Keeps the copy, rewrites link, desktop entry, MIME package and icons |
| `--uninstall` with nothing installed | Succeeds (exit 0) |
| About opened while a background check runs | No second check; the dialog shows "checking" and then that check's outcome (AC-31) |
| About opened again after a check in this run found a downloaded version | The check reports that version again at once (nothing is downloaded twice); Restart Now shows (AC-32) |
| About opened while offline or the update server fails | The dialog reports the error (AC-16); no Restart Now; Pincel keeps running |
| Restart Now clicked, unsaved-changes prompt cancelled | Pincel keeps running; on Windows the update still installs when Pincel exits, elsewhere the new version runs from the next start (AC-14) |
| Restart Now, but the desktop shell holds no downloaded update (state out of sync) | The dialog reports the error; Pincel stays open and nothing restarts |
| About opened (macOS menu) while the first-start question is open | No check starts (AC-24, AC-31); the question stays open |
| Notice open when the user clicks Restart Now | The notice closes with Pincel; its version counts as not announced (as when Pincel exits) |
| Windows installer fails to start at Restart Now | Pincel has closed; the next start runs the old version and the next check finds the update again |
| About in the browser build, a debug build or a package-managed copy | No check starts (AC-15, AC-28) |

## Assumptions

None open.

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `read_write_round_trip`, `write_then_overwrite` (src-tauri/src/main.rs) | built |        |
| AC-02 | `read_missing_path_errors` (src-tauri/src/main.rs) | built |        |
| AC-03 | `first_file_arg_skips_binary`, `first_file_arg_skips_flags`, `first_file_arg_none_when_only_binary` (src-tauri/src/main.rs) | built |        |
| AC-04 | e2e `a file the OS hands over opens as a tab of the mode of its format, without a prompt (AC-04, document-tabs AC-02, AC-03)` (ui/e2e/desktop.spec.ts) — the renderer side, on a stand-in Tauri bridge; which argument becomes `open-file` is AC-03's tests, the second start and the macOS open event stay manual (AC-05, macOS hardware) | built  |        |
| AC-05 |      | open   |        |
| AC-06 | `nsis_hooks_cover_every_file_association` (src-tauri/src/main.rs), `mime_types_match_tauri_conf` (src-tauri/src/linux_install.rs) | built |        |
| AC-07 |      | open   |        |
| AC-08 | `install_creates_every_part`, `install_again_replaces_the_copy`, `install_from_the_installed_copy_keeps_it` (src-tauri/src/linux_install.rs) | built |        |
| AC-09 | `uninstall_removes_every_part`, `uninstall_without_install_is_fine` (src-tauri/src/linux_install.rs) | built |        |
| AC-10 | `install_outside_an_appimage_fails_with_an_error`, `install_without_home_fails_with_an_error`, `install_from_an_appimage_reports_on_stdout` (src-tauri/src/linux_install.rs) — run in CI (`cargo test -p pincel-tauri`; the crate needs GTK) and locally by compiling `linux_install.rs` on its own; `main` exits with that code before `tauri::Builder` runs | built  |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 | `labels_for_german_and_fallback` (src-tauri/src/main.rs) | built |        |
| AC-19 | e2e `the native Open Recent submenu follows the recents; menu items reach the shell or the editor (AC-19)` (ui/e2e/desktop.spec.ts) — the renderer side; drawing the submenu and its "(no recent files)" placeholder is macOS-only native code, checked with the manual AC-17 | built  |        |
| AC-20 | e2e `isTauri / hasNativeMenu outside a page: server-side rendering and each bridge (AC-20)`, `the Tauri 2 and Tauri 1 bridges count as the desktop app, a browser does not (AC-20)`, `the native menu bar replaces the in-app Help menu only in the desktop app (AC-20)`, `a browser on macOS keeps the in-app Help menu (AC-20)` (ui/e2e/desktop.spec.ts) | built  |        |
| AC-21 | e2e `the file-association hint shows on start until "Don't show this again" (AC-21)`, `the file-association hint gives the Windows steps (AC-21)`, `the file-association hint gives the macOS steps (AC-21)` (ui/e2e/desktop.spec.ts) | built  |        |
| AC-22 |      | open   |        |
| AC-23 |      | open   |        |
| AC-24 |      | open   |        |
| AC-25 |      | open   |        |
| AC-26 |      | open   |        |
| AC-27 |      | open   |        |
| AC-28 |      | open   |        |
| AC-29 |      | open   |        |
| AC-30 |      | open   |        |
| AC-31 |      | open   |        |
| AC-32 |      | open   |        |
| AC-33 |      | open   |        |

## Decisions

- 2026-10-29: Extracted from the code (reverse engineering). AC-15 covers only the browser; whether a desktop copy that cannot update itself hides the entries is part of the manual AC-14 because it needs a debug or package-managed build. AC-11 leaves out the 10 s start delay, which no test pins (see Technical notes).
- 2026-11-03: Test gaps closed for AC-04, AC-10, AC-16, AC-19, AC-20 and AC-21; the manual ACs (05, 07, 14, 17) stay open. The renderer-side tests (`ui/e2e/desktop.spec.ts`) run the shared UI on a stand-in Tauri 2 bridge (`window.__TAURI_INTERNALS__`, as `@tauri-apps/api/mocks` builds it) that answers the native commands, records the calls and emits `open-file` / `menu`. To test AC-10 without touching the process environment, `run_cli` now hands `$HOME`'s layout, `$APPIMAGE` and the output streams to `run_cli_with`. Red runs: AC-19 failed against the code as found (bug below); every other new test was shown red against a temporary break that was then reverted — the open-file handler skipping the discard prompt (AC-04), `--install` succeeding without `$APPIMAGE` and exiting 0 without `$HOME` (AC-10), the auto-update checkbox following the click instead of the stored value (AC-16), `isTauri` ignoring the Tauri 1 bridge and `hasNativeMenu` true for any Mac browser (AC-20), "Got it" alone remembering the dismissal and Windows detected as Linux (AC-21).
- 2026-11-03: Bug the tests found, fixed in its own commit: the Image editor did not push its recents to the native Open Recent submenu, so on macOS a file opened or saved in Image mode reached the submenu only after returning to the start screen (AC-19).
- 2026-11-06: Bug fixed: a file double-clicked while Pincel was not running opened the start screen instead of the file. `setup()` emitted `open-file` before the page existed, and Tauri events reach only armed listeners. The paths now wait in `src-tauri/src/open_files.rs` until the renderer calls `take_open_files` (unit tests `offer_before_take_queues_path`, `offer_after_take_emits_directly`, `reset_queues_again_until_next_take`); e2e `a file passed at launch, before the page listens, opens instead of the start screen (AC-04)` (ui/e2e/desktop.spec.ts) was red before the fix.
- 2026-11-05: AC-04 reworded for `document-tabs` (AC-02, AC-03): a handed-over file opens as a tab without the discard prompt, which no longer exists — because a new tab replaces nothing. Also found with the merged tests: under a Tauri 1 bridge `getCurrentWindow()` threw at start and the file-association advisory never showed (AC-20); the shell now starts those listeners inside a promise.
- 2026-11-11: Revised for the update choice (owner): the user picks on the first start how updates are handled — install automatically, notify, never check — and changes it in the About dialog, which shows the version and every update option; the Help menu's checkbox goes away. AC-11, AC-12, AC-13, AC-15, AC-16 and AC-17 reworded and back to `open` (their former tests cover the old wording: `is_due_*`, `parse_settings_*`, `update_settings_keeps_concurrent_toggle`, `the web build offers no updater entries`, e2e `Help ▸ Check for Updates… reports …`); AC-22 to AC-30 new. Owner's answers: ask on the first start, not in the installer; three choices; nobody asked → automatic (silent installs, copies from earlier versions); "notify" announces each version once; closing the question without a choice asks again at the next start; the question comes after the file-association hint. The same feature lands in amigo-blinzel (`updates`) and amigo-krake (`desktop-app`) with the same three choices and texts.
- 2026-11-11, clarification pass: "can update itself" defined once (the Revision paragraph); AC-26 reports in the About dialog and a failed Install leaves the version unannounced, because the user asked for it and should be offered it again; AC-28 names what each switch changes in the running app and that Windows installs on exit; AC-29 means "no settings file yet", because the installer cannot read JSON; an update already downloading or waiting when the user leaves "install automatically" still installs, because cancelling a half-done install is riskier than finishing it. Stale edge case of AC-04 corrected. Reasons for the owner's defaults: automatic when nobody answered, because silent updates keep most copies current and that was the behavior before; the question after the file-association hint, because the owner chose that order. AC-12 / AC-13 keep their storage wording from the extraction (unchanged behavior).
- 2026-11-11, from the review of the same feature in amigo-krake#111 and amigo-blinzel#63: the update choice that applies is the one made in this run (even when storing it failed), else the stored one, else "install automatically" — AC-11, AC-26 and AC-27 now say "the update choice that applies", and AC-28 selects it in the About dialog — because otherwise a choice that "applies for this run" after a failed save would lose to the stored value. A manual check or Install that waits on a background check still downloads, verifies and installs when that check only announced the version ("notify"), because a check alone cannot satisfy a request to install; concurrent installs of the same version share one download.
- 2026-11-12: Revised (owner): opening the About dialog checks for updates, and a version downloaded in this run can be started at once with Restart Now — because the owner wants to see and take an update where the version is shown, without waiting for the next start. AC-31 to AC-33 new; built together with AC-22 to AC-30 (owner), since they share the About dialog's update part. Owner's answer: under "never check" opening the dialog does not check — only the Check for Updates button does — because "never" must not cause network traffic the user did not ask for. Under "notify" opening the dialog checks but does not download, and offers Install as the notice does — because "notify" means nothing is installed without the user's say (AC-26); this follows from AC-26, not from an owner answer. Restart Now goes through the unsaved-changes prompt, because it closes every tab as closing the window does.
- 2026-11-12, clarification pass: AC-31 covers only opening About Pincel, because Check for Updates… and the notice's Install already ask to download whatever the choice (AC-16, AC-26, AC-27); naming a version in About is not an announcement and closing About is not Later, because only the notice and its Later are the user's answer to an announcement (AC-26); an About check counts for the 24-hour interval, as every check does. No check while the first-start question is open, because AC-24 holds back every check until the choice is made. Restart Now shows only while no check runs, so the dialog never offers a restart while it still says it is checking; a failed or cancelled Save in the prompt requests no restart, as it closes no window. Second pass: Escape closes the notice only while the focus is in it, because it takes no focus from the editor and Escape there keeps its editor meaning; an Install that succeeds from About also closes the notice for that version and counts as announced, because the user took the offer; a check in the same run reports an already downloaded version again without asking the server, so a later check never replaces it before the restart (Technical notes, Updater); a check that starts while the unsaved-changes prompt is open is waited for, and the restart then runs the version already downloaded; a running save that fails at Restart Now behaves as at a window close (unsaved-changes-prompt). The notice is a card at the bottom that leaves the editor usable, and closing it counts as Later (owner) — because an update offer must not interrupt drawing, and a dismissed offer should not come back every day. Reasons for older defaults that had none: the first-start question follows the file-association hint so the two first-start dialogs never stack; a question closed without Confirm asks again, because closing is no answer; a debug build shows no reason, because only developers run one.
