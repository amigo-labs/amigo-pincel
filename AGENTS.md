# AGENTS.md — Pincel Implementation Guide

This file is the operational contract between Claude Code and the Pincel codebase. It exists to keep work efficient, predictable, and **scoped small enough that no single session runs into context limits or timeouts**.

Read this entire file before starting any task. Re-read Section 3 ("Session Management") at the start of every session.

---

## 1. Project Overview

Pincel is a pixel-art and image editor in one app: a pixel-art editor at the feature level of Aseprite, focused on game asset creation, and — since M16 — a paint.net-style image editor for photos, screenshots, mockups and artwork. It ships as a PWA and a native Tauri app sharing one Svelte UI and two Rust cores: `pincel-core` behind **Pixel mode** and `pincel-image-core` (the former sister project `amigo-fineliner`) behind **Image mode**. The mode follows the document (`.aseprite` → Pixel; PNG / JPEG / WebP / BMP / GIF / TIFF → Image). The two cores stay separate on purpose — see the spec's Decision Log, 2026-09-06 "Two cores, one shell". `crates/pincel-effects` serves both.

The full design specification lives in **`docs/specs/pincel.md`**. That document is the source of truth for *what* to build. This file is the source of truth for *how* to build it.

**Always read the relevant section of `docs/specs/pincel.md` before starting an implementation task.** The spec is sectioned and indexed; load only what's needed.

---

## 2. Workflow Philosophy

### 2.1 Spec-Driven

Every implementation task traces back to a specific section of `docs/specs/pincel.md`. If a task implies a design decision not in the spec, **stop and update the spec first** with a Decision Log entry. Do not improvise architecture.

New features and behavior changes additionally get a **feature spec** `docs/specs/<name>.md` with testable acceptance criteria, written with `/spec`, built with `/spec-build`, accepted with `/spec-check`. Triage, the two kinds of spec and where knowledge goes: `.claude/rules/specs.md`. Bug fixes, refactorings, tooling and docs need no feature spec.

### 2.2 Compiler-as-Gatekeeper

For Rust work, the compiler is the primary verification tool. The loop is:

1. State the change in plain English (one sentence)
2. Make the smallest code change that could possibly satisfy it
3. `cargo check -p <crate>` — must pass
4. `cargo test -p <crate>` — must pass for changed module
5. `cargo clippy -p <crate> -- -D warnings` — must pass
6. Commit

If step 3 fails, **fix it immediately** before moving on. Never accumulate compiler errors. Never proceed with a "I'll fix that later" attitude.

### 2.3 Test-First Where Sensible

Codecs (`aseprite-writer`, `aseprite-read` adapter), command/undo logic, and `compose()` are test-first: write the test fixture and assertion, run it red, implement, run it green.

Tools, UI components, and rendering adapters are test-after: ship the implementation, then add interaction tests. UI testing is expensive; don't over-invest before the design is stable.

### 2.4 Micro-commits

