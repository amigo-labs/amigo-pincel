---
status: todo
created: 2026-10-29
size: M
mode: Pixel
design: [pincel.md §4.1, pincel.md §4.2, pincel.md §4.3, pincel.md §4.4, pincel.md §15]
---

# Pixel compose and render adapters

## Context

Every Pixel-mode picture — the canvas, PNG frame export, sprite-sheet export — comes out of one function, `pincel_core::compose()` (`pincel.md` §4.1). It flattens one frame of a sprite into a non-premultiplied RGBA8 buffer: layers bottom-up in z-order, each under its own opacity and blend mode, tilemap cels expanded through their tileset, optionally with an onion skin of the neighbouring frames underneath. The UI shows that buffer through a backend-agnostic `CanvasRenderer` (WebGPU, or Canvas2D as the universal fallback) and draws transient UI furniture on a separate overlay canvas (§4.4).

This spec records the behavior as built. Already decided (Decision Log, §15): every blend mode follows the W3C compositing formula with Pincel's own integer rounding, pixel-exact parity with Aseprite is out of scope (2026-10-27); the onion-skin tint math (2026-10-11).

## Scope

### In scope

- The `compose()` contract: frame, viewport, integer zoom, layer filter, layer and cel opacity, all 19 blend modes, group visibility, tilemap layers, onion skin, dirty-rect hint
- The errors `compose()` raises today, including the inputs it does not support yet (non-RGBA color modes, linked cels, overlays)
- The Pixel-mode render adapters: WebGPU / Canvas2D backend choice, the force-Canvas2D switch, the overlay canvas, recompose on document change

### Out of scope

- Indexed and grayscale composition — because the planned `indexed-grayscale-compose` spec covers it; today they are refused (AC-19)
- Linked cels and folding a group's opacity / blend mode into its children — because the planned `linked-cels-group-compose` spec covers them; today linked cels are refused (AC-21) and group opacity / blend mode have no effect
- Compose-side overlays (grid, slices, selection marquee in the composed buffer) — because the UI draws them on its own overlay canvas today; a request for them is refused (AC-22)
- Onion-skin, layer and timeline controls in the UI — because the `pixel-layers-frames` spec owns them; this spec covers what `compose()` does with the settings
- Frame-time targets, profiling and the dirty-rect fast path's speed — because the planned `performance-*` specs own them
- Image-mode composition — because it lives in `pincel-image-core` and the `image-editing` spec

## Acceptance criteria

