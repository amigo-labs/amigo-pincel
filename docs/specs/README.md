# Specs

Two kinds of spec live here (`.claude/rules/specs.md`):

- **Design specs** — architecture, data model and the Decision Log: [`pincel.md`](pincel.md) (§15 Decision Log, §18 competitive landscape), [`canvas.md`](canvas.md), [`cli.md`](cli.md), [`lessons.md`](lessons.md), [`mcp.md`](mcp.md), [`website-spec.md`](website-spec.md).
- **Feature specs** — one feature each, with acceptance criteria and an Evidence table: written with `/spec`, built with `/spec-build`, accepted with `/spec-check` (the only step that sets `status: implemented`).

Feature specs are `status: todo` until `/spec-check` accepts them (see "Done"). A spec with open questions goes back to `/spec <name>` first; `spec-build` refuses it until they are settled. None has had the stranger-reader clarification pass of `/spec` step 4 yet; it runs when the spec is next revised.

## Existing behavior — awaiting acceptance

Reverse-engineered from the code on 2026-10-29 (`/spec-extract`). They describe what is built; Evidence rows marked `built` name an existing test, `open` rows lack one. Next step per spec: `/spec-build <name>` closes the test gaps, then `/spec-check <name>` accepts.

| Area | Specs |
| ---- | ----- |
| Pixel mode | [aseprite-round-trip](aseprite-round-trip.md) |
| Both modes | [i18n](i18n.md), [effects-adjustments](effects-adjustments.md) |
| Shell and platform | [file-access](file-access.md), [desktop-app](desktop-app.md), [app-shell](app-shell.md), [autosave-recovery](autosave-recovery.md) |
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
| 19 | [shortcut-overview](shortcut-overview.md) | both | M | — | No open questions |
| 23 | [symmetry-tiled-mode](symmetry-tiled-mode.md) | Pixel | L | pixel-pencil | Seamless tiles |
| 26 | [psd-import](psd-import.md) | Image | L | image-layer-groups | Needs a dependency decision |
| 29 | [rotsprite](rotsprite.md) | Pixel | L | — | |
| 30 | [website-comparison](website-comparison.md) | website | M | — | `website/` has no test harness yet |
| 31 | [reduced-motion](reduced-motion.md) | both | M | — | Accessibility baseline |
| 32 | [color-picker](color-picker.md) | both | L | — | |
| 34 | [animation-preview](animation-preview.md) | Pixel | M | — | |
| 35 | [tablet-gestures](tablet-gestures.md) | both | M | — | |
| 36 | [more-languages](more-languages.md) | both | L | — | |
| 37 | [first-run-tutorial](first-run-tutorial.md) | both | M | — | Better after more-languages if the tour ships translated |
| 40 | [power-user-shortcuts-keymap](power-user-shortcuts-keymap.md) | both | M | shortcut-overview | |
| 41 | [vectorize-core](vectorize-core.md) | both | L | — | Rust port of the owner's Vectorizer (Potrace → SVG) plus a pixel-exact mode; built, awaits `/spec-check`. The Vectorizer repository is deleted; the golden corpus from it is committed (`crates/pincel-trace/tests/golden/`) |
| 42 | [vectorize-dialog](vectorize-dialog.md) | both | L | vectorize-core | File ▸ Export ▸ SVG… with preview; completes the Vectorizer integration |
| 43 | [unsaved-changes-prompt](unsaved-changes-prompt.md) | shell | M | — | Save / Don't Save / Cancel before work is lost; the desktop app stops closing without asking. Supersedes app-shell AC-14 |
| 44 | [mcp-parity-structure](mcp-parity-structure.md) | both | L | mcp-headless-image | First of five MCP parity specs; runs in parallel with mcp-live-desktop |
| 49 | [history-panel](history-panel.md) | both | L | pixel-undo-history | paint.net parity: History panel with named steps and jumps |
| 50 | [mcp-parity-selection-transform](mcp-parity-selection-transform.md) | both | L | mcp-parity-structure | MCP select.* for both modes, Delete, canvas and layer transforms |
| 51 | [mcp-parity-selection-transform-content](mcp-parity-selection-transform-content.md) | both | M | mcp-parity-selection-transform | MCP move content and headless clipboard (layer.import, export.selection) |
| 52 | [mcp-parity-drawing](mcp-parity-drawing.md) | both | L | mcp-parity-structure | MCP Pixel drawing via draw.stroke / shape / text / gradient |
| 53 | [mcp-parity-drawing-image](mcp-parity-drawing-image.md) | Image | M | mcp-parity-structure | MCP Image recolor / smudge / eraser modes; fixes Image effect defaults; after mcp-parity-drawing |
| 54 | [mcp-parity-pixel-assets](mcp-parity-pixel-assets.md) | Pixel | L | mcp-parity-structure | MCP slices, tilesets / tilemaps, document grid |
| 55 | [mcp-parity-export](mcp-parity-export.md) | both | L | mcp-parity-structure, psd-import, vectorize-dialog | MCP SVG, APNG / GIF, BMP / TIFF, save-back, PSD, PNG as Pixel |
| 56 | [image-live-objects](image-live-objects.md) | Image | L | — | Core live-object record (shapes, line, gradient); text, curves and handles build on it |
| 57 | [image-live-objects-ui](image-live-objects-ui.md) | Image | L | image-live-objects | Handles and commit / cancel for live shapes, line, gradient |
| 58 | [image-live-text](image-live-text.md) | Image | L | image-live-objects | Text stays editable until commit; underline / strikethrough |
| 59 | [image-shapes-curves](image-shapes-curves.md) | Image | L | image-live-objects | Line / Curve with control points, arrow heads, dash-dot |
| 60 | [image-shapes-curves-shapes](image-shapes-curves-shapes.md) | Image | L | image-live-objects-ui, image-shapes-curves | Six new shapes: triangle, diamond, star, block arrow, heart, callout |
| 61 | [image-transform-handles-rotate-zoom](image-transform-handles-rotate-zoom.md) | Image | M | — | Layer ▸ Rotate / Zoom as an Image-only effect; builds the resampler |
| 62 | [image-transform-handles](image-transform-handles.md) | Image | L | image-transform-handles-rotate-zoom, image-live-objects | Scale / rotate selected pixels and the selection with handles |
| 63 | [image-resize-autocrop](image-resize-autocrop.md) | Image | L | — | Resize by percent / print size, Supersampling, Auto Crop |
| 64 | [image-view-aids](image-view-aids.md) | Image | L | — | Rulers, pixel grid, units; view state only |
| 65 | [image-edit-extras](image-edit-extras.md) | Image | L | — | Edit ▸ Fill Selection plus MCP select.fill |
| 66 | [image-edit-extras-tool-options](image-edit-extras-tool-options.md) | Image | L | — | Shared Antialiasing and Normal / Overwrite switches |
| 67 | [clone-stamp](clone-stamp.md) | Image | L | — | Clone Stamp (key C) plus MCP draw.clone |
| 68 | [image-palette-gradient](image-palette-gradient.md) | Image | L | color-picker | Load / save / reset Image palette; new crate pincel-palette |
| 69 | [image-palette-gradient-spiral](image-palette-gradient-spiral.md) | Image | M | — | Spiral gradients, also in MCP draw.gradient |
| 70 | [system-fonts](system-fonts.md) | both | L | — | Installed fonts in both Text tools (fontdb, Local Font Access, font files) |
| 71 | [system-fonts-mcp](system-fonts-mcp.md) | both | M | system-fonts, mcp-parity-drawing | MCP draw.text font argument and font.list |
| 72 | [image-formats-more](image-formats-more.md) | Image | L | — | Open / save TGA, ICO; open DDS (image crate features) |
| 73 | [image-formats-more-modern](image-formats-more-modern.md) | Image | L | image-formats-more | AVIF open (after a wasm spike), JPEG XL open / save |
| 74 | [image-formats-more-print](image-formats-more-print.md) | Image | L | — | File ▸ Print… |

