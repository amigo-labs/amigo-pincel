# Specs

Two kinds of spec live here (`.claude/rules/specs.md`):

- **Design specs** — architecture, data model and the Decision Log: [`pincel.md`](pincel.md) (§15 Decision Log, §18 competitive landscape), [`canvas.md`](canvas.md), [`cli.md`](cli.md), [`lessons.md`](lessons.md), [`mcp.md`](mcp.md), [`website-spec.md`](website-spec.md).
- **Feature specs** — one feature each, with acceptance criteria and an Evidence table: written with `/spec`, built with `/spec-build`, accepted with `/spec-check` (the only step that sets `status: implemented`).

Feature specs are `status: todo` until `/spec-check` accepts them (see "Done"). A spec with open questions goes back to `/spec <name>` first; `spec-build` refuses it until they are settled. None has had the stranger-reader clarification pass of `/spec` step 4 yet; it runs when the spec is next revised.

## Existing behavior — awaiting acceptance

Reverse-engineered from the code on 2026-10-29 (`/spec-extract`). They describe what is built; Evidence rows marked `built` name an existing test, `open` rows lack one. Next step per spec: `/spec-build <name>` closes the test gaps, then `/spec-check <name>` accepts.

| Area | Specs |
| ---- | ----- |
| Pixel mode | [aseprite-round-trip](aseprite-round-trip.md), [pixel-selection-move](pixel-selection-move.md), [pixel-layers-frames](pixel-layers-frames.md), [pixel-palette](pixel-palette.md), [pixel-export](pixel-export.md), [tilemaps](tilemaps.md), [slices](slices.md) |
| Image mode | [image-files](image-files.md) |
| Both modes | [effects-adjustments](effects-adjustments.md), [pen-input](pen-input.md), [i18n](i18n.md) |
| Shell and platform | [app-shell](app-shell.md), [file-access](file-access.md), [autosave-recovery](autosave-recovery.md), [desktop-app](desktop-app.md) |
| Website | [website](website.md) |

## Planned features — order

Owner priorities lead: performance and MCP. Both chains are independent of each other and of the UI specs, so they can run in parallel on separate branches.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| 1 | [performance-benchmarks](performance-benchmarks.md) | both | L | — | Measure first; budgets every later step proves against |
| 2 | [performance-image-kernel](performance-image-kernel.md) | Image | M | performance-benchmarks | Biggest single win (LUT ≈ 7×); no open questions |
| 3 | [performance-image-cache](performance-image-cache.md) | Image | L | performance-image-kernel | Dab and pan budgets |
| 4 | [performance-input-path](performance-input-path.md) | both | M | performance-benchmarks | Pixel part waits on pixel-pencil |
| 5 | [performance-background-jobs](performance-background-jobs.md) | both | L | performance-benchmarks | Autosave / effect jank off the main thread |
| 6 | [performance-build-simd](performance-build-simd.md) | both | M | performance-image-kernel | Recompose and cold-start budgets |
| 7 | [performance-image-undo-memory](performance-image-undo-memory.md) | Image | L | performance-benchmarks | Can move earlier if memory hurts |
| 8 | [performance-desktop-jobs](performance-desktop-jobs.md) | both | M | performance-background-jobs | Desktop only |
| 11 | [mcp-live-desktop](mcp-live-desktop.md) | both | L | mcp-headless-image | Only MCP part touching UI and Tauri |
| 13 | [copy-cut-paste](copy-cut-paste.md) | both | L | — | Basic editor equipment |
| 16 | [pixel-grid](pixel-grid.md) | Pixel | M | — | Grid stored in the Aseprite header |
| 17 | [scaled-export](scaled-export.md) | Pixel | M | — | Shares the upscale with animated-export |
| 18 | [outline-effect](outline-effect.md) | both | M | — | Adds a color param kind to effects |
| 19 | [shortcut-overview](shortcut-overview.md) | both | M | — | No open questions |
| 20 | [indexed-grayscale-compose](indexed-grayscale-compose.md) | Pixel | M | — | Renderer only; the codec rejects indexed files too |
| 21 | [linked-cels-group-compose](linked-cels-group-compose.md) | Pixel | L | — | Linked-cel ACs have no open questions; group folding needs a Decision Log entry |
| 22 | [pixel-pressure](pixel-pressure.md) | Pixel | M | pixel-pencil | |
| 23 | [symmetry-tiled-mode](symmetry-tiled-mode.md) | Pixel | L | pixel-pencil | Seamless tiles |
| 24 | [image-layer-groups](image-layer-groups.md) | Image | L | — | |
| 25 | [image-layer-groups-ora](image-layer-groups-ora.md) | Image | M | image-layer-groups | Fixes ORA group opacity / visibility lost on open |
| 26 | [psd-import](psd-import.md) | Image | L | image-layer-groups | Needs a dependency decision |
| 27 | [dithered-gradient](dithered-gradient.md) | Pixel | M | — | |
| 28 | [shading-ink](shading-ink.md) | Pixel | M | pixel-pencil | |
| 29 | [rotsprite](rotsprite.md) | Pixel | L | — | |
| 30 | [website-comparison](website-comparison.md) | website | M | — | `website/` has no test harness yet |
| 31 | [reduced-motion](reduced-motion.md) | both | M | — | Accessibility baseline |
| 32 | [color-picker](color-picker.md) | both | L | — | |
| 33 | [palette-formats](palette-formats.md) | Pixel | M | — | Lospec palettes |
| 34 | [animation-preview](animation-preview.md) | Pixel | M | — | |
| 35 | [tablet-gestures](tablet-gestures.md) | both | M | — | |
| 36 | [more-languages](more-languages.md) | both | L | — | |
| 37 | [first-run-tutorial](first-run-tutorial.md) | both | M | — | Better after more-languages if the tour ships translated |
| 38 | [power-user-shortcuts-coverage](power-user-shortcuts-coverage.md) | both | M | — | |
| 39 | [power-user-shortcuts-palette](power-user-shortcuts-palette.md) | both | M | — | Command palette; no open questions |
| 40 | [power-user-shortcuts-keymap](power-user-shortcuts-keymap.md) | both | M | shortcut-overview | |