- [ ] AC-01 (always): `compose()` returns a non-premultiplied RGBA8 buffer, row-major, whose width and height are the rendered region's width and height times `zoom`, with every sprite pixel repeated as a `zoom`×`zoom` block (nearest neighbour).
- [ ] AC-02 (always): Layers composite bottom-up in z-order: an opaque pixel of an upper `Normal` layer replaces the pixel below, and a translucent one is blended source-over onto it.
- [ ] AC-03 (always): A cel pixel's effective alpha is its own alpha scaled by the layer opacity and by the cel opacity.
- [ ] AC-04 (always): Only the cels of the requested frame are drawn; a layer without a cel on that frame contributes nothing, and a cel reaching past the viewport is clipped to it.
- [ ] AC-05 (option): Where a viewport smaller than the canvas is requested, the buffer holds exactly that region of the frame, at the requested zoom.
- [ ] AC-06 (option): Where the layer filter is `Visible` (the default), a layer is drawn only if it and every ancestor group are visible.
- [ ] AC-07 (option): Where the layer filter is `All`, every layer is drawn, including hidden layers and children of hidden groups.
- [ ] AC-08 (option): Where the layer filter is `Only(ids)`, exactly the listed layers are drawn, in the sprite's z-order.
- [ ] AC-09 (always): A group layer contributes no pixels of its own; its children are drawn as separate layers.
- [ ] AC-10 (always): Under every non-`Normal` blend mode, a source over a fully transparent backdrop yields the source unchanged, and a fully transparent source leaves the backdrop unchanged.
- [ ] AC-11 (always): An opaque source over an opaque backdrop yields the W3C blend term per channel for Multiply, Screen, Overlay (HardLight with the operands swapped), Darken, Lighten, Addition (saturating at 255) and Subtract (clamping at 0).
- [ ] AC-12 (always): An opaque source over an opaque backdrop yields the W3C / Aseprite blend term per channel for ColorDodge, ColorBurn, HardLight, SoftLight, Difference, Exclusion and Divide.
- [ ] AC-13 (always): Hue, Saturation, Color and Luminosity yield the W3C non-separable blend term (within ±1 per channel of an independent float reference); Hue, Saturation and Color keep the backdrop's luminance, Luminosity takes the source's.
- [ ] AC-14 (always): A translucent source under a non-`Normal` mode mixes the blend term with the backdrop by the W3C compositing formula.
- [ ] AC-15 (always): A tilemap cel draws each grid cell's tile at the cel position plus the cell's grid offset, skips tile id 0, and applies `rotate_90` (clockwise) first, then `flip_x`, then `flip_y`.
- [ ] AC-16 (option): Where an onion skin is requested, up to `frames_back` previous and `frames_forward` next frames are drawn under the current frame — previous frames with each colour channel moved halfway toward red, next frames halfway toward blue — and opaque pixels of the current frame hide them.
- [ ] AC-17 (option): Where an onion skin spans several frames, the nearest neighbour is drawn at the configured alpha and the one at distance `k` of `n` at `alpha · (n − k + 1) / n`; frames before the first or after the last frame are skipped without wrap-around.
- [ ] AC-18 (option): Where a dirty hint is given, the buffer covers only the intersection of viewport and hint, reports that rect's sprite-coordinate origin, and its pixels equal the same region of a full render; a hint disjoint from the viewport yields an empty buffer and no error.
- [ ] AC-19 (error): If the sprite's color mode is not RGBA, then `compose()` fails with `UnsupportedColorMode`.
- [ ] AC-20 (error): If `zoom` is 0 or above 64, the viewport is empty, or the frame index is past the last frame, then `compose()` fails with `InvalidZoom`, `EmptyViewport` or `UnknownFrame`; zoom 64 succeeds.
- [ ] AC-21 (error): If a drawn layer has a linked cel on the requested frame, then `compose()` fails with `LinkedCelUnsupported`.
- [ ] AC-22 (error): If any overlay (grid, slices, selection marquee) is requested, then `compose()` fails with `OverlaysUnsupported`.
- [ ] AC-23 (error): If a drawn cel is corrupt — buffer colour mode differs from the sprite's, buffer length doesn't match its size, cel kind doesn't match the layer kind, the tileset is missing, a tile id is past the tileset, a tilemap grid doesn't match its tile count, or `rotate_90` is set on a non-square tileset — then `compose()` fails with the matching `RenderError` instead of drawing.
- [ ] AC-24 (error): If a tile's image size differs from its tileset's tile size, then `compose()` fails with `TileSizeMismatch`.
- [ ] AC-25 (event): When the Pixel editor opens, it shows the canvas through WebGPU if the browser provides a working WebGPU device, else through Canvas2D, and the status bar names the active backend (`webgpu` / `canvas2d`).
- [ ] AC-26 (option): Where the page URL carries `?renderer=canvas2d`, the Pixel editor uses Canvas2D even when WebGPU is available.
- [ ] AC-27 (event): When the document changes (paint, undo / redo, frame switch, layer visibility), the canvas shows the newly composed frame without further user action.
- [ ] AC-28 (state): While a drag preview, selection marquee, tile-stamp hover or active-slice accent is shown, it is drawn on a layer above the sprite and never becomes part of the document's pixels or of an exported frame.

## Technical notes

> Paths are hints from 2026-10-29 and drift — grep for the names.

- Core: `crates/pincel-core/src/render/` — `compose.rs` (entry point, layer loop, `upscale_nearest_into`, `effectively_visible`), `blend.rs` (`blend_pixel_into`, `composite_into`, `blend_channel`, `blend_nonseparable`), `image_layer.rs`, `tilemap_layer.rs`, `onion.rs`, `request.rs`, `error.rs`.
- Signature as built (differs from the §4.1 sketch: the caller owns the output buffer and `ComposeResult` carries no pixels):