One concept per commit. Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): description`, the scope being the crate or area (`pincel-core`, `aseprite-writer`, `pincel-wasm`, `ui`, `tauri`, `website`, `spec`, …):

```
feat(pincel-core): add Sprite struct with Layers and Frames
feat(aseprite-writer): implement Layer Chunk (0x2004) write
fix(ui): keep the Pencil stroke when the pointer leaves the canvas
docs(spec): Decision Log entry for the updater
```

The type drives releases (release-please, `docs/RELEASING.md`): `feat` and `fix` (and `perf`, `revert`) go into `CHANGELOG.md` and start a release; `docs`, `refactor`, `test`, `build`, `ci` and `chore` don't. A breaking change (`feat!:` or a `BREAKING CHANGE:` footer) bumps the major version — only on purpose.

If a commit message needs an "and" between two unrelated changes, split it.

---

## 3. Session Management

**This is the most important section.** The codebase is large enough that a careless session can run out of context mid-task and leave the repo in a broken state. Follow these rules to avoid that.

### 3.1 Pre-Flight Checklist

Before writing any code in a new session, run through this checklist:

```
[ ] git status is clean (or I have a plan for the dirty state)
[ ] cargo check passes from a clean checkout
[ ] I have read the relevant docs/specs/pincel.md section
[ ] I can state the task in one sentence
[ ] I can estimate complexity: T-shirt size XS / S / M / L
[ ] If L: STOP. Split into smaller tasks. Do not start.
```

If any line is unchecked, address it before writing code.

### 3.2 Task Sizing

| Size | Definition | Action |
| ------ | ------------ | -------- |
| **XS** | 1 file changed, ≤30 lines | Do it inline, no ceremony |
| **S** | 1–3 files changed, ≤150 lines | Single session, single commit |
| **M** | 3–6 files, ≤400 lines, multiple commits | Single session, plan commits up front |
| **L** | 6+ files, or new module / new crate | **Split into M tasks** before starting |
| **XL** | New phase milestone | **Split into L tasks**, write a sub-spec |

A session should produce one M task or two-to-three S tasks. Never a single L task.

### 3.3 Stopping Points

Stop and yield to human review at any of these triggers:

- A milestone exit criterion (Spec Section 16) is met
- A new public API surface is added (functions/types in `lib.rs` or `mod.rs` exports)
- A new dependency is added to `Cargo.toml` or `package.json`
- The spec needs a Decision Log entry
- Three consecutive `cargo check` failures with the same root cause
- A test was disabled or `#[ignore]`d
- Any `unsafe` block is added
- The session has produced ~400 lines of changes and a coherent stopping point exists

When stopping, the state lives in the repository, not in a status file:

- Feature work: the spec's `## Evidence` (what is built), `## Open questions` (what blocks) and `## Decisions`
- Anything else: the commit message says what was completed and what comes next

### 3.4 Resume Protocol

When starting a session that continues prior work:

1. `git log --oneline -10` to see recent commits
2. Read the open feature specs the SessionStart hook lists (order: `docs/specs/README.md`)
3. Read the last commit message and diff
4. Run `cargo check && cargo test` — must be green before proceeding
5. State the next task explicitly
6. Run the Pre-Flight Checklist (§3.1)

If `cargo check` is red on a fresh checkout, **fix the build first**. Do not work on top of a broken state.

### 3.5 Context Discipline

When working in this repo:

- **Read minimally.** Don't load entire files when one function will do. Use `grep` / `rg` for navigation.
- **Don't re-read what you've just written.** Trust your own output for the duration of one session.
- **Drop irrelevant files from context.** If you opened `pincel-core/src/document.rs` to check a type and you're now in `aseprite-writer`, you don't need that file anymore.
- **Don't echo specs back.** When implementing a section, don't paraphrase the spec into the code as comments. Reference it: `// See docs/specs/pincel.md §3.2`.

---

## 4. Implementation Order

Milestones M1–M16 are done; their history is in git and in the Decision Log (`docs/specs/pincel.md` §15).

New work comes as feature specs (`docs/specs/<name>.md`, `.claude/rules/specs.md`). Their order, dependencies and the backlog of gaps not yet written as specs live in `docs/specs/README.md`. Open items that are not code — release secrets, checks on real hardware — are listed there too.

A spec is at most size L (§3.2); an XL feature is split into several specs before work starts.

---

## 5. Crate Conventions

### 5.1 `pincel-core`

- **No platform dependencies.** No `tokio`, no `wasm-bindgen`, no `web-sys`, no `tauri`. Pure logic.
- **No file I/O.** Codec functions take `Read` / `Write` trait objects.
- **`std`-only is acceptable.** Don't waste time on `no_std` unless explicitly requested.
- **Errors:** `thiserror` for crate-level error types. One enum per submodule (`DocumentError`, `CodecError`, `ToolError`, …).
- **Public API:** re-exported from `lib.rs` only. Internal modules are `pub(crate)`.
- **Modules per concept:**

  ```
  src/
    lib.rs              re-exports
    document/           Sprite, Layer, Frame, Cel, Palette, …
    tools/              Tool trait, individual tool impls
    command/            Command trait, command bus, undo stack
    codec/              aseprite_read.rs, aseprite_write.rs, png.rs
    render/             compose(), blend modes, indexed→rgba
    geometry/           Rect, Point, basic geometry helpers
  ```

