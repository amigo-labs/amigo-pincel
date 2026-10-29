---
status: todo
created: 2026-10-29
size: S
mode: Image
design: [pincel.md §5.4, pincel.md §9.2, pincel.md §15 (2026-09-06 Two cores, one shell), pincel.md §15 (2026-10-12 Pixel layers get a lock and drag-reorder), pincel.md §15 (2026-10-27 Image mode gains Gradient, Recolor and Smudge)]
---

# Image-mode editing: layers, tools, compositing, undo

## Context

Image mode is the paint.net-style editor (§1.1) behind PNG / JPEG / WebP / BMP / GIF / TIFF / OpenRaster documents. It runs on its own core, `pincel-image-core`, inherited from the archived sister project `amigo-fineliner` (Decision Log 2026-09-06 "Two cores, one shell"): a dense, canvas-sized layer stack blended in linear light, with a JSON `CommandSpec` wasm ABI. This spec records what that editor does today — the layer stack, the tool set of §5.4, the selection, the Image / Layer / Select menus, compositing and undo — so it can be accepted and kept from regressing.

Already decided: the tool set and keys (§5.4, Decision Log 2026-10-27), the layer lock refusing pixel edits (2026-10-12), pen pressure and tilt (2026-10-12, owned by the `pen-input` spec).

## Scope

### In scope

- The layer stack: add, delete, duplicate, rename, reorder, opacity, blend mode, visibility, lock, Merge Down, Merge Visible, Flatten
- Compositing of the layer stack (visibility, opacity, twelve blend modes, linear-light blending)
- The Image-mode tools of §5.4: Move (three variants), Pan, Zoom, Marquee, Lasso, Magic Wand, Brush, Eraser, Recolor, Smudge, Paint Bucket, Gradient, Eyedropper, Line, Shapes, Text
- The selection: shapes, combine modes, feather, Select menu, Delete, selection bounding every pixel tool
- Image menu (canvas flip / rotate / resize / scale / crop) and Layer menu transforms (flip / rotate)
- Undo / redo of every document edit

### Out of scope

- Paste as a new layer — because the `copy-cut-paste` spec owns clipboard editing in both modes, including today's Image-mode paste
- Pen pressure and tilt — because the `pen-input` spec covers them
- Effects and adjustments — because the `effects-adjustments` spec covers them
- Opening, saving and exporting files, image-size limits — because the `image-files` spec covers them
- Layer groups — because the planned `image-layer-groups` spec adds them; Image mode has no groups today
- The shared keymap's non-tool actions, the tool rail layout and the touch modifier toggles — because the `app-shell`, `shortcut-overview` and `tablet-gestures` specs cover them
- Autosave of Image documents — because the `autosave-recovery` spec covers it

## Acceptance criteria