## Done

- [animated-export](animated-export.md)
- [image-editing](image-editing.md)
- [mcp-headless-image](mcp-headless-image.md)
- [mcp-headless-pixel](mcp-headless-pixel.md)
- [pixel-compose](pixel-compose.md)
- [pixel-drawing-tools](pixel-drawing-tools.md)
- [pixel-pencil](pixel-pencil.md)
- [pixel-undo-history](pixel-undo-history.md)
- [save-original-format](save-original-format.md)
- [wasm-npm-api](wasm-npm-api.md)

## Open items outside specs

Not code; they need a person, hardware or an account.

- **Release prerequisites** (`docs/RELEASING.md`): updater signing key as organization secrets, `RELEASE_PLEASE_TOKEN`, npm trusted publisher for `@amigo-labs/pincel-wasm` then `PUBLISH_NPM=true`, `pincel.amigo-labs.dev` as the Worker's custom domain.
- **Native-speaker review** of the German copy: `website/src/lib/i18n/de.ts` and `ui/src/lib/i18n/de.ts` are machine-written.
- **Checks on real hardware** (spec §16 exit criteria): 60 fps at 256×256 / zoom 32 on M1 and mid-tier Windows (F2 probe); Lighthouse PWA ≥ 90; a Pincel-saved file opened in upstream Aseprite and a Pincel tilemap loaded by `amigo_assets` (former M6.7); real Tauri installs and the self-update on Windows, macOS and Linux.

## Backlog: gaps not yet written as specs

Not commitments. When one becomes next, write it with `/spec` and move it into the order table.

| Gap | Note |
| --- | ---- |
| Layer masks and clipping masks | Probably Image mode first |
| Non-destructive layer effects and adjustment layers | Large; touches compose in both cores |
| Reference layers | Small: a non-exported, non-editable layer kind |
| Clone stamp, Bézier curves, custom brushes | Spec §5.4 / §13.3 |
| History panel (list of undo steps) | Undo exists; only the panel is missing |
| CLI batch export (`pincel-cli`) | Design in `cli.md`; PNG / atlas export already exists in `pincel-core` |
| Isometric and hexagonal tilemaps; Godot / Tiled export | Extends tilemaps |
| Indexed-mode painting, auto-tilemap, tile-editor routing for Line / Rect / Bucket, scripting | Phase 2 (§13); indexed painting needs a SetPixel payload decision |
| PSD export | After psd-import |
| Native iPad app | PWA on iPad today; Tauri mobile has no iOS path (§14 question 5) |
| Brush stabilizer / stroke smoothing | Image brush first |
| New-dialog size presets | 16/32/64 px, 1080p, screen size, last used |
| Color mode conversion (RGB ↔ Indexed ↔ Grayscale) | Tied to indexed painting |
| CJK font for the Text tool | Needed for Japanese text on canvas (more-languages) |
| ICO export, rulers / guides (Image), settings dialog, light theme | Small, as needed |
| Adobe .ase / .aco swatches | After palette-formats |
| WebMCP (agents in the PWA) | Watch browser proposals; see mcp-live-desktop |
| Shared shell package (`@amigo-labs/canvas-shell`) | Extract `fs` / `idb` / PWA / Tauri wrapper for other amigo-labs tools; cut: what knows the document model stays in the app |
| Aseprite round-trip gaps | Layer flags Background / Reference / Collapsed / LockMovement, cel `z_index`, user data on layers / cels / sprite / tags, slice note text and properties |
| Mid-list AddFrame / duplicate frame | Append-only today; needs a FrameIndex remap of cels, tags, slices |
| Selection in the undo stack | Selection lives on `Sprite` outside commands; Aseprite tracks it |
| Multi-frame slice editing | The panel edits frame-0 keys only |
| Per-command dirty regions, per-tile dirty events | Undo / redo and `setTilePixel` emit full-canvas dirty |
| `pincel-wasm` errors as `JsError` | Returns `Result<_, String>` today |
| Scope Tauri file commands | `read_file_bytes` / `write_file_bytes` accept any path from the webview |
| Error surfacing | Failures only reach the transient status bar; a toast area would be harder to miss |