### 5.2 `aseprite-writer`

- **Independent.** No `pincel-core` dependency. Mirrors `aseprite-loader`'s data model so a user could go loader → writer without touching Pincel types.
- **License:** `MIT OR Apache-2.0` — match `aseprite-loader`. README must include the disclaimer about Aseprite trademark.
- **Format reference:** `https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md` — link in module docs.
- **Test fixtures:** `tests/fixtures/*.aseprite` — small, hand-crafted files covering each feature variant.
- **Round-trip test convention:** every fixture has a `<fixture>_roundtrip.rs` test that reads-writes-reads and asserts equality.

### 5.3 `pincel-wasm` and `pincel-image-wasm`

- **Crate type:** `cdylib`. Built via `wasm-pack build --target web --release` (`pnpm wasm:build` in `ui/` builds both).
- **Public API:** `pincel-wasm` matches the npm-package surface in spec §17.5. Keep it small and stable. It is published to npm as `@amigo-labs/pincel-wasm` (`wasm-pack --scope amigo-labs`), which is the name `ui/` links and imports; the crate and its directory keep the bare name. `pincel-image-wasm` is handle-based with a JSON `CommandSpec` / `EffectSpec`; its TypeScript mirror under `ui/src/modes/image/core/generated/` is generated by `cargo test -p pincel-image-wasm export_command_spec` and committed (CI fails on drift).
- **Memory:** state owned in Rust. JS receives `Uint8ClampedArray` views, never owned copies, when possible.
- **Build output:** `pkg/` — gitignored, generated.
- **Image-mode core conventions** (`pincel-image-core`): same rules as §5.1 — no platform deps, no system-level file I/O, `thiserror` errors, `std`-only. The `image` crate is its single codec dependency.

### 5.4 `ui/`

- **Framework:** Svelte 5 with runes (`$state`, `$derived`, `$effect`).
- **Styling:** Tailwind 4 utility classes; complex components via shadcn-svelte (copied into `src/lib/components/ui/`, not a runtime dep).
- **TypeScript strict mode.** No implicit `any`. No `// @ts-ignore` without an issue link.
- **State stores:** Svelte 5 runes in `.svelte.ts` files. Editor state lives with its mode — `src/modes/<mode>/stores/editor.svelte.ts`, one `$state` object per concern (document, view, tools, colors, dialogs, …); logic that mutates it sits in plain modules beside the editor component (`modes/pixel/{render,pointer,doc-ops,file-ops,…}.ts`, `modes/image/core/controller.ts`). State both editors share is a helper under `src/lib/` (e.g. `lib/idb/autosave-session.svelte.ts`, `lib/i18n`). Components keep markup, keyboard wiring and mount / unmount only.
- **WASM bridge:** all Pixel-mode calls go through the `src/lib/core/` adapter and all Image-mode calls through `src/modes/image/core/wasm.ts`; UI never imports `pincel-wasm` / `pincel-image-wasm` directly.
- **Strings:** every user-visible string goes through `t()` from `src/lib/i18n/` (spec §9.5); add the key to `en.ts` and `de.ts` together. Document content (layer / tag / slice names) is never translated.
- **One editor frame:** both editors render through `src/lib/components/layout/EditorFrame.svelte` (menus as data, tool groups, `Panel`-wrapped panels) and style with the `pc-*` theme tokens from `app.css` — no raw `neutral-*` colors, no emoji icons. Keys come from `src/lib/shell/shortcuts.ts`.
- **Two modes, one shell:** `src/App.svelte` owns the start screen, the New dialog, file → mode routing (`src/lib/fs` `sniffFormat`) and mounts exactly one editor per session — `src/modes/pixel/PixelEditor.svelte` or `src/modes/image/ImageEditor.svelte` — via dynamic `import()`. Editors talk to the shell only through `EditorProps` (`src/lib/shell/types.ts`). Shared layers (`src/lib/fs`, `src/lib/idb`, `src/lib/menu`, `src/lib/platform`) stay mode-agnostic.

