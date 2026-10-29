---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §3.1, pincel.md §3.4, pincel.md §3.7, pincel.md §4.1, pincel.md §4.2, pincel.md §14]
---

# Compose indexed and grayscale sprites

## Context

`compose()` — the single render entry point of `pincel-core` (spec §4.1) — rejects every sprite whose color mode is not RGBA with `RenderError::UnsupportedColorMode`. The document model already represents Indexed (8 bpp palette indices plus a transparent index) and Grayscale (16 bpp value + alpha) sprites (§3.1), and the design already says how to render them: "Convert indexed pixels to RGBA at composite time (palette lookup)" (§4.2). Everything downstream of `compose()` — the canvas, layer thumbnails, onion skin, PNG frame and atlas export — fails the same way for such sprites.

This spec makes `compose()` render both modes. It is the renderer prerequisite for opening indexed and grayscale `.aseprite` files, which is a separate codec change (see Out of scope).

What the Aseprite file format reference (<https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>, linked from `crates/aseprite-writer/src/write.rs`) fixes:

- Header: the transparent index is the "Palette entry (index) which represent transparent color in all non-background layers (only for Indexed sprites)".
- Pixels: Indexed uses one byte per pixel (the index); Grayscale uses two bytes per pixel "in the order Value, Alpha".
- Layer flag `8` marks the background layer (kept in Pincel as `Layer::extra_flags` bit `0x8`).
- Palette entries carry an alpha byte (0–255), which Pincel's palette keeps.

Already decided: blend modes are computed in u8 sRGB and Aseprite pixel-exact parity is out of scope (Decision Log 2026-07-09, 2026-10-27); indexed painting is Phase 2, so palette remove / reorder / replace are refused on indexed sprites while recolouring an entry is allowed (§3.7, Decision Log 2026-10-11); §3.1 marks Grayscale as "Phase 2".

## Scope

### In scope

