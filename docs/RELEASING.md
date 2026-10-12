# Releasing Pincel

Every push to `main` publishes a new version automatically. This document
describes what happens, the decisions behind it, and the one-time setup
required to enable the optional (guarded) steps.

## TL;DR

- **Push to `main` → new release.** The `Release` workflow
  (`.github/workflows/release.yml`) computes the next version, tags it,
  and creates a GitHub Release with auto-generated notes and the built
  web (PWA) bundle attached.
- **Web deploy is Cloudflare's Git integration**, not this workflow:
  site and editor go live from the same push (see "Web deploy" below).
- **npm publish needs opt-in.** It only runs if the `NPM_TOKEN` repo
  secret is set; otherwise the job is skipped.
- **Desktop binaries are manual** (`Release Desktop` workflow), because a
  native 3-OS build is slow and the macOS icon is not in place yet.

## Versioning

Tag-based SemVer. The **git tag is the source of truth** — the workflow
never commits version changes back to `main`, so it can't trigger itself.

- A push to `main` bumps the **patch** level of the latest `v*` tag.
- A manual run (Actions → Release → *Run workflow*) lets you choose
  `patch` / `minor` / `major`.
- The first release starts at **`v0.1.0`** (matches the crate versions).

The computed version is passed to the web build as `VITE_APP_VERSION`
and shown in the editor footer; released npm packages are stamped with it
at publish time.

> Note: the repo uses `[crate] action` commit messages, not Conventional
> Commits, so bump level is push=patch / manual=your choice rather than
> inferred from commit text. If you later adopt `feat:` / `fix:` prefixes,
> the compute-version step can be upgraded to read them.

## What each job does

| Job | Runs when | Needs | Result |
|-----|-----------|-------|--------|
| `preflight` | always | — | Computes the next version; checks which secrets exist |
| `release` | always | `GITHUB_TOKEN` (built-in) | Runs the Rust test gate, builds the optimized wasm + web bundle, creates the tag + GitHub Release, attaches `pincel-web-<version>.zip` |
| `publish-npm` | `NPM_TOKEN` set | npm secret | Publishes `pincel-wasm` to npm at the release version |

The wasm optimization step installs `binaryen` from apt and runs
`wasm-opt -O3` explicitly on both wasm packages (`pincel-wasm` for Pixel
mode, `pincel-image-wasm` for Image mode). wasm-pack's own bundled `wasm-opt`
is disabled in both crates' `Cargo.toml` because it downloads binaryen from
GitHub at build time, which fails in sandboxed/firewalled runners.

## One-time setup to enable the guarded steps

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

### npm publish (`pincel-wasm`)

1. Create an npm automation token with publish rights for the
   `pincel-wasm` package name.
2. Add it as the `NPM_TOKEN` repo secret.

The package is published with `--access public` at the release version.

### Desktop binaries (Tauri)

Manual: Actions → *Release Desktop* → *Run workflow*, passing an existing
release tag (e.g. `v0.1.0`). Installers for Linux / macOS / Windows are
attached to that release. The macOS `icon.icns` is committed, so all
three legs have their icons; the first manual run is the verification
(the dev env here is headless). Optional: add signing/notarization
secrets for shippable binaries — unsigned builds compile but warn on
first launch.

## Changing the defaults

- **Different host** (GitHub Pages, a subdomain): replace the
  Cloudflare build (`scripts/cf-build.sh`, root `wrangler.toml`). The
  editor is built for `/app/` (Vite `base`, PWA `scope` / `start_url`),
  so a host that serves it elsewhere needs those adjusted.
- **Don't auto-release every push**: change `release.yml`'s `on:` to
  `workflow_dispatch` only, or trigger on tag pushes.