### 5.5 `src-tauri/`

- **Tauri 2.** Don't introduce v1 patterns.
- **`pincel-core` is a direct cargo dependency.** No WASM round-trip for native FS.
- **Commands** are thin wrappers: deserialize args, call into core, serialize result. No business logic in command handlers.

---

## 6. Code Style

### 6.1 Rust

- Edition: latest stable in workspace `Cargo.toml`
- Formatter: `cargo fmt` with default config
- Linter: `cargo clippy -- -D warnings` — must be clean
- No `unwrap()` outside tests. Use `?` or `.expect("documented invariant")`.
- No `unsafe` without a `// SAFETY:` comment justifying it. PR must call out new `unsafe` in description.
- Prefer `&[u8]` over `Vec<u8>` for function args when ownership isn't needed.
- Doc comments on every public item. `///` not `//`.

### 6.2 TypeScript

- Strict mode, all flags on
- ESLint config matches the amigo-labs convention (see existing repos)
- No `any`. Use `unknown` and narrow.
- Imports: `import type { … }` for type-only imports
- File naming: `kebab-case.ts` for utilities, `PascalCase.svelte` for components

### 6.3 Markdown / Specs

- Headings sentence case
- Code blocks always have a language tag
- Tables for any structured comparison
- One blank line above and below code blocks and tables

### 6.4 Line endings

`.gitattributes` at repo root owns line endings. `* text=auto eol=lf` means:

- **LF in the index and LF in the working tree, on every platform.** Not "LF in
  the repo, CRLF on Windows" — both sides match byte-for-byte, so a checkout can
  never produce a whitespace-only diff.
- **`core.autocrlf` and `core.eol` are irrelevant here.** Don't set them, don't
  ask contributors to set them, and never resolve line-ending noise by changing a
  git config: attributes override both, and a config change only fixes the one
  machine it ran on. (The Windows Git installer writes `core.autocrlf=true` into
  the *system* config, which is exactly the kind of invisible per-machine state
  this file removes.)
- **Binary types are listed explicitly** in `.gitattributes` (`binary` =
  `-text -diff`). Add a new binary extension to that list in the same commit
  that first adds such a file — `.aseprite` fixtures above all: content
  detection is a heuristic, and a line-ending-mangled fixture is a silently
  failing test, not a compile error.

If `git status` ever reports a batch of files as modified with equal insertion
and deletion counts and no visible content change, the working tree has drifted
from the attributes. Re-apply them:

```bash
git rm --cached -r . && git reset --hard
```

That empties the index so every entry counts as new, then rewrites the working
tree through the attribute filters. It discards uncommitted work — commit or
stash first. `git checkout-index -a -f` does *not* do the job: it skips entries
whose cached stat still matches, which is every entry in exactly this situation.

---

## 7. Testing Requirements

### 7.1 Unit Tests

Inline `#[cfg(test)] mod tests { ... }` in the same file as the code under test. Name tests `<function>_<scenario>_<expected>`:

```rust
#[test]
fn add_layer_at_top_increases_layer_count() { … }

#[test]
fn add_layer_with_existing_name_returns_err() { … }
```

### 7.2 Integration Tests

`tests/` directory at crate root. Use these for cross-module flows: load → modify → save round-trips, command/undo sequences across modules.

### 7.3 Property-Based Tests

`proptest` for codecs and pixel-level operations. Strategy:

- For `aseprite-writer`: generate random valid `AseFile`, write, read back via `aseprite-loader`, assert structural equality
- For `compose()`: assert idempotency of identity operations, associativity of layer ordering, etc.

### 7.4 UI Tests

Playwright (`@playwright/test`, set up 2026-10-11) under `ui/e2e/`, run against the built bundle via `vite preview` at `/app/` (`ui/playwright.config.ts`). Shared helpers (start screen, New dialog, canvas drags) live in `ui/e2e/helpers.ts`. Each tool has a happy-path interaction test minimum; every UI block adds at least one spec. Tests may inspect the DOM and screenshots; app code still never reads canvas pixels (§9).

