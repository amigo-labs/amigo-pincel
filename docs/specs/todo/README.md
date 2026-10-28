# Todo specs: gaps from the competitive landscape

These specs come from the feature comparison in `docs/specs/pincel.md` §18 (2026-10-28). Each file is written so that **a fresh session with no prior context** can pick it up.

Every spec gives:
- the goal and the reason for it;
- the current code, with paths and approximate line numbers. These drift, so grep for the names;
- the design decisions, with a recommendation, including which ones need a Decision Log entry or the user's approval;
- a commit and session plan sized per CLAUDE.md §3.2;
- the tests that prove the feature, and what is out of scope.

## How to start a session on one of these

1. Read `CLAUDE.md` in full, then this README, then the chosen spec.
2. Run the pre-flight checklist (CLAUDE.md §3.1). `cargo check` must be green before you start.
3. Do **one** task from the spec's task list per session (one M task).
4. The first commit of every spec is a `docs(spec)` commit: the Decision Log entry (§15) and the spec sections it touches. Decisions marked "stop and ask" (new dependencies, public API changes) go to the user **before** any code.
5. When the spec's last task is done, set `status: done` in its front matter and move its row below to "Done".

## Order and sizes

| # | Spec | Mode | Size | Depends on | Why this position |
|---|------|------|------|------------|-------------------|
| 23 | [Performance: make it fly](23-performance.md) | both (Image first) | XL (7 × M) | — | Owner priority. Measured: Image compose 101 ms at 1K², 1.26 s at 4K² (9 `powf` per pixel and layer; a LUT alone is ≈ 7×). Step 0 (benchmarks + budgets) first; independent of the UI specs |
| 18 | [MCP server: LLMs work with Pincel](18-mcp.md) | both | XL (6 × M) | — | The owner's top priority. Headless `pincel-mcp` first, then live control of the desktop app; phase 0 is a refactor that also tidies the wasm crates |
| 01 | [Pixel pencil: sizes 1–8, Shift constrain, pixel-perfect](01-pixel-pencil.md) | Pixel | L (3 × M) | — | Fixes §5.2 promises that were never built. 02, 07 and 08-B build on its `PencilStroke` command |
| 05 | [Copy, Cut, Paste in both modes](05-copy-cut-paste.md) | both | L (3 × M) | — | Basic editor equipment; users expect it first |
| 06 | [Animated export (APNG, GIF)](06-animated-export.md) | Pixel | L (3 × M) | — | How pixel art gets shared; APNG needs no new dependency |
| 11 | [Image mode: save back to the original format](11-save-original-format.md) | Image | M | — | Screenshot workflow: Ctrl+S on an opened PNG asks for an `.ora` today |
| 09 | [Pixel grid and custom grid](09-pixel-grid.md) | Pixel | M | — | Basic pixel-art equipment; grid stored in the sprite via the Aseprite header |
| 10 | [Scaled export (×1–×10)](10-scaled-export.md) | Pixel | S–M | — | Shareable exports; shares the upscale with 06 |
| 12 | [Outline effect](12-outline-effect.md) | Pixel first, both | S–M | — | Everyday game-asset need; adds a color param kind to effects |
| 13 | [Keyboard shortcuts overview](13-shortcut-overview.md) | both | S | — | Discoverability; 21 builds on it |
| 02 | [Pen pressure in Pixel mode](02-pixel-pressure.md) | Pixel | M | 01 | Small once 01 exists |
| 07 | [Symmetry and tiled mode](07-symmetry-tiled-mode.md) | Pixel | L (2 × M) | 01 | Game-asset focus (seamless tiles) |
| 03 | [Layer groups in Image mode](03-image-layer-groups.md) | Image | XL (4 × M) | — | Also fixes ORA group attributes being dropped on open (silent data loss) |
| 04 | [PSD import](04-psd-import.md) | Image | L (2 × M) | 03 for groups (flat import works without it) | Needs a dependency decision |
| 08 | [Dithered gradient, shading ink, RotSprite](08-pixel-tools.md) | Pixel | XL (3 independent parts) | 01 (shading ink only) | Polish for pixel artists |
| 24 | [Website comparison in line with §18](24-website-comparison.md) | website | S–M | — | Adds Pixelorama and an Image-mode table, fixes the Aseprite license cell, shows honest gaps |
| 22 | [Respect "reduce motion"](22-reduced-motion.md) | both | S | — | Accessibility baseline; small, can go any time |
| 14 | [Own color picker (HSV, alpha, recent)](14-color-picker.md) | both | L (2 × M) | — | Native picker has no alpha and differs per OS |
| 15 | [Palette formats (.hex, .pal, PNG)](15-palette-formats.md) | Pixel | M | — | Lospec palettes |
| 16 | [Animation preview panel](16-animation-preview.md) | Pixel | M | — | Watch and draw at once |
| 17 | [Tablet gestures](17-tablet-gestures.md) | both | M | — | iPad: two/three-finger tap, pen eraser end |
| 19 | [More languages: es, ja, fr, pt-BR](19-more-languages.md) | both | L (M + 4 × S) | — | Infrastructure (plurals, detection) first |
| 20 | [First-run tutorial and samples](20-first-run-tutorial.md) | shell + both | M | — | Better after 19 if the tour should ship translated |
| 21 | [Power-user keyboard](21-power-user-shortcuts.md) | both | L (3 × M) | 13 | Coverage, custom keymap, command palette |