## Done

- [adjustments-paintnet](adjustments-paintnet.md)
- [animated-export](animated-export.md)
- [dithered-gradient](dithered-gradient.md)
- [document-tabs](document-tabs.md)
- [effect-catalog-v2](effect-catalog-v2.md)
- [effect-panel](effect-panel.md)
- [effects-artistic-photo](effects-artistic-photo.md)
- [effects-blurs](effects-blurs.md)
- [effects-distort](effects-distort.md)
- [effects-object-stylize-color-noise](effects-object-stylize-color-noise.md)
- [effects-render](effects-render.md)
- [image-editing](image-editing.md)
- [image-files](image-files.md)
- [image-layer-groups-ora](image-layer-groups-ora.md)
- [image-layer-groups](image-layer-groups.md)
- [indexed-grayscale-compose](indexed-grayscale-compose.md)
- [linked-cels-group-compose](linked-cels-group-compose.md)
- [mcp-headless-image](mcp-headless-image.md)
- [mcp-headless-pixel](mcp-headless-pixel.md)
- [outline-effect](outline-effect.md)
- [palette-formats](palette-formats.md)
- [pen-input](pen-input.md)
- [pixel-compose](pixel-compose.md)
- [pixel-drawing-tools](pixel-drawing-tools.md)
- [pixel-export](pixel-export.md)
- [pixel-layers-frames](pixel-layers-frames.md)
- [pixel-palette](pixel-palette.md)
- [pixel-pencil](pixel-pencil.md)
- [pixel-pressure](pixel-pressure.md)
- [pixel-selection-move](pixel-selection-move.md)
- [pixel-undo-history](pixel-undo-history.md)
- [power-user-shortcuts-coverage](power-user-shortcuts-coverage.md)
- [power-user-shortcuts-palette](power-user-shortcuts-palette.md)
- [save-original-format](save-original-format.md)
- [scaled-export](scaled-export.md)
- [shading-ink](shading-ink.md)
- [slices](slices.md)
- [tilemaps](tilemaps.md)
- [tool-variant-flyout](tool-variant-flyout.md)
- [wasm-npm-api](wasm-npm-api.md)