### 7.5 Test Performance

Tests run in CI on every push. Keep the full test suite under 60 seconds. If a test is slow, mark it `#[ignore]` and add it to a separate `cargo test --ignored` job.

---

## 8. Branch & PR Conventions

- `main` is protected; all changes via PR
- Branch names: `feat/<short-desc>`, `fix/<short-desc>`, `chore/<short-desc>`, `spec/<short-desc>`
- PR title matches commit message convention: `type(scope): description`
- PR description includes:
  - Spec section reference (e.g., "Implements §5.2 Pencil Tool")
  - Test evidence (test names that cover the change)
  - Any open questions for review
- Before requesting review, the PR must be green in CI

---

## 9. Forbidden Patterns

These produce immediate revert. No exceptions.

- **`unwrap()` in non-test code.** Always `?` or documented `expect`.
- **Reading pixel data from a `<canvas>` element.** Document state lives in Rust; canvas is render-only. See spec §17.1.
- **`localStorage` / `sessionStorage` in the UI.** Use IndexedDB for persistence.
- **New runtime dependencies without explicit approval.** A new entry in `Cargo.toml` `[dependencies]` or `package.json` requires either a Decision Log entry or stop-and-ask.
- **`// @ts-ignore` / `#[allow(clippy::…)]` blanket suppressions.** Fix the underlying issue or document why the suppression is correct in a comment.
- **Disabling a test instead of fixing it.** If a test must be disabled, file an issue and link it from the `#[ignore]` attribute.
- **Mixing Phase 1 and Phase 2 features.** If a task needs Phase 2 functionality, stop and re-scope.
- **Touching `aseprite-writer` and the Pincel app in the same commit.** Separate concerns, separate commits.

---

## 10. Common Commands

Quick reference. All commands run from repo root unless noted.

### Rust workspace

```bash
cargo check                                  # all crates (pincel-tauri needs GTK/WebKit libs)
cargo check -p pincel-core                   # one crate
cargo test -p pincel-core
cargo clippy -p pincel-core -p pincel-effects -p aseprite-writer -p pincel-wasm -p pincel-image-core -p pincel-image-wasm --all-targets -- -D warnings
cargo fmt
cargo doc --workspace --no-deps --open

# `--workspace` clippy/test include pincel-tauri, which needs the GTK /
# WebKit system libraries — absent from most dev containers. CI checks
# pincel-tauri in a dedicated job; scope local clippy/test to the six
# library crates as above.

# Image mode: the TypeScript mirror of pincel-image-wasm's CommandSpec /
# EffectSpec is generated by a test and committed; CI fails on drift.
cargo test -p pincel-image-wasm export_command_spec   # writes ui/src/modes/image/core/generated/
```

### WASM

```bash
cd crates/pincel-wasm
wasm-pack build --target web --release       # produces pkg/
```

### UI

```bash
cd ui
pnpm wasm:build                              # build both wasm pkgs (pincel-wasm, pincel-image-wasm) — required before install
pnpm install
pnpm dev                                     # Vite dev server
pnpm build                                   # production bundle
pnpm check                                   # svelte-check (type-check)
pnpm lint
pnpm test:e2e                                # Playwright, needs `pnpm build` first (§7.4)
```

UI unit tests (Vitest) are not set up yet. Locally Playwright uses the
preinstalled Chromium (`PLAYWRIGHT_BROWSERS_PATH`); CI runs
`pnpm exec playwright install --with-deps chromium`.

### Tauri

```bash
cd ui
pnpm tauri:dev                               # native dev
pnpm tauri:build                             # release binary
```

### Full pre-commit gate

```bash
cargo fmt && cargo clippy -p pincel-core -p pincel-effects -p aseprite-writer -p pincel-wasm -p pincel-image-core -p pincel-image-wasm --all-targets -- -D warnings && cargo test -p pincel-core -p pincel-effects -p aseprite-writer -p pincel-wasm -p pincel-image-core -p pincel-image-wasm && cd ui && pnpm lint && pnpm check && pnpm build && pnpm test:e2e
```