- [ ] AC-01 (always): The composite shows the visible layers bottom to top: a hidden layer contributes nothing, an opaque Normal layer hides everything below it, and a layer's opacity scales its alpha before blending in linear light — a white layer at 50 % opacity over opaque black composites to a gray between 186 and 190, not 128.
- [ ] AC-02 (always): Every layer has one of twelve blend modes — Normal, Multiply, Screen, Overlay, Darken, Lighten, Color Dodge, Color Burn, Hard Light, Soft Light, Difference, Exclusion — and composites with the W3C compositing-and-blending formula of that mode (e.g. Multiply with white keeps the backdrop, Screen with black keeps the backdrop, Difference is the absolute difference).
- [ ] AC-03 (always): Compositing the same layer stack twice yields identical pixels, and a layer composited source-over never lowers the alpha of the pixels below it.
- [ ] AC-04 (event): When the user adds a layer, then a transparent layer appears directly above the active layer and becomes the active layer; one Undo removes it.
- [ ] AC-05 (error): If the user tries to delete the only layer, then the document keeps that layer.
- [ ] AC-06 (event): When the user renames a layer (Layer ▸ Rename…), drags it to a new position by its grip, duplicates it, or changes its opacity, blend mode, visibility or lock, then the change shows in the layers panel and each is one undo step that restores the previous state; opacity is clamped to 0–100 %, and consecutive opacity changes to the same layer form one undo step.
- [ ] AC-07 (state): While a layer is locked, the pixel-writing tools refuse to change it and a notice "Layer “<name>” is locked" appears; the layer can still be renamed, hidden, reordered and unlocked, and after unlocking it accepts paint again.
- [ ] AC-08 (event): When the user chooses Merge Down, Merge Visible or Flatten, then: Merge Down combines the active layer into the layer below, which keeps its own blend mode and opacity; Merge Visible merges the visible layers into one without changing the composite and keeps hidden layers; Flatten leaves one fully opaque layer composited over white. Each is one undo step that restores the stack.
- [ ] AC-09 (event): When the user drags with the Brush, then a connected stroke is painted on the active layer in the foreground color with the chosen size, opacity, hardness and tip (a soft tip's edge is lighter than its center; 50 % opacity over transparency yields half alpha), and the whole drag is one undo step.
- [ ] AC-10 (event): When the user paints with the Brush or the Paint Bucket using the right mouse button, then the background color is used; when the user clicks with the Eyedropper using the right mouse button, then the sampled color becomes the background color.
- [ ] AC-11 (state): While a selection exists, the Brush, Paint Bucket, Gradient, Shapes, Line and Text change only pixels inside it, and Delete reduces a partially selected pixel's alpha in proportion to its selection coverage.
- [ ] AC-12 (event): When the user erases, then in "to transparent" mode the erased pixels lose alpha by the eraser's opacity (50 % opacity halves an opaque pixel's alpha), and in "to background" mode they take the background color; the stroke is one undo step.
- [ ] AC-13 (event): When the user clicks with the Paint Bucket, then the pixels matching the clicked color within the tolerance are filled — only the connected region when Contiguous is on, every matching pixel when off — sampling either the active layer or the composite of all layers; one undo step.
- [ ] AC-14 (event): When the user drags with the Gradient tool, then the active layer gets a ramp from the foreground (at the drag start) to the background color (at the drag end): Linear keeps the end color past the drag end, Reflected mirrors through the start, Radial is symmetric around the start; in the Transparency channel the existing colors stay and only their alpha ramps. One undo step.
- [ ] AC-15 (event): When the user drags with Recolor, then only pixels within the tolerance of the background color under the tip are repainted with the foreground color, other pixels stay unchanged, and a stroke delivered in several segments gives the same result as one delivered at once.
- [ ] AC-16 (event): When the user drags with Smudge, then the color under the tip is carried along the stroke — further with higher strength; strength 0 and a flat single-color area change nothing; a stroke delivered in segments gives the same result as one delivered at once.
- [ ] AC-17 (event): When the user drags with Line, Rectangle, Rounded Rectangle, Ellipse or Polygon, then the shape is drawn on the active layer as outline (foreground color), fill (background color) or both, with the stroke width, dash pattern (solid / dashed / dotted) and anti-alias setting; a line is stroked whatever the fill mode; one undo step.
- [ ] AC-18 (event): When the user holds Shift while dragging a shape or a line, then an area shape's box becomes a square and a line snaps to the nearest multiple of 45°.
- [ ] AC-19 (event): When the user commits text with the Text tool, then it is rasterised onto the active layer in the foreground color at the clicked point, a larger size covers more pixels, and the commit is one undo step; empty text changes nothing.
- [ ] AC-20 (event): When the user clicks with the Eyedropper, then the foreground color becomes the average of a 1, 3, 5, 11 or 31 px square around the point, taken from the active layer or from the composite; a click off the canvas changes nothing.
- [ ] AC-21 (event): When the user drags with a Move variant, then Move Layer shifts the active layer's pixels (pixels pushed past the canvas edge are lost, the vacated area becomes transparent); Move Selected Pixels lifts only the selected pixels, leaves transparency behind, composites them over what lies at the target and takes the outline along, so a second drag carries them further; Move Selection moves only the outline. Each drag is one undo step.
- [ ] AC-22 (event): When the user makes a selection with Rectangle Select, Ellipse Select, Lasso, Polygonal Lasso or Magic Wand, then the selection covers that shape (the wand: the pixels within the tolerance of the clicked color, contiguous or global, from the layer or the composite); a feather radius gives partial coverage at the edges and keeps the deep interior fully selected; Add, Subtract and Intersect combine the new shape with the existing selection as union, difference and overlap.
- [ ] AC-23 (event): When the user chooses Select All, Deselect, Invert Selection, or Expand / Contract / Feather (which ask for a radius in px), then the selection changes accordingly and each change is one undo step.
- [ ] AC-24 (event): When the user presses Delete, then the selected pixels of the active layer become transparent, or the whole layer when nothing is selected; one undo step.
- [ ] AC-25 (event): When the user flips or rotates the canvas, resizes the canvas with an anchor, scales the image, crops to the selection, or flips / rotates the active layer, then: canvas flips and rotations transform every layer and a 90° rotation swaps width and height; Resize Canvas keeps the existing pixels at the chosen anchor and crops what falls outside; Scale Image resamples every layer to the new size; Crop shrinks the canvas to the selection's bounding box; layer flips and rotations keep the canvas size. Each is one undo step; Resize and Rotate drop the selection and Undo restores it.
- [ ] AC-26 (always): Undo reverts the last document edit and Redo re-applies it; a new edit after an Undo discards the redo steps; the history has a fixed capacity and drops the oldest step once full.
- [ ] AC-27 (event): When the user selects the Pan tool (H) or the Zoom tool (Z), then the rail shows it active; a click with Zoom steps the view in, and the zoom-out and 1:1 buttons keep working while Zoom is the active tool.
- [ ] AC-28 (event): When the user presses a tool key, then the tool of §5.4 becomes active — V Move (repeated presses cycle Move Layer, Move Selected Pixels, Move Selection), H Pan, Z Zoom, M Marquee, Q Lasso, W Magic Wand, B Brush, E Eraser, R Recolor, S Smudge, G Paint Bucket, N Gradient, I Eyedropper, L Line, U Shapes (repeated presses cycle Rectangle, Rounded Rectangle, Ellipse, Polygon), T Text.

