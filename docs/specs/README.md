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
| Both modes | [i18n](i18n.md) |
| Shell and platform | [file-access](file-access.md), [desktop-app](desktop-app.md) |
| Website | [website](website.md) |

## Planned features — order

Owner priorities lead: performance and MCP. The specs are grouped by area; each group is built top to bottom, and groups that share no files run in parallel on separate branches or worktrees (`D:/worktrees/<repo>.<name>`, AGENTS.md). A spec number is its group letter plus its place in the group.

### A — MCP

Agents reach every document-changing capability; live mode last. Runs in parallel with every other group.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| A8 | [mcp-live-desktop](mcp-live-desktop.md) | both | L | mcp-headless-image | Only MCP part touching UI and Tauri; revised 2026-11-06 for the in-app chat (I1) |

### B — Image live objects

Shapes, line, gradient, text and selection transforms stay editable until committed (paint.net parity). `image-live-objects` first; parallel with A, D, E, F.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| B1 | [image-live-objects](image-live-objects.md) | Image | L | — | Core live-object record (shapes, line, gradient); text, curves and handles build on it |
| B2 | [image-live-objects-ui](image-live-objects-ui.md) | Image | L | image-live-objects | Handles and commit / cancel for live shapes, line, gradient |
| B7 | [image-transform-handles](image-transform-handles.md) | Image | L | image-transform-handles-rotate-zoom, image-live-objects | Scale / rotate selected pixels and the selection with handles |

### C — Editor feedback

Owner feedback of 2026-11-05 on the running editor. Touches the options bar and the Colors panel, so build before D, not beside it.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| C4 | [power-user-shortcuts-coverage](power-user-shortcuts-coverage.md) | shell | S | — | Revision 2026-11-05: Ctrl+N in the desktop app and installed PWA |

### D — Image tools

paint.net tools and tool options. After C (shared options bar and Colors panel).

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| D2 | [image-edit-extras](image-edit-extras.md) | Image | L | — | Edit ▸ Fill Selection plus MCP select.fill |
| D4 | [image-palette-gradient](image-palette-gradient.md) | Image | L | color-picker | Load / save / reset Image palette; new crate pincel-palette |

### E — Text and fonts

Installed fonts in both Text tools; the MCP part after `mcp-parity-drawing`.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| E1 | [system-fonts](system-fonts.md) | both | L | — | Installed fonts in both Text tools (fontdb, Local Font Access, font files) |
| E2 | [system-fonts-mcp](system-fonts-mcp.md) | both | M | system-fonts, mcp-parity-drawing | MCP draw.text font argument and font.list |

### F — Image size, view and files

Independent of every other group.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| F2 | [image-view-aids](image-view-aids.md) | Image | L | — | Rulers, pixel grid, units; view state only |
| F4 | [image-formats-more-modern](image-formats-more-modern.md) | Image | L | image-formats-more | AVIF open (after a wasm spike), JPEG XL open / save |
| F5 | [image-formats-more-print](image-formats-more-print.md) | Image | L | — | File ▸ Print… |

### G — Performance

Owner priority; measure first, then the budgets of pincel.md §4.5.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| G1 | [performance-benchmarks](performance-benchmarks.md) | both | L | — | Measure first; budgets every later step proves against |
| G2 | [performance-image-kernel](performance-image-kernel.md) | Image | M | performance-benchmarks | Biggest single win (LUT ≈ 7×); no open questions |
| G3 | [performance-image-cache](performance-image-cache.md) | Image | L | performance-image-kernel | Dab and pan budgets |
| G4 | [performance-input-path](performance-input-path.md) | both | M | performance-benchmarks | Pixel part waits on pixel-pencil |
| G5 | [performance-background-jobs](performance-background-jobs.md) | both | L | performance-benchmarks | Autosave / effect jank off the main thread |
| G6 | [performance-build-simd](performance-build-simd.md) | both | M | performance-image-kernel | Recompose and cold-start budgets |
| G8 | [performance-desktop-jobs](performance-desktop-jobs.md) | both | M | performance-background-jobs | Desktop only |

### H — Other planned features

