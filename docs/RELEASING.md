# Releasing Pincel

Releases work as in amigo-blinzel: **release-please** keeps a release PR
open on `main`; merging it tags the version, and `release.yml` builds the
web bundle and the desktop installers, attaches them to the draft release
and publishes it. Installed desktop apps update themselves from there.
Decision Log: spec §15, 2026-10-26.

## TL;DR

- **Write Conventional Commits** (`feat(ui): …`, `fix(pincel-core): …`,
  CLAUDE.md §2.4). `feat` / `fix` / `perf` / `revert` end up in the
  changelog and make a release; `docs`, `ci`, `chore`, `refactor`,
  `test`, `build` don't.
- **Every push to `main` opens or refreshes the release PR**
  ("chore: release pincel X.Y.Z"): the version in `Cargo.toml`,
  `ui/package.json`, `src-tauri/tauri.conf.json` and `Cargo.lock`, plus
  the new `CHANGELOG.md` entry.
- **Merge the release PR to release.** release-please tags `vX.Y.Z` and
  creates the release as a draft with the changelog; `release.yml` builds
  everything, attaches it and publishes the release.
- **Web deploy is Cloudflare's Git integration**, not this flow: site and
  editor go live from every push to `main` (see "Web deploy" below).

## Versioning

SemVer, one version for the whole workspace (`[workspace.package]` in
`Cargo.toml`; every crate inherits it). release-please derives the next
version from the commits since the last release:

| Commits since the last release | Next version |
|--------------------------------|--------------|
| a breaking change (`feat!:`, `BREAKING CHANGE:` footer) | major |
| at least one `feat` | minor |
| only `fix` / `perf` / `revert` | patch |
| only hidden types (`docs`, `ci`, `chore`, …) | no release PR |

A footer `Release-As: X.Y.Z` in a commit on `main` forces a version
(that is how 1.0.0 was set). The released version reaches the web build as
`VITE_APP_VERSION` (editor footer) and the npm package through the crate
version.

Configuration: `release-please-config.json` and
`.release-please-manifest.json` (the last released version). The manifest
started at 0.1.9, the last tag of the old push-to-release flow;
`bootstrap-sha` is the merge of PR #59.

## What runs

| Workflow / job | Runs when | Result |
|----------------|-----------|--------|
| `release-please.yml` → `release-please` | every push to `main` | Opens / updates the release PR; commits `Cargo.lock` to it; on the merge of a release PR creates the tag and the draft release |
| `release.yml` → `web` | after a release PR is merged, or by hand | Checks tag = crate version and the updater public key, runs the Rust test gate, builds the optimized wasm and the web bundle → `pincel-web.zip` |
| `release.yml` → `desktop` (3 runners) | same | `pnpm tauri build` on Windows (x64), macOS (universal) and Ubuntu 22.04 (x64); installers plus signed updater artifacts |
| `release.yml` → `publish` | all builds green | `SHA256SUMS`, the updater manifest `latest.json`, uploads everything to the draft, then publishes it as latest |
| `release.yml` → `publish-npm` | `NPM_TOKEN` set | Publishes `pincel-wasm` to npm at the release version |

The files go up before the release is published, so neither the download
links nor the updaters ever see a release without its files. If a build
fails, the release stays a draft: fix the cause and run **Actions →
Release → Run workflow** with the existing tag (e.g. `v1.0.0`). A fix that
needs a code change becomes a new release instead.

### Release files

Stable names, so `releases/latest/download/<name>` always works:

| File | What |
|------|------|
| `pincel-web.zip` | The PWA build (contains `app/`, built for `/app/`) |
| `pincel-windows-x64-setup.exe`, `pincel-windows-x64.msi` | Windows installers (NSIS, MSI) |
| `pincel-macos-universal.dmg` | macOS disk image (Apple silicon + Intel) |
| `pincel-linux-x64.AppImage`, `.deb`, `.rpm` | Linux (glibc 2.35+, Ubuntu 22.04 and newer) |
| `*.sig`, `pincel-macos-universal.app.tar.gz` | Updater artifacts and their signatures |
| `latest.json` | Updater manifest |
| `SHA256SUMS` | Checksums of every file except the signatures |

Nothing carries a code signing certificate: Windows SmartScreen and
macOS Gatekeeper warn on first launch (macOS: right-click → Open).

## Self-updater (desktop)

`src-tauri/src/updater.rs`, built on `tauri-plugin-updater`, behaves like
amigo-blinzel's updater:

- **Check:** at most once a day, 10 s after start, on a background thread.
  Source: `https://github.com/amigo-labs/amigo-pincel/releases/latest/download/latest.json`
  (only published releases are "latest").