## Technical notes

Hints, verified 2026-10-29; paths and line numbers drift.

**Document model** (`crates/pincel-image-core/src/document/`)

- `Document { id, title, canvas: CanvasSize, layers: Vec<Layer> /* bottom (0) → top */, active_layer, selection: Option<SelectionMask> /* None = everything */, metadata }`; `MAX_LAYERS = 999`; at least one layer always (`DocumentError::LastLayer`).
- `Layer { id: Uuid, name, kind: LayerKind::Raster, pixels: ImageBuffer /* straight-alpha RGBA8, canvas-sized */, opacity: f32 /* 0..=1 */, blend_mode: BlendMode, visible, locked }`.
- `BlendMode`: `Normal, Multiply, Screen, Overlay, Darken, Lighten, ColorDodge, ColorBurn, HardLight, SoftLight, Difference, Exclusion` (no HSL modes, unlike Pixel mode).
- `SelectionMask`: one coverage byte per canvas pixel; `apply_mode(prior, shape, SelectionMode)` combines (Subtract without a prior selection starts from everything).

**Compositing** (`render/mod.rs`): `compose(&[Layer]) -> ImageBuffer` — per pixel, sRGB → linear, W3C blended source `(1-da)·Cs + da·B(Cb,Cs)`, source-over, back to sRGB; skips hidden layers and `opacity <= 0`. `compose_region(layers, Rect)` returns exactly the matching sub-rect (the viewport renderer). `compose_over(layers, Color::WHITE)` is Flatten.

**Undo** (`command/mod.rs`): `CommandBus { document, history }`; `DEFAULT_UNDO_CAPACITY = 100`, `UndoStack::with_capacity` clamps to `10..=500`. Brush-type strokes arrive as one command per pointer event and merge by `stroke_id` (`SetPixels::merge_with`); `SetLayerOpacity` merges consecutive edits on one layer. Selection changes are undoable `SetSelection` commands (unlike Pixel mode).

**wasm** (`crates/pincel-image-wasm/src/lib.rs`, handle-based): `create_document`, `apply_command(handle, json)`, `undo`, `redo`, `set_active_layer`, `pick_color(handle, x, y, sample, size)`, `composite`, `composite_region`, `get_document_info` (`{ width, height, layer_count, active_layer, can_undo, can_redo, undo_depth, has_selection, layers: [{ id, name, opacity, blend_mode, visible, locked }] }`), `get_layer_thumbnail` (32×32), `get_selection_bounds`, `get_selection_mask`, `register_font`. `apply_command` refuses a pixel edit on a locked layer with `"layer is locked"`; `CommandSpec::edited_layer` lists the pixel edits.