No shared files with the groups above; pick by priority.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| H1 | [copy-cut-paste](copy-cut-paste.md) | both | L | — | Basic editor equipment |
| H2 | [pixel-grid](pixel-grid.md) | Pixel | M | — | Grid stored in the Aseprite header |
| H3 | [shortcut-overview](shortcut-overview.md) | both | M | — | No open questions |
| H4 | [symmetry-tiled-mode](symmetry-tiled-mode.md) | Pixel | L | pixel-pencil | Seamless tiles |
| H5 | [psd-import](psd-import.md) | Image | L | image-layer-groups | Needs a dependency decision |
| H6 | [rotsprite](rotsprite.md) | Pixel | L | — | |
| H7 | [website-comparison](website-comparison.md) | website | M | — | `website/` has no test harness yet |
| H8 | [reduced-motion](reduced-motion.md) | both | M | — | Accessibility baseline |
| H9 | [animation-preview](animation-preview.md) | Pixel | M | — | |
| H10 | [tablet-gestures](tablet-gestures.md) | both | M | — | |
| H11 | [more-languages](more-languages.md) | both | L | — | |
| H12 | [first-run-tutorial](first-run-tutorial.md) | both | M | — | Better after more-languages if the tour ships translated |
| H13 | [power-user-shortcuts-keymap](power-user-shortcuts-keymap.md) | both | M | shortcut-overview | |
| H14 | [vectorize-core](vectorize-core.md) | both | L | — | Rust port of the owner's Vectorizer (Potrace → SVG) plus a pixel-exact mode; built, awaits `/spec-check`. The Vectorizer repository is deleted; the golden corpus from it is committed (`crates/pincel-trace/tests/golden/`) |
| H15 | [vectorize-dialog](vectorize-dialog.md) | both | L | vectorize-core | File ▸ Export ▸ SVG… with preview; completes the Vectorizer integration |
| H16 | [unsaved-changes-prompt](unsaved-changes-prompt.md) | shell | M | — | Save / Don't Save / Cancel before work is lost; the desktop app stops closing without asking. Supersedes app-shell AC-14 |

### I — AI assistant and image generation

Owner request 2026-11-06: chat with an agent that edits the image (PixelAssist), and image generation with ComfyUI. Desktop only. The chat line needs `mcp-live-desktop` (A8); the ComfyUI line starts independently. Decision Log 2026-11-06.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| I1 | [ai-chat-panel](ai-chat-panel.md) | both | L | mcp-live-desktop | Assistant panel, Claude Code CLI over the live endpoint, one undo step per answer |
| I2 | [ai-chat-clis](ai-chat-clis.md) | both | M | ai-chat-panel | Codex CLI, Gemini CLI, opencode (local models) |
| I3 | [comfyui-connect](comfyui-connect.md) | shell | M | — | Find / start / connect a ComfyUI server, model list |
| I4 | [comfyui-install](comfyui-install.md) | shell | L | comfyui-connect | One-click managed install (uv, PyTorch for NVIDIA / Apple Silicon, model catalog) |
| I5 | [comfyui-generate](comfyui-generate.md) | Image | L | comfyui-connect, mcp-live-desktop | Generate panel, text / image to image, inpainting, variants, `gen.*` tools |
| I6 | [comfyui-generate-pixel](comfyui-generate-pixel.md) | Pixel | M | comfyui-generate | Downscale + palette mapping in Pixel mode |

### J — OCR

Owner request 2026-11-08: recognize text in an image and copy it. A Rust port of Tesseract's LSTM recognizer in a new crate `pincel-ocr` (Decision Log 2026-11-08). Shares no files with the other groups until J3 touches Image mode's menu and job worker.

| # | Spec | Mode | Size | Depends on | Note |
| - | ---- | ---- | ---- | ---------- | ---- |
| J1 | [ocr-recognizer](ocr-recognizer.md) | Image | L | — | Model reader, int8 LSTM, beam search for one line; golden tests against Tesseract 5.5 |
| J2 | [ocr-page](ocr-page.md) | Image | L | ocr-recognizer | Binarization, skew, lines and blocks, page text; accuracy on a page corpus |
| J3 | [ocr-dialog](ocr-dialog.md) | Image | L | ocr-page | Tools ▸ Recognize Text…, model download on first use, copy to clipboard |

## Done