If any step fails, do not commit.

---

## 11. Definition of Done

A task is done when **all** of the following hold:

- [ ] Code change is minimal and matches the spec
- [ ] `cargo check` and `cargo test` pass for the affected crate(s)
- [ ] `cargo clippy -- -D warnings` is clean
- [ ] New public API has doc comments
- [ ] New behavior has at least one test
- [ ] Spec references are updated if the design shifted
- [ ] Commit message follows the convention
- [ ] No forbidden patterns introduced (§9)

For UI tasks add:

- [ ] `pnpm lint` clean
- [ ] `pnpm build` succeeds
- [ ] Visually verified in dev server

For feature-spec work add:

- [ ] `/spec-check` verdict is `READY` and the spec is `status: implemented`
- [ ] `docs/specs/README.md` lists the spec under "Done"
- [ ] Demo recorded or screenshotted in PR description

---

## 12. When to Stop and Ask

Some situations warrant stopping the session and asking for human input. **It is always better to stop than to guess.**

Stop and ask when:

- The spec is silent or ambiguous on a question that affects the public API
- Two reasonable implementations exist and the choice is not local
- A new dependency is needed
- The task as written would take more than one session
- A test is failing for a reason that suggests the spec is wrong
- An external API behavior contradicts the spec assumption
- Performance work requires architectural changes (cross-crate boundaries)

Do not stop for:

- Style choices that don't affect API
- Local refactors that improve clarity
- Adding tests beyond the requirement
- Documentation improvements
- Renames within a single module

---

## 13. Skills Directory

Project-specific skills live under `.claude/skills/`. When a skill exists for a task type, follow it.

| Skill | Purpose |
| ------- | --------- |
| `/spec <name>` | Write or revise a feature spec with acceptance criteria and a clarification pass |
| `/spec-build <name>` | Implement a feature spec test-first, one commit series per AC block, independent verifier loop |
| `/spec-check <name>` | Independent acceptance against a full gate run; only it sets `status: implemented` |
| `/spec-extract` | Spec from existing code, or current-state analysis of an open spec |
| `/qa` | Full gate run (`.claude/gates.json`, mirrors CI) |

Rules under `.claude/rules/` load with this file. Hooks (`.claude/settings.json`): SessionStart lists open feature specs, a guard refuses new suppressions (§9), and the Stop hook runs the fast gates for the areas a turn changed.

Skills still to create as the project matures:

- `aseprite-format` — chunk-by-chunk reference for the `.aseprite` binary format
- `command-pattern` — recipe for adding a new command (data shape, apply/revert, merge logic, undo test)
- `tool-impl` — recipe for adding a new tool (Tool trait impl, command emission, UI binding, tests)
- `wasm-binding` — recipe for exposing a new pincel-core API to JS via wasm-bindgen

These don't need to exist on day one. Add them when you've implemented the second instance of a pattern; that's when the recipe becomes useful.

---

## 14. Quick Reference: Where Things Live