`CommandSpec` tags (TS mirror generated into `ui/src/modes/image/core/generated/CommandSpec.ts`): `pencil_stroke`, `recolor_stroke`, `smudge_stroke`, `eraser_stroke`, `fill_bucket`, `translate_layer`, `translate_selection`, `translate_selection_mask`, `delete_selection`, `add_layer`, `remove_layer`, `move_layer`, `duplicate_layer`, `rename_layer`, `set_layer_opacity`, `set_layer_blend_mode`, `set_layer_visible`, `set_layer_locked`, `merge_down`, `merge_visible`, `flatten_image`, `select_rectangle`, `select_ellipse`, `select_polygon`, `select_wand`, `select_all`, `deselect`, `invert_selection`, `expand_selection`, `contract_selection`, `feather_selection`, `transform_layer` (`flip_h` / `flip_v` / `rotate_180`), `rotate_layer_90`, `flip_canvas`, `rotate_canvas` (`cw90` / `ccw90` / `rotate_180`), `scale_image`, `resize_canvas` (anchor), `crop_to_selection`, `draw_shape` (`line` / `rectangle` / `rounded_rectangle` / `ellipse` / `polygon`), `draw_gradient` (kind `linear` / `reflected` / `radial` / `conical` / `diamond`, channel `color` / `transparency`), `draw_text`. A command that misses the canvas, has a zero offset, an empty selection or a bad layer index is a silent no-op (no undo step).

