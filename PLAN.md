# Deep Fixup plan — 2026-06-10

Baseline at session start: **fully green**.

```bash
cargo fmt --all --check
cargo clippy -p pincel-core -p aseprite-writer -p pincel-wasm --all-targets -- -D warnings
cargo test -p pincel-core -p aseprite-writer -p pincel-wasm   # 454 tests
cd ui && pnpm lint && pnpm check && pnpm build
```

(`cargo check -p pincel-tauri` needs GTK system libraries absent in this
container — CI's `tauri` job covers it; `src-tauri` is untouched here.)

Analysis notes: several candidate findings were investigated and rejected as
false positives — the `undo()`/`redo()` try-catch asymmetry in `App.svelte`
matches the wasm API (undo is infallible by design, redo returns `Result`);
the command bus is covered by `crates/pincel-core/tests/command_bus.rs`; the
`syncRecentMenu` fire-and-forget `$effect` is documented as deliberate.

## Tasks

- [x] T1: [pincel-core] read tag color in aseprite_read (round-trip fidelity bug)
      Files: crates/pincel-core/src/codec/aseprite_read.rs:218 (map_tag),
             crates/pincel-core/tests/aseprite_codec.rs:256
      Change: the write path emits `tag.color` RGB but the read path
              hard-codes `Rgba::WHITE`, bleaching tag colors on every open.
              Read the loader's deprecated-but-still-written `color` field
              into `Rgba { r, g, b, a: 255 }`; keep `#[allow(deprecated)]`
              with a justifying comment. Extend
              `tags_round_trip_with_directions` to assert two distinct
              non-white colors survive.
      Verify: cargo test -p pincel-core

- [ ] T2: [ui] re-entrancy guard on async file operations
      Files: ui/src/App.svelte — newDoc (1016), openDoc (1035), save (1075),
             applyRecovery (~1150), openRecent (~1250), openByPath (~1710),
             toolbar buttons (~1808–1834)
      Change: add `let fileOpBusy = $state(false)`; each async file fn
              early-returns when busy, sets it before the first await and
              clears it in `finally`. Guard `newDoc()` entry too (it frees
              the doc synchronously under an in-flight save). Disable
              New/Open/Save/Save As/Recent buttons while busy.
      Verify: cd ui && pnpm check && pnpm lint && pnpm build

- [ ] T3: [ui] extract duplicated document-reset block (×5)
      Files: ui/src/App.svelte:1019–1028, 1049–1058, 1154–1163, 1257–1266,
             1714–1723
      Change: identical 10-line reset sequence in newDoc/openDoc/
              applyRecovery/openRecent/openByPath → extract
              `resetDocViewState()` next to `disposeDoc()`; call it from all
              five sites (per-site `saveTarget`/`docId`/status lines stay).
      Verify: cd ui && pnpm check && pnpm lint && pnpm build

- [ ] T4: [pincel-core] document the two bare clippy suppressions
      Files: crates/pincel-core/src/render/compose.rs:431,
             crates/pincel-wasm/src/lib.rs:1395
      Change: CLAUDE.md §9 forbids undocumented suppressions; add a one-line
              comment above each `#[allow(clippy::too_many_arguments)]`
              stating why the suppression is correct (private single-caller
              compose helper; wasm-bindgen method mirroring the JS API shape).
      Verify: cargo clippy -p pincel-core -p pincel-wasm --all-targets -- -D warnings

- [ ] T5: [chore] add missing LICENSE texts
      Files: new LICENSE-MIT + LICENSE-APACHE at repo root, copies in
             crates/aseprite-writer/
      Change: workspace Cargo.toml declares `MIT OR Apache-2.0` but no
              LICENSE file exists anywhere (wasm-pack warns on every build,
              and aseprite-writer is meant to be publishable). Add the
              standard texts (MIT copyright line: 2026 amigo-labs).
      Verify: files present at both locations

- [ ] T6: [docs] root README.md (clone → running)
      Files: new README.md at repo root
      Change: no root README; encode the non-obvious build order
              (`pnpm wasm:build` before `pnpm install` due to the
              `link:../crates/pincel-wasm/pkg` dep — an open question in
              STATUS.md). What Pincel is, repo layout, prerequisites, exact
              clone→run steps, command table, links to spec/CLAUDE.md,
              license + Aseprite trademark note.
      Verify: steps match the commands exercised this session

- [ ] T7: [ui] shape-tool tooltips + group key cycling
      Files: ui/src/App.svelte — TOOL_KEYS (1455), onKeyDown (1516–1520),
             tool buttons (1914–1937)
      Change: Rect Fill / Ellipse / Ellipse Fill have no `title` and no
              keyboard access. Make TOOL_KEYS values `Tool[]` groups
              (`u: [rectangle, rectangle-fill, ellipse, ellipse-fill]`);
              repeated presses cycle within the group (Aseprite pattern),
              first press selects the first entry. Add the three missing
              `title` attributes.
      Verify: cd ui && pnpm check && pnpm lint && pnpm build

- [ ] T8: [docs] sync CLAUDE.md / spec / STATUS.md to reality
      Files: CLAUDE.md §10 + §14, docs/specs/pincel.md:634, STATUS.md
      Change: §10 lists nonexistent `pnpm test` / `pnpm test:e2e` (incl. the
              pre-commit gate); §14 maps nonexistent `ui/src/lib/stores/` and
              `ui/src/lib/tools/`; spec §11.4 `isTauri` snippet predates
              Tauri 2's `__TAURI_INTERNALS__` (open question asks for this
              sweep); STATUS.md claims fmt drift in pincel-wasm that is no
              longer present. Fix all four; add a session entry to STATUS.md
              and mark the resolved open questions.
      Verify: re-read touched sections

## Not this session

- Pixel-offset formula duplication (~14 production sites, 8 command files):
  locally idiomatic, bespoke bounds/error handling per site; low payoff.
- Tag color via User Data chunk (modern Aseprite's canonical location).
- Vitest/Playwright UI test infra — Phase 2 per CLAUDE.md; new devDeps need approval.
- `App.svelte` (2,202 lines) decomposition — multi-session refactor.
- Touch pinch-zoom.
- `src-tauri` path-scoping of `read_file_bytes`/`write_file_bytes`
  (renderer may pass arbitrary paths — typical for a local editor, but
  worth a maintainer decision).
