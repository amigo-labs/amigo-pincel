# CLAUDE.md — Pincel Implementation Guide

This file is the operational contract between Claude Code and the Pincel codebase. It exists to keep work efficient, predictable, and **scoped small enough that no single session runs into context limits or timeouts**.

Read this entire file before starting any task. Re-read Section 3 ("Session Management") at the start of every session.

---

## 1. Project Overview

Pincel is a pixel-art editor at the feature level of Aseprite, focused on game asset creation. It ships as a PWA and a native Tauri app sharing one Svelte UI and one Rust core.

The full design specification lives in **`docs/specs/pincel.md`**. That document is the source of truth for *what* to build. This file is the source of truth for *how* to build it.

**Always read the relevant section of `docs/specs/pincel.md` before starting an implementation task.** The spec is sectioned and indexed; load only what's needed.

---

## 2. Workflow Philosophy

### 2.1 Spec-Driven

Every implementation task traces back to a specific section of `docs/specs/pincel.md`. If a task implies a design decision not in the spec, **stop and update the spec first** with a Decision Log entry. Do not improvise architecture.

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

One concept per commit. Commit messages start with the crate name in brackets:

```
[pincel-core] add Sprite struct with Layers and Frames
[aseprite-writer] implement Layer Chunk (0x2004) write
[ui] wire Pencil tool onPointerDown handler
```

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
|------|------------|--------|
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

When stopping, write a short status note in the commit message or in a `STATUS.md` file at repo root with:

- What was completed
- What's the next concrete task
- Any open questions

### 3.4 Resume Protocol

When starting a session that continues prior work:

1. `git log --oneline -10` to see recent commits
2. Read `STATUS.md` if present
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

Implement in this order. Each step is a milestone gate; do not start step N+1 until step N is complete and committed.

```
M1  pincel-core skeleton
    └─ Cargo.toml workspace, basic types (Sprite, Layer, Frame, Cel, Palette)
    └─ No I/O, no rendering. Pure data model + builder API.
    └─ Unit tests for type construction.

M2  pincel-core commands + undo
    └─ Command trait, command bus, undo stack
    └─ Implement 3 commands: SetPixel, AddLayer, AddFrame
    └─ Tests: apply / revert round-trip preserves state

M3  pincel-core compose() — image layers only
    └─ RGBA-only path first, indexed deferred
    └─ No tilemaps, no slices, no overlays
    └─ Snapshot test: known sprite produces expected RGBA bytes

M4  aseprite-loader integration (read)
    └─ Adapter from loader output to pincel-core::Sprite
    └─ Fixture: open a hand-crafted .aseprite, assert structure
    └─ Skip tilemap/slice chunks at first; preserve as opaque

M5  aseprite-writer crate — image layers only
    └─ Chunks: Header, Layer (0x2004), Cel (0x2005, image), Palette (0x2019), Tags (0x2018)
    └─ Round-trip test: read fixture → write → read → assert equal

M6  pincel-wasm + minimal UI
    └─ wasm-bindgen exports for Document, applyTool, compose
    └─ Svelte 5 + Vite scaffold under ui/
    └─ Single tool: Pencil
    └─ Demo: open .aseprite, paint, save (download), reopen — file valid in Aseprite

M7  Tools expansion
    └─ Eraser, Bucket, Line, Rectangle, Ellipse, Eyedropper, Move, Selection
    └─ Each tool: command type + input handler + UI button

M8  Tilemap support
    └─ Document model: Tilemap layer, Tileset, TileRef
    └─ aseprite-loader: hydrate tilemap chunks
    └─ aseprite-writer: write Tileset (0x2023) and Tilemap Cel (Cel Type 3)
    └─ UI: Tileset Panel, Tilemap Stamp tool, Tileset Editor sub-mode

M9  Slice support
    └─ Document model: Slice, SliceKey
    └─ aseprite read/write: Slice Chunk (0x2022)
    └─ UI: Slice tool, Slices Panel, 9-patch + pivot editing

M10 PWA polish
    └─ Service worker, IndexedDB autosave, recent files registry
    └─ File System Access API + download fallback

M11 Tauri build
    └─ src-tauri/, native commands, OS dialogs, file association

M12 Performance pass
    └─ Dirty-rect compose, profiling
    └─ Target: 256×256 sprite at zoom 32 maintains 60 fps
```

