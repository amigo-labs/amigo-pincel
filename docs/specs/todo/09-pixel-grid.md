---
id: todo-pixel-grid
title: Pixel grid and custom grid in Pixel mode
status: todo
size: M (one session)
mode: Pixel
depends_on: []
created: 2026-10-28
---

# Pixel grid and custom grid in Pixel mode

## Goal

- **View ▸ Show Pixel Grid** draws thin lines between pixels. It only shows from a zoom where the cells are big enough: ≥ 8 screen px per pixel.
- **View ▸ Show Grid** draws a configurable grid on top:
  - cell size W×H (default 16×16) and offset X/Y;
  - the color and opacity of the grid lines.
- **View ▸ Snap to Grid** is optional; see "Out of scope".

## Why

For pixel art this is basic equipment: Aseprite, Pixelorama and Pro Motion have both grids, Piskel has the pixel grid. In Pincel, a grid exists only as the **tile grid while stamping tiles**.

## Current state (read these first)

**Overlays.** `ui/src/modes/pixel/overlay-painter.ts`
- `OverlayState` (≈L29–50) and `paintOverlays` (≈L53).
- **`paintTileGrid` (≈L209)** already draws a grid at the tileset's tile size in screen space. Reuse its sprite → screen mapping (zoom and pan) and its line style.
- The overlay is a separate canvas above the render target (`ui/src/lib/render/types.ts`).

**Rendering.** `ui/src/modes/pixel/render.ts`: `overlayState()` (≈L84), `paintOverlays` (≈L120), the rAF `tick` (≈L168). The overlay redraws on view changes: zoom, pan, resize.

**View state.** `viewInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L62–80) has no grid fields yet.

**View menu.** `ui/src/modes/pixel/menus.ts` (≈L224–235). The **Probe** item (`checked: c.probeOn`) is the pattern for a checkbox entry.

**Persistence.** IndexedDB prefs (`ui/src/lib/idb/prefs.ts`); the per-mode layout pref is in `ui/src/lib/components/layout/panels.svelte.ts`.

**File format.** The Aseprite header has grid bounds (x, y, width, height; see the format reference linked in `crates/aseprite-writer`). **Check** whether `aseprite-loader` exposes them, and whether `crates/pincel-core/src/codec/aseprite_write.rs` writes them today. `grep -n -i grid` in `pincel-core` finds only tilemap grids.

## Design

**Rendering.**
- Both grids are drawn on the **overlay canvas**, only over the visible area.
- Lines are 1 device pixel wide, snapped to device pixels, so they stay crisp at any zoom and on HiDPI screens.
- Pixel grid: a low-contrast line (`pc-*` token), hidden below the zoom threshold.
- Custom grid: a stronger line in the user color. It draws on top of the pixel grid.
- Cost: draw lines only over the viewport, so it is O(visible lines), never O(sprite size).

**Grid settings.** View ▸ Grid Settings… opens a small dialog with size, offset, color and opacity. The tile size of the active tileset can be offered as a preset.

**Where the grid is stored.**
- **Recommendation: in the document.** The grid fields go into `Sprite` (`pincel-core`) and are read and written through the Aseprite header grid fields. That round-trips with Aseprite, and per-sprite grids are what pixel artists expect.
  - This needs a Decision Log entry and an `aseprite-writer` change in its own commit (CLAUDE.md §9: never mix writer and app changes in one commit).
- Show / hide flags are view state, persisted in the per-mode prefs, not in the file.

**Keys.**
- Ctrl+' toggles the grid, and Ctrl+Shift+' toggles the pixel grid (`ui/src/lib/shell/shortcuts.ts`, Aseprite-like).
- Check against `21-power-user-shortcuts.md`.

## Tasks (one M session)

1. **`docs(spec)`.** In §9.2, add the View menu entries and grid rendering. Decision Log entry: grid stored in the sprite through the Aseprite header; show flags kept in prefs.
2. **`feat(aseprite-writer)`**, if the writer lacks the header grid fields, plus a round-trip test.
3. **`feat(pincel-core)` + `feat(pincel-wasm)`.**
   - `Sprite.grid: GridSpec { x, y, w, h }`: read, write and set with an undoable command, so changing the grid can be undone like other document edits.
   - Tests:
     - `grid_round_trips_through_aseprite`
     - `set_grid_is_undoable`
4. **`feat(ui)`.**
   - Overlay drawing, menu entries, the settings dialog, keys and i18n.
   - e2e `ui/e2e/grid.spec.ts`:
     - toggling sets the menu checkbox state;
     - at zoom ≥ 8 the overlay screenshot differs with the pixel grid on versus off (element screenshots, §7.4);
     - grid settings survive Save and reopen (download, then reopen, the pattern in `ui/e2e/ora.spec.ts` / the aseprite specs).

## Out of scope

- Snap-to-grid for moves and shapes (a follow-up).
- An isometric grid.
- Grids in Image mode (rulers and guides are in the backlog).