## Open items outside specs

Not code; they need a person, hardware or an account.

- **Release prerequisites** (`docs/RELEASING.md`): updater signing key as organization secrets, access to the `MACOS_SIGNING_CERT` organization secrets (`create-signing-cert.sh --repos amigo-blinzel,amigo-pincel`, optional), `RELEASE_PLEASE_TOKEN`, npm trusted publisher for `@amigo-labs/pincel-wasm` then `PUBLISH_NPM=true`, `pincel.amigo-labs.dev` as the Worker's custom domain.
- **Native-speaker review** of the German copy: `website/src/lib/i18n/de.ts` and `ui/src/lib/i18n/de.ts` are machine-written.
- **Checks on real hardware** (spec §16 exit criteria): 60 fps at 256×256 / zoom 32 on M1 and mid-tier Windows (F2 probe); Lighthouse PWA ≥ 90; a Pincel-saved file opened in upstream Aseprite and a Pincel tilemap loaded by `amigo_assets` (former M6.7); real Tauri installs and the self-update on Windows, macOS and Linux.

## Backlog: gaps not yet written as specs

Not commitments. When one becomes next, write it with `/spec` and move it into the order table.

| Gap | Note |
| --- | ---- |
| Layer masks and clipping masks | Probably Image mode first |
| Non-destructive layer effects and adjustment layers | Large; touches compose in both cores |
| Reference layers | Small: a non-exported, non-editable layer kind |
| Custom brushes | Spec §13.3 |
| CLI batch export (`pincel-cli`) | Design in `cli.md`; PNG / atlas export already exists in `pincel-core` |
| Isometric and hexagonal tilemaps; Godot / Tiled export | Extends tilemaps |
| Indexed-mode painting, auto-tilemap, tile-editor routing for Line / Rect / Bucket, scripting | Phase 2 (§13); indexed painting needs a SetPixel payload decision |
| PSD export | After psd-import |
| Native iPad app | PWA on iPad today; Tauri mobile has no iOS path (§14 question 5) |
| Pixel tools clip to the selection | Bucket, strokes and shapes ignore the selection in the Pixel editor; MCP `draw.fill` already clips (mcp-parity-drawing) |
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