Each milestone is a sequence of S/M tasks. Plan the tasks before starting the milestone; write them as a checklist in `STATUS.md`.

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

### 5.3 `pincel-wasm`

- **Crate type:** `cdylib`. Built via `wasm-pack build --target web --release`.
- **Public API:** matches the npm-package surface in spec §17.5. Keep it small and stable.
- **Memory:** state owned in Rust. JS receives `Uint8ClampedArray` views, never owned copies, when possible.
- **Build output:** `pkg/` — gitignored, generated.

### 5.4 `ui/`

- **Framework:** Svelte 5 with runes (`$state`, `$derived`, `$effect`).
- **Styling:** Tailwind 4 utility classes; complex components via shadcn-svelte (copied into `src/lib/components/ui/`, not a runtime dep).
- **TypeScript strict mode.** No implicit `any`. No `// @ts-ignore` without an issue link.
- **State stores:** Svelte 5 runes in `src/lib/stores/`. One store per concern (document, tools, panels, prefs).
- **WASM bridge:** all calls go through `src/lib/core/` adapter; UI never imports `pincel-wasm` directly.

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

Phase 2 concern. Use Playwright when added. Each tool has a happy-path interaction test minimum.

### 7.5 Test Performance

Tests run in CI on every push. Keep the full test suite under 60 seconds. If a test is slow, mark it `#[ignore]` and add it to a separate `cargo test --ignored` job.

---

## 8. Branch & PR Conventions

- `main` is protected; all changes via PR
- Branch names: `feat/<short-desc>`, `fix/<short-desc>`, `chore/<short-desc>`, `spec/<short-desc>`
- PR title matches commit message convention: `[crate] action`
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
cargo check                                  # all crates, fast
cargo check -p pincel-core                   # one crate
cargo test -p pincel-core
cargo clippy --workspace -- -D warnings
cargo fmt
cargo doc --workspace --no-deps --open
```

### WASM

```bash
cd crates/pincel-wasm
wasm-pack build --target web --release       # produces pkg/
```

### UI

```bash
cd ui
pnpm install
pnpm dev                                     # Vite dev server
pnpm build                                   # production bundle
pnpm test                                    # unit tests (Vitest)
pnpm test:e2e                                # Playwright (later phases)
pnpm lint
```

### Tauri

```bash
cd src-tauri
pnpm tauri dev                               # native dev
pnpm tauri build                             # release binary
```

### Full pre-commit gate

```bash
cargo fmt && cargo clippy --workspace -- -D warnings && cargo test --workspace && cd ui && pnpm lint && pnpm test
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

For milestone-level work add:

- [ ] Exit criterion in spec §16 is met
- [ ] `STATUS.md` updated
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

Initial skills to create as the project matures:

- `aseprite-format` — chunk-by-chunk reference for the `.aseprite` binary format
- `command-pattern` — recipe for adding a new command (data shape, apply/revert, merge logic, undo test)
- `tool-impl` — recipe for adding a new tool (Tool trait impl, command emission, UI binding, tests)
- `wasm-binding` — recipe for exposing a new pincel-core API to JS via wasm-bindgen

These don't need to exist on day one. Add them when you've implemented the second instance of a pattern; that's when the recipe becomes useful.

---

## 14. Quick Reference: Where Things Live

```
docs/specs/pincel.md          The spec — what to build
CLAUDE.md                     This file — how to build it
STATUS.md                     Current session state, next task
.claude/skills/               Project-specific skill recipes

crates/pincel-core/           Pure logic, no I/O, no platform
crates/aseprite-writer/       Standalone, publishable, MIT/Apache
crates/pincel-wasm/           wasm-bindgen layer, cdylib

ui/                           Svelte 5 + Vite frontend (PWA + Tauri)
ui/src/lib/core/              WASM adapter
ui/src/lib/render/            WebGPU + Canvas2D adapters
ui/src/lib/tools/             Tool input handlers
ui/src/lib/components/        Svelte components

src-tauri/                    Native shell
```

---

*Last updated: 2026-05-07. Amend in place via PR. Significant changes get a Decision Log entry in `docs/specs/pincel.md` §15.*
