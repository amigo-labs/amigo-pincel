# Releasing Pincel

Every push to `main` publishes a new version automatically. This document
describes what happens, the decisions behind it, and the one-time setup
required to enable the optional (guarded) steps.

## TL;DR

- **Push to `main` → new release.** The `Release` workflow
  (`.github/workflows/release.yml`) computes the next version, tags it,
  and creates a GitHub Release with auto-generated notes and the built
  web (PWA) bundle attached.
- **Nothing outward-facing happens without opt-in.** Web deploy and npm
  publish only run if you add the matching repo secret; otherwise those
  jobs are skipped (they never fail).
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
| `deploy-web` | `CLOUDFLARE_API_TOKEN` set | Cloudflare secrets | Deploys `ui/dist` to the `pincel-app` Worker (config: `ui/wrangler.toml`) |
| `publish-npm` | `NPM_TOKEN` set | npm secret | Publishes `pincel-wasm` to npm at the release version |

The wasm optimization step installs `binaryen` from apt and runs
`wasm-opt -O3` explicitly. wasm-pack's own bundled `wasm-opt` is disabled
in `crates/pincel-wasm/Cargo.toml` because it downloads binaryen from
GitHub at build time, which fails in sandboxed/firewalled runners.

## One-time setup to enable the guarded steps

### Web deploy (Cloudflare)

The marketing site already deploys via Cloudflare's Git integration
(root `wrangler.toml`, project `amigo-pincel`). The **editor** deploys as
a separate Worker project (`pincel-app`, `ui/wrangler.toml`). To enable:

1. Create a Cloudflare API token with *Workers Scripts: Edit* permission.
2. Add repo secrets:
   - `CLOUDFLARE_API_TOKEN`
   - `CLOUDFLARE_ACCOUNT_ID`
3. (Optional) Map a custom domain to the `pincel-app` Worker.

Until then the editor is available only as the release's downloadable
bundle. The marketing site's `/app` link still points at a placeholder
(product decision — see STATUS.md) until the editor's public URL is
decided.

### npm publish (`pincel-wasm`)

1. Create an npm automation token with publish rights for the
   `pincel-wasm` package name.
2. Add it as the `NPM_TOKEN` repo secret.

The package is published with `--access public` at the release version.

### Desktop binaries (Tauri)

Manual: Actions → *Release Desktop* → *Run workflow*, passing an existing
release tag (e.g. `v0.1.0`). Installers for Linux / macOS / Windows are
attached to that release. Before it passes cleanly on macOS:

- Generate the icon set and commit `src-tauri/icons/icon.icns`:
  `pnpm --dir ui exec tauri icon public/favicon.svg`.
- (Optional) add signing/notarization secrets for shippable binaries;
  unsigned builds compile but warn on first launch.

## Changing the defaults

- **Different host** (GitHub Pages, a subdomain): swap the `deploy-web`
  job. GitHub Pages needs the app built with a matching Vite `base` and
  the PWA `scope`/`start_url` adjusted, since the PWA is currently
  configured for root-origin hosting.
- **Don't auto-release every push**: change `release.yml`'s `on:` to
  `workflow_dispatch` only, or trigger on tag pushes.