- **Verify:** the download's minisign signature is checked against the
  public key in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`).
  Nothing unsigned is installed.
- **Swap:** macOS and the Linux AppImage replace the installed app right
  away, and the next start runs the new version. Windows keeps the
  verified installer and runs it (passive) when Pincel exits.
- **Menu:** Help → "Check for Updates…" (reports the result) and "Install
  Updates Automatically" (default on). Both are stored in `updater.json` in
  the app config directory.
- **Never:** debug builds, Linux `.deb` / `.rpm` installs (they belong
  to the package manager) and an AppImage that wasn't installed with
  `--install` (a portable download stays as it is).

Rollback after failed starts (amigo-blinzel has it) is not ported: Tauri
has no mechanism for it.

## Setup and start-up (desktop)

Also modelled on amigo-blinzel (spec §15, 2026-10-26). There is no
resident mode and no tray: closing the window ends Pincel.

- **Windows:** `pincel-windows-x64-setup.exe` (Tauri's NSIS setup,
  `bundle.windows.nsis` in `src-tauri/tauri.conf.json`):
  - installs per user, without admin rights, to
    `%LOCALAPPDATA%\Pincel`, with a Start menu shortcut and an "Installed
    apps" entry (publisher amigo-labs);
  - speaks English or German, following the system language;
  - `src-tauri/windows/hooks.nsh` registers Pincel for its file types
    (Capabilities, RegisteredApplications, Open with). Windows lets only
    the user choose defaults, so a first interactive install opens
    Settings → Default apps on Pincel's page;
  - silent with `/S`, with a progress bar but no questions with `/P`
    (winget, the updater); uninstall through Settings → Apps.
  - `packaging/winget/` holds the winget manifest templates.
- **Linux:** the AppImage is portable as downloaded.
  `./pincel-linux-x64.AppImage --install` copies it to
  `~/.local/share/pincel/pincel.AppImage`, links `~/.local/bin/pincel`
  and adds the desktop entry (with the MIME types it opens, plus an
  Aseprite MIME type) and icons; this copy updates itself.
  `pincel --uninstall` removes all of it. `.deb` / `.rpm` stay with the
  package manager.
- **macOS:** the universal `.dmg`; drag Pincel to Applications.
- **Start:** the window starts hidden on the editor's background colour
  and shows once the page has loaded (after 400 ms at the latest), so it
  never flashes white or empty. A second start hands its file to the
  running window and brings it to the front (also out of the minimized
  state).

## One-time setup

### Repository settings

- Settings → Actions → General → "Allow GitHub Actions to create and
  approve pull requests" must be on (release-please opens the release PR).
- Optional secret `RELEASE_PLEASE_TOKEN` (fine-grained PAT or GitHub App
  token with contents and pull-requests write). Without it the release PR
  is opened with the workflow's own token, and GitHub starts no CI on it.

### Updater signing key (required)

The release job refuses to run without it. The key pair exists (minisign
key ID `313783124C4CF977`, no password): its public half is
`plugins.updater.pubkey` in `src-tauri/tauri.conf.json`, the private half
lives only in the repository secret `TAURI_SIGNING_PRIVATE_KEY` (and in
the maintainer's password manager). `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
stays unset.

Keep the private key safe outside the repository. If it is lost,
installed copies accept no further updates. To replace it:

1. `pnpm --dir ui tauri signer generate --ci -w ~/.tauri/pincel.key`
   (`--ci`: no password; drop it to set one, then also set
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).
2. Store the file's content as the secret `TAURI_SIGNING_PRIVATE_KEY`.
3. Put `~/.tauri/pincel.key.pub` into `plugins.updater.pubkey` and merge
   that.
4. A new key reaches installed copies only through a release still signed
   with the old one, so ship one release with the old key that carries
   the new public key first.

### npm publish (`pincel-wasm`)

1. Create an npm automation token with publish rights for the
   `pincel-wasm` package name.
2. Add it as the `NPM_TOKEN` repo secret.

The package is published with `--access public` at the release version.

### Web deploy (Cloudflare)

No secrets needed. Cloudflare's Git integration (project `amigo-pincel`,
root `wrangler.toml`) builds every push with `scripts/cf-build.sh`: it
installs Rust + wasm-pack, builds the marketing site into
`website/build/`, builds the editor into `ui/dist/app/` and copies it to
`website/build/app/`. One Worker serves both — the site at
**https://pincel.amigo-labs.dev/** and the editor at
**https://pincel.amigo-labs.dev/app/** — and every branch preview URL
carries the editor at `/app/` too. The only one-time step is attaching
`pincel.amigo-labs.dev` as a *Custom Domain* of the `amigo-pincel`
Worker (dashboard → Workers → amigo-pincel → Settings → Domains &
Routes).

The web build uses `base: '/app/'` and writes into `ui/dist/app/`
(Workers static assets match the full request path), so the release zip
contains an `app/` folder. The Tauri build keeps the root base
(`ui/vite.config.ts` keys on `TAURI_ENV_PLATFORM`).

The wasm optimization step installs `binaryen` from apt and runs
`wasm-opt -O3` explicitly on both wasm packages. wasm-pack's own bundled
`wasm-opt` is disabled in both crates' `Cargo.toml` because it downloads
binaryen from GitHub at build time, which fails in sandboxed / firewalled
runners.