- `compose()` renders Indexed sprites by palette lookup, honouring the transparent index and the background layer flag
- `compose()` renders Grayscale sprites by expanding value + alpha to RGBA
- Both for image cels and for tilemap cels (tile images in the sprite's color mode), under every blend mode, layer and cel opacity, zoom, viewport / dirty rect and onion skin
- Everything built on `compose()` — PNG frame and atlas export, layer thumbnails — works for both modes without further change

### Out of scope

- Painting into indexed or grayscale sprites, including the choice between a pixel-payload enum on `SetPixel` and a separate indexed-pixel command — because indexed painting is Phase 2 (§3.7, Decision Log 2026-10-11) and rendering reads pixels without depending on how they were written
- Opening and saving indexed or grayscale `.aseprite` files — because the read and write adapters reject both modes today (`CodecError::UnsupportedColorMode`); lifting that is codec work with its own round-trip fixtures, built on top of this spec
- Color-mode conversion (RGB ↔ Indexed ↔ Grayscale) — because it belongs with indexed painting (backlog in `docs/specs/README.md`)
- Bit-exact parity with Aseprite's renderer — because no Aseprite-generated reference fixture exists to cross-validate against; tests assert hand-derived values
- An indexed-rendering benchmark or a GPU palette-LUT path (§14 question 1) — because performance budgets are owned by the performance work (`performance-benchmarks.md`)
- Cel z-index — because `compose()` doesn't apply it in any color mode yet (§7.1)

## Acceptance criteria

- [ ] AC-01 (event): When an Indexed sprite is composed, then every pixel whose index differs from the transparent index renders as its palette entry's RGBA — including the entry's alpha — before layer opacity, cel opacity and the layer's blend mode are applied.
- [ ] AC-02 (event): When an Indexed sprite is composed, then a pixel equal to the transparent index on a layer that is not flagged as background contributes nothing to the output.
- [ ] AC-03 (event): When an Indexed sprite is composed, then a pixel equal to the transparent index on a layer flagged as background renders as that index's palette entry.
- [ ] AC-04 (event): When a Grayscale sprite is composed, then every pixel with value V and alpha A renders as RGBA (V, V, V, A) before layer opacity, cel opacity and blend mode are applied.
- [ ] AC-05 (always): For an Indexed or Grayscale sprite, `compose()` returns the same output as for an RGBA sprite with the same structure whose cels hold the per-pixel RGBA values that AC-01 … AC-04 define — for every blend mode, opacity, zoom, viewport, dirty hint and onion-skin setting.
- [ ] AC-06 (event): When an Indexed or Grayscale sprite has a tilemap layer, then its tiles render with the same per-pixel rules as image cels, and tile id 0 stays empty.
- [ ] AC-07 (event): When a palette entry of an Indexed sprite is recoloured and the sprite is composed again, then pixels with that index show the new colour, and after undo they show the old colour again.
- [ ] AC-08 (event): When a frame of an Indexed or Grayscale sprite is exported as PNG or as part of an atlas, then the export succeeds and its pixels equal `compose()`'s output for that frame.
- [ ] AC-09 (error): If a cel's or tile's pixel buffer uses a color mode other than the sprite's, then `compose()` fails with `RenderError::CelColorModeMismatch` naming the layer and frame.
- [ ] AC-10 (error): If a cel's or tile's pixel buffer length doesn't match its dimensions for the sprite's color mode (1 byte per pixel Indexed, 2 Grayscale), then `compose()` fails with `RenderError::MalformedCelBuffer` naming the layer and frame, as for RGBA.
- [ ] AC-11 (always): `compose()` no longer returns `RenderError::UnsupportedColorMode` for any color mode the document model can represent.

## Technical notes

Hints from a 2026-10-29 code check; line numbers drift — grep for the names.

- **Rejection today**: `crates/pincel-core/src/render/compose.rs` ≈L42 (`sprite.color_mode != ColorMode::Rgba` → `UnsupportedColorMode`); test `rejects_indexed_color_mode` (≈L403) must change. `RenderError::UnsupportedColorMode` lives in `render/error.rs`; once no mode is rejected the variant is dead — removing it is a public API change of `pincel-core` (the 2026-10-11 onion-skin entry removed `OnionSkinUnsupported` the same way).
- **Per-cel paths**: `render/image_layer.rs` (`composite_image_cel`, RGBA source assumed) and `render/tilemap_layer.rs` (`composite_tilemap_cel` takes `sprite_color_mode`, already checks each tile's mode ≈L72). Blending is `render/blend.rs` (`blend_pixel_into`) on RGBA8. Onion skin (`render/onion.rs`) reuses `composite_visible_layers`, so it inherits the change.
- **Model**: `ColorMode` (`document/color_mode.rs`): `Rgba`, `Indexed { transparent_index: u8 }`, `Grayscale`; `bytes_per_pixel` 4 / 1 / 2. `PixelBuffer { width, height, color_mode, data }` with `is_well_formed()` (`document/cel.rs`). `Palette { colors: Vec<PaletteEntry> }`, `PaletteEntry { rgba, name }` (`document/palette.rs`). Background flag: `Layer::extra_flags` bit `0x8` (`document/layer.rs` ≈L119).
- **Codec state** (contradicts the old prompt's "indexed exists in the codec"): `codec/aseprite_read.rs` `map_color_mode` (≈L159) and `codec/aseprite_write.rs` both return `CodecError::UnsupportedColorMode` for Indexed and Grayscale. The reader pads palette gaps with transparent entries (≈L559). `aseprite-writer` itself already knows `ColorDepth::Indexed` / `Grayscale` and writes `transparent_index` in the header. `pincel.md` §18.2 lists Pincel's Indexed support as "~ read / write", which the code does not back.
- **PNG export**: `codec/png.rs` `export_frame_png` / `export_atlas_png` go through `compose()`; their doc comment lists Indexed / Grayscale as a limitation to drop.
- **Tests**: in-memory sprites via `Sprite::builder(..).color_mode(..)`, helpers in `render/test_support.rs`. AC-05 suits a `proptest` comparing an indexed sprite with its RGBA expansion (CLAUDE.md §7.3).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Transparent index is outside the palette (e.g. 255 with a 32-entry palette) | Pixels equal to it are still transparent on non-background layers (AC-02) |
| Palette entry with alpha < 255 on a non-background layer | Renders with that alpha, then layer / cel opacity multiply on top (AC-01) |
| Background layer whose transparent-index entry has alpha 0 | Renders as that entry, i.e. transparent — the flag only stops the index from being special |
| Grayscale pixel with A = 0 | Contributes nothing, whatever V is |
| A layer flagged background in a Grayscale or RGBA sprite | The flag changes nothing; there is no transparent index |
| Sub-viewport or dirty hint over an Indexed sprite | Same pixels as the matching region of a full compose (AC-05) |
| Palette recoloured with undo merging (drag) | Compose after the drag shows the final colour; one undo restores the original (AC-07) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| On a background layer the transparent index renders as its palette colour | The format reference scopes the transparent index to "non-background layers" and says nothing more about background layers | Low–medium: Aseprite's renderer might special-case it further; no Aseprite reference fixture exists to check |
| Grayscale renders as (V, V, V, A) with no gamma or palette step | The format reference defines the pixel as Value, Alpha and nothing else | Low |

## Open questions

- Grayscale is marked "Phase 2" in §3.1 (and in `ColorMode`'s doc comment). Rendering it now mixes Phase 1 and Phase 2 (CLAUDE.md §9) unless a Decision Log entry pulls Grayscale *rendering* into Phase 1 — write that entry, or drop AC-04 and narrow AC-11 so Grayscale stays rejected?
- What does an index at or past the end of the palette (and not the transparent index) render as: transparent, or a structured `RenderError` like `TileIdOutOfRange`? The format reference doesn't say; transparent keeps a file with one stray index viewable, an error surfaces corrupt data.

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |

## Decisions

- The palette is looked up at compose time, not cached into RGBA cels — §4.2 prescribes it, and it makes palette recolours show immediately (AC-07) without touching cel data.
- The transparent-index and background-layer rules come from the Aseprite format reference, not from Aseprite's renderer source — the reference is the contract the codec already follows; renderer parity is out of scope.
- The SetPixel payload question from the old prompt is not decided here — rendering does not depend on it, and indexed painting is Phase 2.