Notes on the order:
- 23 (performance) and 18 (MCP) lead: the owner named both as top priorities, and neither blocks the UI specs. Within 23, step 1 (compositing kernel) is the biggest single win and can start right after step 0.
- 18 (MCP) is the owner's most important feature. Its phase 0 and phases 1–4 (headless) don't touch the UI, so they can run in parallel with UI specs.
- 03 can move ahead of 05 or 06 if Image-mode users report the ORA group problem. Opening a layered `.ora` from Krita or MyPaint already loses group opacity and visibility.
- 01, 05 and 06 don't depend on each other and can run in parallel sessions on separate branches.

## Done

_None yet._

## Backlog: gaps found but not yet written as specs

These come from the same comparison. They aren't commitments; write a spec here when one becomes next.

| Gap | Who has it | Note |
|-----|------------|------|
| Layer masks and clipping masks | Resprite, Photopea, Krita, Affinity, Pixlr | Probably Image mode first |
| Non-destructive layer effects and adjustment layers | Pixelorama (layer effects), Resprite (layer styles), Affinity, Photopea | Large; touches compose in both cores |
| Reference layers | Aseprite | Small: a non-exported, non-editable layer kind |
| Clone stamp, Bézier curves, custom brushes | Paint.NET, Krita, Photopea; Aseprite (custom brushes) | Spec §5.4 lists them as not implemented; custom brushes are §13.3 |
| History panel (list of undo steps) | Photoshop-style editors, Krita, Photopea | Undo exists; only the panel is missing |
| Command palette | Aseprite 1.3.16 (Run Command) | Could reuse the menu data from `EditorFrame` |
| CLI batch export | Aseprite (`--batch`) | `docs/specs/cli.md` is a draft; no binary exists |
| Isometric and hexagonal tilemaps; Godot / Tiled export | Pixelorama, Resprite | Extends M8 tilemaps |
| Indexed-mode painting, auto-tilemap, scripting | Aseprite, Pixelorama | Already in §13 (Phase 2) |
| PSD export | Resprite, Pixaki, Photopea | After 04 |
| Native iPad app | Pixaki, Resprite | Pincel is a PWA on iPad; Tauri mobile has no iOS path today (§14 question 5) |
| Paste into New Image (Ctrl+Alt+V) | Paint.NET | Cheap follow-up to 05 |
| Brush stabilizer / stroke smoothing | Krita, Resprite | Image brush first |
| New-dialog size presets | Aseprite, paint.net | 16/32/64 px, 1080p, screen size, last used |
| Color mode conversion (RGB ↔ Indexed ↔ Grayscale) | Aseprite | Tied to indexed painting (§13) |
| CJK font for the Text tool | — | Needed for Japanese text on canvas (see 19) |
| ICO export, rulers / guides (Image), settings dialog, light theme | paint.net, Photopea | Small, as needed |
| Adobe .ase / .aco swatches | Photoshop ecosystem | After 15 |
| WebMCP (agents in the PWA) | — | Watch browser proposals; see 18 |