- [adjustments-paintnet](adjustments-paintnet.md)
- [animated-export](animated-export.md)
- [app-shell](app-shell.md)
- [autosave-recovery](autosave-recovery.md)
- [clone-stamp](clone-stamp.md)
- [color-picker](color-picker.md)
- [dithered-gradient](dithered-gradient.md)
- [document-tabs](document-tabs.md)
- [effect-catalog-v2](effect-catalog-v2.md)
- [effect-panel](effect-panel.md)
- [effects-adjustments](effects-adjustments.md)
- [effects-artistic-photo](effects-artistic-photo.md)
- [effects-blurs](effects-blurs.md)
- [effects-distort](effects-distort.md)
- [effects-object-stylize-color-noise](effects-object-stylize-color-noise.md)
- [effects-render](effects-render.md)
- [history-panel](history-panel.md)
- [image-edit-extras-tool-options](image-edit-extras-tool-options.md)
- [image-formats-more](image-formats-more.md)
- [image-editing](image-editing.md)
- [image-files](image-files.md)
- [image-layer-groups-ora](image-layer-groups-ora.md)
- [image-layer-groups](image-layer-groups.md)
- [image-live-text](image-live-text.md)
- [image-palette-gradient-spiral](image-palette-gradient-spiral.md)
- [image-shapes-curves](image-shapes-curves.md)
- [image-shapes-curves-shapes](image-shapes-curves-shapes.md)
- [image-resize-autocrop](image-resize-autocrop.md)
- [image-transform-handles-rotate-zoom](image-transform-handles-rotate-zoom.md)
- [indexed-grayscale-compose](indexed-grayscale-compose.md)
- [linked-cels-group-compose](linked-cels-group-compose.md)
- [mcp-headless-image](mcp-headless-image.md)
- [mcp-headless-pixel](mcp-headless-pixel.md)
- [mcp-parity-drawing](mcp-parity-drawing.md)
- [mcp-parity-drawing-image](mcp-parity-drawing-image.md)
- [mcp-parity-export](mcp-parity-export.md)
- [mcp-parity-pixel-assets](mcp-parity-pixel-assets.md)
- [mcp-parity-selection-transform](mcp-parity-selection-transform.md)
- [mcp-parity-selection-transform-content](mcp-parity-selection-transform-content.md)
- [mcp-parity-structure](mcp-parity-structure.md)
- [outline-effect](outline-effect.md)
- [palette-formats](palette-formats.md)
- [pen-input](pen-input.md)
- [performance-image-kernel](performance-image-kernel.md)
- [performance-image-undo-memory](performance-image-undo-memory.md)
- [pixel-compose](pixel-compose.md)
- [pixel-drawing-tools](pixel-drawing-tools.md)
- [pixel-export](pixel-export.md)
- [pixel-layers-frames](pixel-layers-frames.md)
- [pixel-palette](pixel-palette.md)
- [pixel-pencil](pixel-pencil.md)
- [pixel-pressure](pixel-pressure.md)
- [pixel-selection-move](pixel-selection-move.md)
- [pixel-undo-history](pixel-undo-history.md)
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
| Image-mode Save while the previous Save finishes | `saveTab` silently drops a Save / Save As while the previous one still writes the autosave snapshot and recents; queue it or show that a save is running (found by `save-original-format`'s acceptance) |
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
| AI image generation: outpainting, upscaling, control layers (ControlNet), live painting | After comfyui-generate (owner, 2026-11-06); krita-ai-diffusion shows the scope |
| ComfyUI custom workflows | Import API-format workflows with marked placeholders; after comfyui-generate |
| ComfyUI managed install for AMD (ROCm), Intel (XPU), CPU | After comfyui-install; v1 is NVIDIA and Apple Silicon |
| Agent vision tools: object detection, segmentation, style transfer | Need local vision models (e.g. SAM); the PixelAssist prompt names them as unavailable |
| Shared shell package (`@amigo-labs/canvas-shell`) | Extract `fs` / `idb` / PWA / Tauri wrapper for other amigo-labs tools; cut: what knows the document model stays in the app |
| Aseprite round-trip gaps | Layer flags Background / Reference / Collapsed / LockMovement, cel `z_index`, user data on layers / cels / sprite / tags, slice note text and properties |
| Mid-list AddFrame / duplicate frame | Append-only today; needs a FrameIndex remap of cels, tags, slices |
| Selection in the undo stack | Selection lives on `Sprite` outside commands; Aseprite tracks it |
| Multi-frame slice editing | The panel edits frame-0 keys only |
| Per-command dirty regions, per-tile dirty events | Undo / redo and `setTilePixel` emit full-canvas dirty |
| `pincel-wasm` errors as `JsError` | Returns `Result<_, String>` today |
| Scope Tauri file commands | `read_file_bytes` / `write_file_bytes` accept any path from the webview |
| Live-edit speed in Image mode | Proposed budgets 8 ms (ellipse handle edit) and 16 ms (gradient); today 200 ms / 767 ms on 4096² / 2048² with 5 layers. Needs span-based shape fill, a cheaper gradient pass and a faster region compose; budgets join `performance-benchmarks` (image-live-objects Decisions, 2026-11-06) |
| Error surfacing | Failures only reach the transient status bar; a toast area would be harder to miss |