```
docs/specs/pincel.md          The design spec — architecture, data model, Decision Log (§15)
docs/specs/<name>.md          Feature specs with acceptance criteria (/spec), next to the design specs
docs/specs/README.md          Index: spec order, dependencies, backlog, open non-code items
AGENTS.md                     This file — how to build it (CLAUDE.md only imports it)
.gitattributes                Line endings (§6.4) — LF in index and working tree
.claude/skills/               Project-specific skill recipes (spec workflow, qa)
.claude/rules/                Always-loaded rules: spec triage, quality gates
.claude/hooks/                SessionStart / guard / Stop hooks + the gate runner (gates.mjs)
.claude/gates.json            Gate commands: fast (Stop hook, per path) and full (/qa, mirrors CI)

crates/pincel-core/           Pixel-mode core: pure logic, no I/O, no platform
crates/pincel-image-core/     Image-mode core (from amigo-fineliner): layers, brushes, codecs
crates/pincel-effects/        Stateless effects + adjustments, shared by both cores
crates/aseprite-writer/       Standalone, publishable, MIT/Apache
crates/pincel-wasm/           wasm-bindgen layer for pincel-core, cdylib
crates/pincel-image-wasm/     wasm-bindgen layer for pincel-image-core, cdylib

ui/                           Svelte 5 + Vite frontend (PWA + Tauri)
ui/src/App.svelte             Two-mode shell: start screen, New dialog, mode routing
ui/src/lib/shell/             Shell contract (EditorProps), session store, start screen, New dialog
ui/src/modes/pixel/           PixelEditor.svelte (markup, keys, menus) + its modules (render, view-ops, pointer, doc-ops, image-ops, text-tool, file-ops)
ui/src/modes/pixel/stores/    Pixel-mode editor state (one $state object per concern)
ui/src/modes/image/           ImageEditor.svelte + its stores / controller / components
ui/src/lib/core/              WASM adapter (Pixel mode)
ui/src/modes/image/core/      WASM adapter (Image mode) + generated ts-rs types
ui/src/lib/render/            WebGPU + Canvas2D adapters
ui/src/lib/components/        Components both editors / the shell share (effect + recovery dialogs, ColorPair)
ui/src/lib/components/layout/ Shared responsive editor frame: EditorFrame, MenuBar, ToolRail, Panel, ZoomControl
ui/src/lib/components/icons/  Inline-SVG icon set (no icon dependency)
ui/src/lib/components/layers/ The layers panel both modes render (rows mapped by each mode)
ui/src/lib/i18n/              UI strings: en.ts (keys), de.ts (typed against it), t(), locale switch
ui/src/lib/shell/shortcuts.ts The one keyboard map both modes use (spec §9.4)
ui/src/modes/pixel/components/ Pixel-only panels, menus, dialogs (timeline, palette, layers, …)
ui/e2e/                       Playwright specs (§7.4)
ui/src/lib/fs/                File open/save (FSA / download / Tauri)
ui/src/lib/idb/               IndexedDB: prefs, recents, autosave
ui/src/lib/menu/              Native menu bridge (Tauri)
ui/src/lib/platform/          Host detection (isTauri)
ui/src/lib/view/              Viewport helpers (fit/zoom)

src-tauri/                    Native shell
```

---

## Gotchas

Pitfalls and non-obvious knowledge that code and git history don't show. Short entries, no noise; `/spec-build` adds to this list when a loop surprised it.

- **Stale wasm package breaks `pnpm check`.** `ui/` links `crates/*/pkg` (gitignored build output). After Rust changes to a wasm crate — or pulling someone else's — svelte-check reports missing exports (e.g. "has no exported member 'paste_layer'") although the Rust source has them. Fix: `pnpm wasm:build` in `ui/`, not a code change.
- **E2e input waits for `data-ready`, not for a visible canvas.** Both editors mount their canvas, menus, panels and key handlers before the core hands back a document; under CPU load a spec that clicks, types or takes `boundingBox()` then hits a 64×64 placeholder canvas or a document-less editor (passes idle, fails with many workers). The canvas carries `data-ready="true"` once the document is loaded, fitted and drawn — `createDocument` / `waitForEditor` in `ui/e2e/helpers.ts` wait for it. Any other way into an editor (Open… from the start screen, a startup document after reload) calls `waitForEditor` before the first input or status-bar check. It also keeps editor start-up (fetching and compiling the dev wasm takes 10–15 s with 16 workers) off the 30 s test timeout — don't wait for an editor with a bare `expect` instead.
- **E2e Chromium has no WebGPU adapter.** The Playwright browser exposes `navigator.gpu`, but `requestAdapter()` resolves `null` even with `--enable-unsafe-webgpu` / SwiftShader flags, so every e2e run renders through Canvas2D. A spec about the WebGPU path installs a stand-in device with `addInitScript` (see `ui/e2e/renderer.spec.ts`); it cannot check GPU output.

---

*Last updated: 2026-10-29. Amend in place via PR. Significant changes get a Decision Log entry in `docs/specs/pincel.md` §15.*