**UI** (`ui/src/modes/image/`): `ImageEditor.svelte` (menus, keys, dialogs), `core/controller.ts` (builds every command; Gradient: `from` = foreground, `to` = background; Recolor: `from` = background, `to` = foreground; shapes: stroke = foreground, fill = background, both at the tool opacity), `tools/pointer.ts` (Shift = add, Alt = subtract, Shift+Alt = intersect for selections; Shift constrains shapes; right button → background color), `components/panels/LayersPanel.svelte` over the shared `lib/components/layers/LayersPanel.svelte`. Text uses the bundled Liberation Sans, registered once (`ensureFont`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A stroke, shape or fill entirely off the canvas | Nothing changes, no undo step |
| Move by a zero offset | Nothing changes, no undo step |
| Move Selected Pixels with no selection | Moves the whole layer |
| Move Selected Pixels with an empty selection | Nothing changes |
| Merge Down on the bottom layer | Refused; the menu item is disabled |
| Merge Visible with every layer hidden | Nothing changes |
| Crop to Selection without a selection | Nothing changes; the menu item is disabled |
| Invert Selection with no selection | Results in an empty selection (no selection means everything is selected) |
| Expand / Contract / Feather with no selection | Nothing changes |
| Magic Wand click off the canvas | Nothing changes |
| Gradient with a zero-length drag | Nothing changes |
| Text with only whitespace, or the font cannot be loaded | Nothing is committed, no error |
| Adding a layer to a document with 999 layers | Refused |
| Expand Selection with a huge radius | Selects everything without overflow |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pincel-image-core: render::tests::hidden_layer_is_skipped`, `render::tests::opaque_top_normal_layer_hides_backdrop`, `render::tests::fifty_percent_opacity_white_over_black_is_mid_gray` | built | |
| AC-02 | `pincel-image-core: render::blend::tests::normal_returns_source`, `render::blend::tests::multiply_with_white_is_identity_with_black_is_black`, `render::blend::tests::screen_with_black_is_identity`, `render::blend::tests::overlay_is_hard_light_with_swapped_operands`, `render::blend::tests::darken_and_lighten_pick_extremes`, `render::blend::tests::color_dodge_and_burn_handle_endpoints`, `render::blend::tests::soft_light_neutral_gray_source_is_near_identity`, `render::blend::tests::difference_is_absolute_delta`, `render::blend::tests::exclusion_half_half_is_half`, `render::tests::multiply_opaque_black_over_white_is_black`, `render::tests::screen_opaque_white_over_black_is_white`; `pincel-image-core (tests/compose_snapshots.rs): compose_multiply_and_screen_stack_snapshot` | built | |
| AC-03 | `pincel-image-core (tests/compose_proptest.rs): compose_is_deterministic`, `composite_alpha_never_below_backdrop`; `pincel-image-core (tests/compose_snapshots.rs): compose_normal_over_gradient_snapshot` | built | |
| AC-04 | `pincel-image-core: document::tests::add_layer_inserts_above_active_and_activates_it`, `command::layers::tests::add_layer_round_trip` | built | |
| AC-05 | `pincel-image-core: document::tests::remove_last_layer_is_blocked` | built | |
| AC-06 | `ui/e2e/layers.spec.ts › image: Layer ▸ Rename… renames the active layer`; `ui/e2e/layers.spec.ts › image: drag the grip to reorder layers`; `pincel-image-core: command::layers::tests::duplicate_layer_inserts_independent_copy_above`, `command::properties::tests::set_opacity_clamps_and_round_trips`, `command::properties::tests::set_opacity_merges_consecutive_same_layer_edits`, `command::properties::tests::set_blend_mode_round_trip`, `command::properties::tests::set_visible_round_trip`, `command::properties::tests::set_locked_round_trip` | built | |
| AC-07 | `ui/e2e/layers.spec.ts › image: a locked layer refuses paint`; `pincel-image-wasm: command_spec_tests::edited_layer_covers_pixel_edits_only` | built | |
| AC-08 | `pincel-image-core: command::merge::tests::merge_down_combines_two_layers_into_one`, `command::merge::tests::merge_down_preserves_lower_layer_blend_mode_and_opacity`, `command::merge::tests::merge_down_round_trip_restores_stack`, `command::merge::tests::merge_visible_preserves_composite_and_keeps_hidden_layers`, `command::merge::tests::flatten_produces_single_opaque_layer`, `command::merge::tests::flatten_round_trip_restores_stack` | built | |
| AC-09 | `pincel-image-core: tools::tests::stroke_paints_a_connected_line`, `tools::tests::soft_round_edge_is_softer_than_center`, `tools::tests::half_opacity_over_transparent_yields_half_alpha`, `command::pixels::tests::merge_with_combines_two_segments_of_same_stroke`; `ui/e2e/smoke.spec.ts › image mode: a brush stroke is undoable and the pixel wasm is not loaded` | built | |
| AC-10 | `ui/e2e/tools.spec.ts › image mode: the right button paints and samples the background color` | built | |
| AC-11 | `pincel-image-core: tools::tests::stroke_outside_selection_writes_nothing`, `tools::tests::stroke_straddling_selection_edge_writes_only_selected_side`, `tools::fill::tests::fill_respects_active_selection`, `tools::gradient::tests::a_selection_bounds_the_gradient`, `tools::shapes::tests::fill_respects_active_selection`, `tools::text::tests::respects_active_selection`, `tools::delete::tests::delete_with_partial_coverage_reduces_alpha_proportionally` | built | |
| AC-12 | `pincel-image-core: tools::eraser::tests::erase_to_transparent_clears_opaque_pixel`, `tools::eraser::tests::erase_partial_opacity_halves_alpha`, `tools::eraser::tests::erase_to_background_paints_background_color`, `tools::eraser::tests::erase_is_undoable_to_original_pixels` | built | |
| AC-13 | `pincel-image-core: tools::fill::tests::fill_with_zero_tolerance_fills_exact_color_region`, `tools::fill::tests::fill_contiguous_stops_at_color_boundary`, `tools::fill::tests::fill_non_contiguous_fills_all_matching_pixels`, `tools::fill::tests::fill_tolerance_includes_near_colors`, `tools::fill::tests::fill_all_layers_sample_matches_composite_not_layer`, `tools::fill::tests::fill_is_undoable_to_original_pixels` | built | |
| AC-14 | `pincel-image-core: tools::gradient::tests::linear_runs_from_the_start_color_to_the_end_color`, `tools::gradient::tests::linear_holds_the_end_color_past_the_drag`, `tools::gradient::tests::reflected_mirrors_through_the_start`, `tools::gradient::tests::radial_is_symmetric_around_the_start`, `tools::gradient::tests::transparency_channel_keeps_the_color_and_clears_the_alpha`, `tools::gradient::tests::gradient_is_undoable`; `ui/e2e/tools.spec.ts › image mode: the Gradient tool paints a ramp` | built | |
| AC-15 | `pincel-image-core: tools::recolor::tests::recolor_replaces_only_the_matching_pixels`, `tools::recolor::tests::recolor_leaves_everything_alone_without_a_match`, `tools::recolor::tests::tolerance_widens_what_counts_as_a_match`, `tools::recolor::tests::a_split_stroke_matches_the_unsplit_one` | built | |
| AC-16 | `pincel-image-core: tools::smudge::tests::smudge_drags_color_across_an_edge`, `tools::smudge::tests::higher_strength_drags_further`, `tools::smudge::tests::strength_zero_changes_nothing`, `tools::smudge::tests::smudge_leaves_a_flat_color_alone`, `tools::smudge::tests::a_split_stroke_matches_the_unsplit_one` | built | |
| AC-17 | `pincel-image-core: tools::shapes::tests::rectangle_fill_covers_interior_and_corners`, `tools::shapes::tests::rectangle_outline_strokes_border_not_center`, `tools::shapes::tests::rounded_rectangle_clips_its_corner`, `tools::shapes::tests::ellipse_fill_covers_center_not_corner`, `tools::shapes::tests::polygon_triangle_fill_covers_centroid`, `tools::shapes::tests::fill_and_outline_paints_both_colors`, `tools::shapes::tests::line_strokes_along_its_path_regardless_of_mode`, `tools::shapes::tests::dashed_outline_paints_fewer_pixels_than_solid`, `tools::shapes::tests::dotted_line_leaves_gaps_along_its_path`, `tools::shapes::tests::fill_is_undoable_to_transparent`; `ui/e2e/tools.spec.ts › image mode: the Line tool draws and undoes` | built | |
| AC-18 | `ui/e2e/tools.spec.ts › image mode: Shift squares a shape and snaps a line to 45°`; `ui/e2e/modifiers.spec.ts › image mode: the latched Shift squares a shape like the Shift key` | built | |
| AC-19 | `pincel-image-core: tools::text::tests::renders_non_empty_text`, `tools::text::tests::larger_size_paints_more_pixels`, `tools::text::tests::empty_text_returns_none`, `tools::text::tests::commit_is_undoable` | built | |
| AC-20 | `pincel-image-core: tools::eyedropper::tests::pick_one_returns_exact_pixel`, `tools::eyedropper::tests::pick_three_by_three_averages_neighborhood`, `tools::eyedropper::tests::pick_composite_sees_lower_layer_through_transparent_top`, `tools::eyedropper::tests::pick_off_canvas_returns_none` | built | |
| AC-21 | `pincel-image-core: tools::move_tool::tests::move_right_shifts_pixels`, `tools::move_tool::tests::move_clears_the_vacated_pixel`, `tools::move_tool::tests::move_drops_pixels_shifted_off_canvas`, `tools::move_tool::tests::move_selection_takes_only_the_selected_pixels`, `tools::move_tool::tests::move_selection_composites_over_what_is_already_there`, `command::selection::move_selection_tests::the_outline_follows_the_pixels`, `command::selection::move_selection_tests::a_second_drag_carries_the_patch_further`, `command::selection::move_selection_tests::undo_restores_both_the_pixels_and_the_outline`, `selection::tests::translated_shifts_coverage`; `ui/e2e/tools.spec.ts › image mode: the move tools drag pixels, the outline, or the layer`; `ui/e2e/tools.spec.ts › image mode: a moved selection can be moved again` | built | |
| AC-22 | `pincel-image-core: selection::tests::rectangle_selects_only_inside`, `selection::tests::ellipse_selects_center_not_corner`, `selection::tests::polygon_fills_triangle_interior`, `selection::wand::tests::magic_wand_contiguous_stops_at_color_boundary`, `selection::wand::tests::magic_wand_global_selects_all_matching_pixels`, `selection::wand::tests::magic_wand_tolerance_includes_near_colors`, `selection::wand::tests::magic_wand_all_layers_samples_composite`, `selection::modifiers::tests::feather_introduces_partial_coverage_at_edges`, `selection::modifiers::tests::feather_keeps_deep_interior_fully_selected`, `selection::tests::combine_add_is_union`, `selection::tests::combine_subtract_removes_overlap`, `selection::tests::combine_intersect_keeps_only_overlap` | built | |
| AC-23 | `ui/e2e/tools.spec.ts › image mode: Select ▸ Expand asks for a radius`; `pincel-image-core: command::selection::tests::set_selection_round_trip_restores_none`, `command::selection::tests::clear_deselects_and_undo_restores`, `selection::tests::invert_swaps_full_and_empty`, `selection::modifiers::tests::expand_grows_selection`, `selection::modifiers::tests::contract_shrinks_selection` | built | |
| AC-24 | `pincel-image-core: tools::delete::tests::delete_clears_inside_selection_only`, `tools::delete::tests::delete_without_selection_clears_whole_layer_and_undoes` | built | |
| AC-25 | `pincel-image-core: command::transform::tests::flip_canvas_transforms_all_layers_and_round_trips`, `command::transform::tests::rotate_canvas_90_swaps_dimensions_and_round_trips`, `command::transform::tests::rotate_canvas_clears_selection_and_restores_on_undo`, `command::resize::tests::resize_larger_preserves_existing_pixels`, `command::resize::tests::resize_smaller_then_revert_restores_pixels`, `command::resize::tests::resize_with_center_anchor_centers_content`, `command::resize::tests::resize_with_bottom_right_anchor_keeps_corner`, `command::resize::tests::resize_clears_selection_and_undo_restores_it`, `command::transform::tests::scale_image_resizes_canvas_and_all_layers_and_round_trips`, `command::transform::tests::crop_to_selection_shrinks_canvas_to_bbox_and_round_trips`, `command::transform::tests::transform_layer_flip_h_round_trips`, `command::transform::tests::transform_layer_180_round_trips`, `command::transform::tests::rotate_layer_90_keeps_canvas_size_and_round_trips` | built | |
| AC-26 | `pincel-image-core: command::tests::apply_then_undo_restores_state`, `command::tests::new_command_after_undo_discards_redo`, `command::tests::undo_on_empty_history_returns_false`, `command::tests::capacity_drops_oldest_command` | built | |
| AC-27 | `ui/e2e/tools.spec.ts › image mode: Pan and Zoom are rail tools`; `ui/e2e/tools.spec.ts › image mode: the zoom buttons still work with the Zoom tool selected` | built | |
| AC-28 | `ui/e2e/tools.spec.ts › image mode: W, B, E, G and T select their tools`, `ui/e2e/tools.spec.ts › image mode: L selects Line and the shapes are its own group`, `ui/e2e/tools.spec.ts › image mode: Pan and Zoom are rail tools`, `ui/e2e/tools.spec.ts › image mode: the move tools drag pixels, the outline, or the layer`, `ui/e2e/tools.spec.ts › image mode: the Gradient tool paints a ramp`, `ui/e2e/tools.spec.ts › image mode: Recolor writes only over the colour it replaces`, `ui/e2e/tools.spec.ts › image mode: Smudge drags along a stroke`; `ui/e2e/shortcuts.spec.ts › image mode: the shared keymap selects tools and cycles variants` | built | |

Not separately evidenced inside built ACs: the Gradient's Conical and Diamond kinds (AC-14 lists only the tested kinds), selection clipping of Eraser, Recolor and Smudge (AC-11 lists only the tested tools), the default undo capacity of 100 (AC-26 says "fixed capacity").

## Decisions

- AC-10, AC-18 and AC-28 were green on their first run: the behavior already existed, the spec records it. To show the new AC-10 and AC-18 tests are not vacuous they were run red against a temporarily mutated `ui/src/modes/image/tools/pointer.ts` (right button forced to the foreground color; Shift constraint, then only the 45° line snap, disabled; Eyedropper right click sampling into the foreground) and failed on the expected assertions; the mutation was reverted.
- The new e2e tests read the document through the Eyedropper (a 1 px composite sample lands in the foreground swatch), because tests stay on the DOM and app code never reads canvas pixels (AGENTS.md §7.4, §9).
- AC-28's G is asserted against the rail label "Fill", the Paint Bucket's name in the Image-mode tool rail.