```rust
pub fn compose(
    sprite: &Sprite,
    cels: &CelMap,
    request: &ComposeRequest,
    out: &mut Vec<u8>,
) -> Result<ComposeResult, RenderError>;

pub struct ComposeResult { pub width: u32, pub height: u32, pub dirty_rect: Rect, pub generation: u64 }
pub enum LayerFilter { Visible, All, Only(Vec<LayerId>) }
pub struct OnionSkin { pub previous_alpha: u8, pub next_alpha: u8, pub frames_back: u32, pub frames_forward: u32 } // default 76, 76, 1, 1
```

- `generation` is always `0`; the UI keeps its own counter. `ComposeRequest::full(frame, w, h)` = full canvas, zoom 1, `Visible`, no onion, no overlays, no dirty hint.
- `RenderError` variants: `UnsupportedColorMode`, `InvalidZoom`, `EmptyViewport`, `UnknownFrame`, `LinkedCelUnsupported`, `CelColorModeMismatch`, `MalformedCelBuffer`, `CelTypeMismatch`, `OverlaysUnsupported`, `TilesetNotFound`, `TileIdOutOfRange`, `TileSizeMismatch`, `NonSquareRotateUnsupported`.
- Group visibility walk is capped at `sprite.layers.len()` hops (a parent cycle terminates); a dangling parent id ends the walk and the layer still renders.
- wasm (`crates/pincel-wasm/src/lib.rs`): `compose(frame, zoom)` and `composeDirty(frame, zoom, dx, dy, dw, dh)` return a `ComposeFrame` (`width`, `height`, `dirtyX`, `dirtyY`, `pixels` — copied today); both use the full canvas, `Visible`, and the onion skin set by `setOnionSkin(enabled, back, forward, prevAlpha, nextAlpha)` (`timeline.rs`).
- UI: `ui/src/lib/render/` (`types.ts` `CanvasRenderer { backend, draw(frame), drawDirty(frame), destroy() }`, `webgpu-renderer.ts`, `canvas2d-renderer.ts`, `canvas2d.ts`), `ui/src/modes/pixel/render.ts` (`createRenderer`, `forceCanvas2d`, `recompose`, `recomposeDirty`, `tick`), `overlay-painter.ts`. The UI always composes at zoom 1; display zoom is CSS (`image-rendering: pixelated`) on a sprite-sized backing store. `tick()` drains wasm events once per animation frame; `dirty-rect` events union into one `composeDirty` call only while no overlay is live, anything else takes the full path.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Cel positioned partly or fully outside the canvas | Only the in-canvas part is drawn; no error |
| Tileset with a zero tile width or height | The tilemap cel draws nothing; no error |
| Layer whose parent id points to no layer | The layer renders under `Visible` |
| `frames_back` larger than the current frame index | Clamped to the frames that exist; no wrap-around |
| Onion alpha 0 on both sides | Output equals a render without onion skin |
| Dirty hint partly outside the viewport | Clamped to the overlap |
| Sprite with no cels at all | Fully transparent buffer of the requested size |
| WebGPU device lost after start | Further draws are no-ops until reload (no hot swap to Canvas2D) |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pincel-core: render::compose::tests::zoom_duplicates_pixels_horizontally_and_vertically`, `render::compose::tests::zoom_3_produces_9x_pixel_count`, `render::image_layer::tests::opaque_cel_matches_source` | built | |
| AC-02 | `pincel-core: render::compose::tests::opaque_top_layer_overrides_bottom`, `render::compose::tests::translucent_top_blends_with_bottom`; `pincel-core (tests/render_compose.rs): end_to_end_two_layer_sprite_composes_to_expected_pixels` | built | |
| AC-03 | `pincel-core: render::image_layer::tests::layer_opacity_scales_alpha`, `render::image_layer::tests::cel_opacity_scales_alpha` | built | |
| AC-04 | `pincel-core (tests/render_compose.rs): frame_navigation_picks_per_frame_cel_data`; `pincel-core: render::image_layer::tests::cel_clipped_to_viewport` | built | |
| AC-05 | `pincel-core: render::image_layer::tests::viewport_offset_renders_subregion`; `pincel-core (tests/render_compose.rs): viewport_subregion_with_zoom_returns_zoomed_subregion` | built | |
| AC-06 | `pincel-core: render::compose::tests::invisible_layer_is_skipped`, `render::compose::tests::hidden_group_hides_child_layers`, `render::compose::tests::hidden_grandparent_hides_nested_child` | built | |
| AC-07 | `pincel-core: render::compose::tests::layer_filter_all_renders_invisible`, `render::compose::tests::layer_filter_all_ignores_hidden_group` | built | |
| AC-08 | `pincel-core: render::compose::tests::layer_filter_only_renders_subset`, `layer_filter_only_draws_in_sprite_z_order_not_list_order` | built | |
| AC-09 | `pincel-core: render::compose::tests::group_layer_is_skipped_and_children_render` | built | |
| AC-10 | `pincel-core: render::blend::tests::blend_over_transparent_backdrop_equals_source`, `transparent_source_leaves_backdrop_unchanged` | built | |
| AC-11 | `pincel-core: render::blend::tests::multiply_blends_channels`, `screen_blends_channels`, `overlay_matches_hardlight_swapped`, `darken_takes_min_and_lighten_takes_max`, `addition_saturates_at_255` | built | |
| AC-12 | `pincel-core: render::blend::tests::separable_modes_match_w3c_reference_for_every_channel_pair`, `separable_modes_hit_their_exact_terms_at_known_points`, `separable_modes_compose_opaque_source_as_blend_term` | built | |
| AC-13 | `pincel-core: render::blend::tests::hsl_modes_match_w3c_reference_values`, `hsl_modes_compose_opaque_source_as_blend_term`, `hsl_modes_keep_the_luminance_they_promise` | built | |
| AC-14 | `pincel-core: render::blend::tests::blend_with_translucent_source_matches_w3c_formula`, `hsl_modes_with_translucent_source_mix_like_separable_modes` | built | |
| AC-15 | `pincel-core: render::tilemap_layer::tests::tilemap_renders_tile_at_grid_position`, `tilemap_cel_position_offsets_tile_placement`, `tilemap_empty_tile_id_zero_is_skipped_without_consulting_tileset`, `tilemap_flip_x_mirrors_tile_horizontally`, `tilemap_flip_y_mirrors_tile_vertically`, `tilemap_rotate_90_rotates_clockwise`, `tilemap_combined_flags_rotate_then_flip_x_then_flip_y` | built | |
| AC-16 | `pincel-core: render::onion::tests::previous_frame_shows_red_tinted_under_a_hole`, `next_frame_is_blue_tinted_at_next_alpha`, `opaque_current_frame_hides_the_onion`, `previous_frame_tint_moves_each_channel_halfway_to_red`, `next_frame_tint_moves_each_channel_halfway_to_blue` | built | |
| AC-17 | `pincel-core: render::onion::tests::farther_frames_fade_linearly`, `first_frame_without_neighbours_renders_plainly`, `first_frame_does_not_wrap_to_the_last_frame`, `last_frame_does_not_wrap_to_the_first_frame` | built | |
| AC-18 | `pincel-core: render::compose::tests::dirty_hint_inside_viewport_renders_only_intersection`, `dirty_hint_partially_outside_viewport_clamps_to_overlap`, `dirty_hint_disjoint_from_viewport_returns_empty_buffer`, `dirty_hint_equal_to_viewport_matches_full_render`, `dirty_hint_with_zoom_upscales_only_intersection` | built | |
| AC-19 | `pincel-core: render::compose::tests::rejects_indexed_color_mode` | built | |
| AC-20 | `pincel-core: render::compose::tests::rejects_invalid_zoom`, `zoom_at_max_factor_succeeds`, `rejects_empty_viewport`, `rejects_unknown_frame` | built | |
| AC-21 | `pincel-core: render::compose::tests::rejects_linked_cel` | built | |
| AC-22 | `pincel-core: render::compose::tests::rejects_overlays_request`, `rejects_slices_or_selection_marquee_overlay_alone` | built | |
| AC-23 | `pincel-core: render::compose::tests::rejects_cel_with_wrong_color_mode`, `rejects_malformed_cel_buffer`, `rejects_tilemap_data_on_image_layer`, `tilemap_image_cel_on_tilemap_layer_errors`; `render::tilemap_layer::tests::tilemap_missing_tileset_errors`, `tilemap_dangling_tile_id_errors`, `tilemap_malformed_cel_buffer_errors`, `tilemap_grid_whose_tile_count_overflows_usize_errors`, `tilemap_rotate_90_on_non_square_errors` | built | |
| AC-24 | `pincel-core: render::tilemap_layer::tests::tilemap_tile_with_wrong_image_size_errors` | built | |
| AC-25 | `ui/e2e/renderer.spec.ts`: `pixel mode: a working WebGPU device draws the canvas through WebGPU`, `pixel mode: without a WebGPU adapter the canvas falls back to Canvas2D` | built | |
| AC-26 | `ui/e2e/renderer.spec.ts`: `pixel mode: ?renderer=canvas2d forces Canvas2D although WebGPU works` | built | |
| AC-27 | `ui/e2e/compose.spec.ts`: `pixel mode: the canvas follows paint, undo / redo, frames and layer visibility` | built | |
| AC-28 | `ui/e2e/compose.spec.ts`: `pixel mode: a drag preview and the selection marquee stay off the sprite`, `pixel mode: the tile-stamp hover and the active slice stay off the sprite` | built | |

## Decisions

- AC-12: the blend term is checked against an independent float reference of the W3C / Aseprite formulas with a tolerance of ±1 per channel (exhaustively over all 256×256 channel pairs), plus exact hand-derived values at each mode's clamps — because the Decision Log (2026-10-27) fixes Pincel's own integer rounding, not bit-exact parity.
- AC-12: the three new tests were green at once (the behavior exists). A temporary break of the Exclusion term (`2·Cb·Cs` → `Cb·Cs`) turned all three red (`Exclusion(255, 255)`, `Exclusion channel 0: 209 vs reference 177`, `Exclusion(Cb=2, Cs=160) = 161, reference 159`); restored afterwards.
- AC-24: `tilemap_tile_with_wrong_image_size_errors` was green at once (the behavior exists). A temporary `if false &&` on the tile-size check in `composite_tilemap_cel` turned it red; restored afterwards.
- AC-25 / AC-26: headless Chromium (local and CI) exposes `navigator.gpu` but hands out no adapter, so the WebGPU branch runs against a recording stand-in installed with `addInitScript` (fake adapter, device and `webgpu` context counting `requestAdapter`, `writeTexture` and `submit`). It proves which backend the editor picks, that frames reach it and what the status bar names — not GPU output, which no browser in the test environment can produce.
- AC-28: "drawn on a layer above the sprite" is checked by hiding the overlay canvas for one screenshot (the base canvas must then equal the empty sprite) while the full stack differs from it; "never part of the document or an exported frame" by File ▸ Export ▸ PNG (current frame) bytes equal to the export taken before the overlay went live.
- AC-25 – AC-28: the five e2e tests were green at once (the behavior exists). Temporary breaks in `ui/src/modes/pixel/render.ts`, each followed by `pnpm build` and a run, turned them red, then were restored: dropping `if (gpu) return gpu` (AC-25 WebGPU test: no `webgpu` label) together with ignoring `forceCanvas2d` (AC-26: `adapterRequests` 1, expected 0) and painting overlays onto the base canvas (both AC-28 tests: base screenshot differs from the empty sprite); separately, not setting `needsRecompose` on dirty events (AC-27: the canvas stayed blank after the stroke).
- `gotoStart` / `createDocument` in `ui/e2e/helpers.ts` take an optional start `url`, so a spec can open the app with a query (`./?renderer=canvas2d`).
- AC-08, AC-10, AC-15, AC-16, AC-17, AC-22 (acceptance rework): the eight added tests were green at once (the behavior exists). One set of temporary breaks turned each of them red, then was restored: drawing `Only(ids)` layers in list order (AC-08: green-over-red gave red); dropping both `sa == 0` guards and using the blend term in place of the backdrop in the `(1 − αs)·αb` term (AC-10: Multiply over `[40, 160, 220, 255]` gave `[31, 63, 43, 255]`); undoing `rotate_90` before the flips, i.e. flipping first (AC-15: rotate_90 + flip_x gave D B / C A instead of A C / B D); tinting by `(c + 3·t) / 4` (AC-16: `[216, 37, 50]` vs `[177, 75, 100]`, `[25, 37, 241]` vs `[50, 75, 227]`); wrapping onion frame indices modulo the frame count (AC-17: both no-wrap tests showed a pixel instead of transparent); checking only `overlays.grid` (AC-22: `slices` / `selection_marquee` alone were accepted).
