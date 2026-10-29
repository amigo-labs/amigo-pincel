---
status: implemented
created: 2026-10-28
size: L
mode: Pixel
design: [pincel.md §7.3, pincel.md §7.5, pincel.md §3.3, pincel.md §3.6]
---

# Animated export (APNG, GIF) in Pixel mode

## Context

Pixel art is shared as animated GIFs (chat, forums, itch.io). Aseprite, Piskel, Pixelorama and Resprite all export animated GIFs; Pincel exports only a single PNG frame or a sprite sheet, and §7.3 has no animated format.

File ▸ Export ▸ Animation… writes the animation as **APNG** or **GIF**. The user chooses all frames or one tag, the export follows the tag's direction, applies an integer scale and a loop count, and every frame keeps its own duration.

APNG needs no new dependency: `pincel-core` already depends on `png` 0.18 (Decision Log 2026-08-20), which can write APNG. GIF uses `gif` 0.14 as a new direct dependency of `pincel-core`, approved 2026-10-29 (see Decisions).

## Scope

### In scope

- File ▸ Export ▸ Animation… with a dialog: format (APNG / GIF), frames (all frames or one tag), scale ×1–×10, loop count (forever or N)
- A live estimate in the dialog: number of exported frames and output size in pixels
- Frame order that follows the chosen tag's direction (Forward, Reverse, Pingpong, PingpongReverse)
- Per-frame durations carried into the file
- Nearest-neighbour integer upscale
- APNG writer
- GIF writer, with a warning when the colours have to be approximated and a hint when semi-transparent pixels lose their partial alpha
- en + de strings

### Out of scope

- Animated WebP — because the workspace's `image-webp` encodes still lossless images only.
- MP4 / WebM video — because the workspace has no video encoder and adding one is a new dependency (AGENTS.md §9).
- Per-layer export — because it writes one file per layer, which needs a multi-file save flow; follow-up together with split-layer sheet export.
- Export from Image mode — because Image-mode documents have no frames.
- Frame-difference optimization (sub-rect frames) — because full-canvas frames are simple and correct; file size can be optimized later.
- Changing the sprite-sheet frame order — because `select_frames` for sheets keeps its forward order; that is existing behavior.

## Acceptance criteria

- [x] AC-01 (event): When the user chooses File ▸ Export ▸ Animation… on a Pixel document, then a dialog opens with a format choice (APNG, GIF), a frames choice (all frames or one of the sprite's tags), a scale choice from ×1 to ×10 and a loop choice (forever or a number N ≥ 1).
- [x] AC-02 (state): While the animation export dialog is open, it shows the number of frames the export will contain and the output size `(width × scale) × (height × scale)`, and both update when the frames choice or the scale changes.
- [x] AC-03 (state): While "all frames" is chosen, the export contains every frame of the sprite once, in forward order.
- [x] AC-04 (option): Where a tag is chosen, the export contains the tag's frames in the order of its direction: Forward `from…to`; Reverse `to…from`; Pingpong `from…to` then back down to `from + 1` (a tag over frames 0–2 gives 0, 1, 2, 1); PingpongReverse `to…from` then back up to `to − 1` (frames 0–2 give 2, 1, 0, 1). A tag whose direction byte Pincel doesn't know is exported as Forward.
- [x] AC-05 (option): Where the scale is N, the exported image is `N × width` by `N × height` pixels and every source pixel becomes an N×N block of exactly its colour.
- [x] AC-06 (event): When the user exports as APNG, then the editor downloads `<stem>.png` (`<stem>` is the document's file name without `.aseprite` / `.ase`, as for the existing exports), a PNG file with an `acTL` chunk whose frame count equals the number of exported frames.
- [x] AC-07 (always): Decoding an exported APNG yields, for every frame, exactly the composed sprite frame at that position of the sequence (upscaled by the scale), independent of the frames before it.
- [x] AC-08 (always): Each APNG frame's delay equals the source frame's duration in milliseconds.
- [x] AC-09 (option): Where the loop choice is "forever", the APNG's play count is 0 (infinite); where it is N, the APNG's play count is N.
- [x] AC-10 (always): The loop choice defaults to the chosen tag's repeat count when that tag has a non-zero repeat count, and to "forever" otherwise (all frames, or a tag with repeat count 0).
- [x] AC-11 (event): When the user exports as GIF, then the editor downloads `<stem>.gif`, a file starting with `GIF89a` that carries a NETSCAPE2.0 loop extension with loop count N − 1 (0 for forever), and no loop extension when N = 1.
- [x] AC-12 (always): In an exported GIF, a pixel whose composed alpha is below 128 is transparent and a pixel with alpha 128 or more is opaque.
- [x] AC-13 (state): While every exported frame fits a 256-entry colour table (its distinct opaque colours, plus one entry when the frame has transparent pixels), each decoded GIF frame keeps the exact RGB of every opaque pixel.
- [x] AC-14 (state): While GIF is chosen and at least one exported frame does not fit a 256-entry colour table, the dialog shows a warning that the GIF will be approximated, and the export still writes a GIF.
- [x] AC-15 (state): While GIF is chosen and an exported frame has semi-transparent pixels (alpha between 1 and 254), the dialog shows a hint that GIF transparency is on/off only.
- [x] AC-16 (always): Each GIF frame's delay is the source duration in hundredths of a second, rounded to the nearest integer, and never less than 2.
- [x] AC-17 (always): Decoding an exported GIF yields transparent pixels in frame k wherever composed frame k is transparent, even when an earlier frame was opaque there.
- [x] AC-18 (error): If the export fails (the renderer refuses the sprite, for example an indexed sprite; the sequence has no frames; or a caller of the wasm API passes a scale outside 1–10 or an unknown tag), then the export returns an error instead of bytes, no file is written, and in the editor the status bar reports the failure with the error message.
- [x] AC-19 (error): If the scaled output would be wider or higher than 16384 pixels, then the export fails with an error naming the limit and no file is written.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names.

**UI.** `ui/src/modes/pixel/file-ops.ts`: `exportPng()` (≈L297) calls `ed.doc.exportPng(ed.currentFrame)`; `exportSheet()` (≈L321) calls `exportAtlas(columns, tag, …)`; writes go through `saveExport`; errors land in `ed.status` via `t('status.failed', …)`. `ui/src/modes/pixel/components/SpriteSheetDialog.svelte` (`SpriteSheetOptions { columns, tag, data }`) has the tag `<select>` (≈L70–78) to reuse. The File ▸ Export submenu is built in `ui/src/modes/pixel/menus.ts` (≈L119–123). On macOS the native menu has its own item table in `src-tauri/src/main.rs` (`export_sheet`, ≈L88/L301) if the new entry should appear there too.

**Core.** `crates/pincel-core/src/codec/png.rs`: `export_frame_png` (≈L211, `compose(ComposeRequest::full(frame, …))` then `encode_rgba8` ≈L377); `export_atlas_png` (≈L246) with `AtlasOptions::with_tag` (≈L104); `select_frames(sprite, tag)` (≈L344) returns `from..=to` forward and ignores the tag direction. `ExportError` (≈L25) wraps `RenderError`; `compose()` refuses non-RGBA sprites (`render/compose.rs` ≈L42), so indexed sprites fail export today, and the legacy plan's remark that indexed sprites are written exactly as GIF does not apply until indexed compose lands.

**Model.** `Frame.duration_ms: u16` (`document/frame.rs` ≈L17). `Tag { from, to, direction: TagDirection { Forward, Reverse, Pingpong, PingpongReverse }, repeats: u16 (0 = infinite), raw_direction }` (`document/tag.rs`).

**Direction stepping** exists in TS for playback: `ui/src/modes/pixel/playback.ts` (`startCursor` / `advance`). Port it to Rust (proposed `animation_frames(sprite, tag: Option<&str>) -> Vec<FrameIndex>` in `codec/`); don't call into TS. Unit-test it against the TS behavior.

**Scaling.** Nearest-neighbour integer upscale after compose: `Raster::resample(w, h, Nearest)` in `crates/pincel-core/src/command/transform_support.rs` (≈L214) or a plain block copy. The scaled-export spec needs the same upscale; share one function.

**APNG.** `png` 0.18 writes APNG (`acTL` / `fcTL` through `Encoder::set_animated` and `set_frame_delay`; check the 0.18 names). Proposed: every frame at full canvas size, `dispose_op = None`, `blend_op = Source`; delay `duration_ms / 1000`.

**GIF.** `gif = "0.14"` (approved 2026-10-29; MIT OR Apache-2.0; `default-features = false` plus `std`, encoder side only) is already in `Cargo.lock` as a dependency of `image`, so adding it to `pincel-core` adds no new crate to the lockfile. Loop count: N plays in total → NETSCAPE2.0 loop count N − 1; forever → 0; N = 1 → no NETSCAPE block. Proposed: one 8-bit palette per frame; quantize frames that don't fit with `Frame::from_rgba_speed` (NeuQuant); disposal `Background`; delay in 1/100 s (browsers clamp faster delays).

**Memory.** Frames are encoded one at a time (compose → scale → encode → drop); never hold all scaled frames. Memory is bounded by one scaled frame plus the output bytes. The 16384 px per-edge cap is shared with `scaled-export`.

**wasm.** `crates/pincel-wasm/src/lib.rs`: `exportPng(frame)` (≈L167), `exportAtlas(columns, tag, image_name)` (≈L190). Proposed `exportAnimation(format, tag?, scale, loops)` following the `exportAtlas` style; the npm API is public (§17.5).

**e2e.** Proposed `ui/e2e/export-animation.spec.ts`: a 3-frame sprite, export APNG, assert the download name, the PNG signature and an `acTL` chunk with `num_frames = 3`; for GIF the `GIF89a` header and the NETSCAPE2.0 extension. Download pattern: `ui/e2e/export.spec.ts` (forces the download fallback by deleting `showSaveFilePicker`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Tag with `from == to` | One frame, in every direction |
| Pingpong tag over two frames (0–1) | Sequence 0, 1 — the end frames are not repeated |
| Single-frame sprite, all frames | A valid animation with one frame |
| Frame duration below 20 ms, GIF | Delay written as 2 (20 ms) |
| Tag with an unknown direction byte (`raw_direction`) | Exported as Forward, as playback does (§7.1) |
| Semi-transparent pixels, GIF | Alpha < 128 becomes transparent, ≥ 128 opaque; the dialog shows the hint |
| A frame with 256 distinct opaque colours plus transparent pixels, GIF | Does not fit a 256-entry table: quantized, the dialog warns |
| Indexed sprite | Export fails with the renderer's message in the status bar; no file |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `the dialog offers format, frames, scale and loop and estimates the output` (ui/e2e/export-animation.spec.ts) | verified | e7cee29 |
| AC-02 | `the dialog offers format, frames, scale and loop and estimates the output` (ui/e2e/export-animation.spec.ts); `animation_info_counts_the_sequence_frames` (crates/pincel-core/src/codec/animation/mod.rs) | verified | e7cee29 |
| AC-03 | `animation_frames_all_frames_are_forward_once` (crates/pincel-core/src/codec/animation/sequence.rs) | verified | 74d5722 |
| AC-04 | `animation_frames_forward_tag_runs_from_to`, `animation_frames_reverse_tag_runs_to_from`, `animation_frames_pingpong_returns_without_repeating_ends`, `animation_frames_pingpong_reverse_starts_at_to`, `animation_frames_single_frame_tag_is_one_frame_in_every_direction`, `animation_frames_unknown_direction_byte_plays_forward` (crates/pincel-core/src/codec/animation/sequence.rs); `export_animation_apng_frames_equal_composed_frames_in_sequence_order` (crates/pincel-core/src/codec/animation/apng.rs) | verified | 74d5722 |
| AC-05 | `upscale_nearest_makes_exact_blocks`, `scaled_size_multiplies_both_edges` (crates/pincel-core/src/codec/scale.rs); `export_animation_apng_scale_makes_exact_blocks` (crates/pincel-core/src/codec/animation/apng.rs) | verified | 74d5722 |
| AC-06 | `APNG export downloads <stem>.png with an acTL frame count` (ui/e2e/export-animation.spec.ts); `export_animation_apng_has_png_signature_and_actl_frame_count` (crates/pincel-core/src/codec/animation/apng.rs); `export_animation_apng_carries_actl_with_frame_count` (crates/pincel-wasm/src/animation.rs) | verified | e7cee29 |
| AC-07 | `export_animation_apng_frames_equal_composed_frames_in_sequence_order`, `export_animation_apng_single_frame_is_valid_animation` (crates/pincel-core/src/codec/animation/apng.rs) | verified | 74d5722 |
| AC-08 | `export_animation_apng_delay_is_duration_in_ms` (crates/pincel-core/src/codec/animation/apng.rs) | verified | 74d5722 |
| AC-09 | `export_animation_apng_play_count_follows_loops` (crates/pincel-core/src/codec/animation/apng.rs) | verified | 74d5722 |
| AC-10 | `the loop choice defaults to the tag repeat count`, `the dialog offers format, frames, scale and loop and estimates the output` (ui/e2e/export-animation.spec.ts); `animation_info_default_loops_follow_the_tag_repeats` (crates/pincel-core/src/codec/animation/mod.rs); `animation_info_reports_count_loops_and_notices` (crates/pincel-wasm/src/animation.rs) | verified | e7cee29 |
| AC-11 | `GIF export downloads <stem>.gif with the NETSCAPE loop count` (ui/e2e/export-animation.spec.ts); `export_animation_gif_starts_with_gif89a`, `export_animation_gif_loop_extension_counts_extra_plays` (crates/pincel-core/src/codec/animation/gif.rs) | verified | e7cee29 |
| AC-12 | `export_animation_gif_alpha_threshold_is_128` (crates/pincel-core/src/codec/animation/gif.rs) | verified | ad1cd36 |
| AC-13 | `export_animation_gif_fitting_frames_keep_exact_rgb`, `fits_table_counts_the_transparent_entry` (crates/pincel-core/src/codec/animation/gif.rs) | verified | ad1cd36 |
| AC-14 | `GIF notices warn about approximated colours and on/off transparency` (ui/e2e/export-animation.spec.ts); `animation_info_flags_frames_over_the_gif_table` (crates/pincel-core/src/codec/animation/mod.rs); `export_animation_gif_overflowing_frame_is_quantized_and_written` (crates/pincel-core/src/codec/animation/gif.rs); `median_cut_overflowing_set_returns_at_most_max_colors` (crates/pincel-core/src/codec/animation/quantize.rs) | verified | e7cee29 |
| AC-15 | `GIF notices warn about approximated colours and on/off transparency` (ui/e2e/export-animation.spec.ts); `animation_info_flags_semi_transparent_pixels` (crates/pincel-core/src/codec/animation/mod.rs); `has_partial_alpha_detects_alpha_between_1_and_254` (crates/pincel-core/src/codec/animation/gif.rs) | verified | e7cee29 |
| AC-16 | `delay_cs_rounds_to_hundredths_with_minimum_two`, `export_animation_gif_frame_delays_follow_durations` (crates/pincel-core/src/codec/animation/gif.rs) | verified | ad1cd36 |
| AC-17 | `export_animation_gif_pixel_turning_transparent_shows_transparent`, `export_animation_gif_frames_follow_sequence_and_scale` (crates/pincel-core/src/codec/animation/gif.rs) | verified | ad1cd36 |
| AC-18 | `an output over 16384 px fails in the status bar without a download` (ui/e2e/export-animation.spec.ts); `export_animation_indexed_sprite_returns_render_error`, `export_animation_frameless_sprite_returns_no_frames`, `export_animation_scale_out_of_range_returns_err`, `export_animation_unknown_tag_returns_err` (crates/pincel-core/src/codec/animation/mod.rs); `export_animation_bad_arguments_return_err` (crates/pincel-wasm/src/animation.rs) | verified | 74d3e7f |
| AC-19 | `an output over 16384 px fails in the status bar without a download` (ui/e2e/export-animation.spec.ts); `export_animation_over_edge_limit_returns_err_naming_limit` (crates/pincel-core/src/codec/animation/mod.rs); `scaled_size_above_limit_returns_err_naming_limit`, `scaled_size_exactly_at_limit_is_accepted` (crates/pincel-core/src/codec/scale.rs); `export_animation_over_edge_limit_names_the_limit` (crates/pincel-wasm/src/animation.rs) | verified | e7cee29 |

## Summary

> Completed: 2026-10-29

- File ▸ Export ▸ Animation… (web menu and native macOS menu) opens a dialog with format (APNG / GIF), frames (all or one tag), scale ×1–×10 and loop (forever / N plays), a live frame-count and size estimate and the two GIF notices.
- `pincel-core` `export_animation` / `animation_info` / `animation_frames`: tag directions including pingpong without repeated ends, the nearest-neighbour upscale in crate-private `codec/scale.rs`, the 16384 px per-edge cap, frames streamed one at a time.
- APNG through `png` 0.18 (full-canvas frames, dispose None, blend Source, delay in ms, play count); GIF through `gif` 0.14 (one palette per frame, exact when it fits 256 entries, in-house median cut otherwise, alpha threshold 128, delay ≥ 2 cs, disposal to background, NETSCAPE2.0 loop count N − 1).
- `pincel-wasm` `exportAnimation(format, tag?, scale, loops)` and `animationInfo(tag?)`; failures reach the status bar without a download.

## Decisions

- 2026-10-28 (legacy plan): APNG keeps the `.png` extension, so every image viewer opens it — APNG is a backwards-compatible PNG.
- 2026-10-28 (legacy plan): Pingpong does not repeat its end frames — matches Aseprite and Pincel's own playback (`playback.ts`).
- 2026-10-28 (legacy plan): The loop default comes from the tag's repeat count — the tag already records how often it should play.
- 2026-10-28 (legacy plan): GIF alpha threshold 128 and minimum delay 2 cs — GIF transparency is 1-bit, and browsers clamp faster delays.
- 2026-10-28 (legacy plan): APNG ships before GIF — it needs no new dependency.
- 2026-10-29: `gif = "0.14"` becomes a direct dependency of `pincel-core` (MIT OR Apache-2.0, `default-features = false` plus `std`, encoder only); AC-11 to AC-17 are unblocked — the crate is already compiled into the workspace through `image`, builds for wasm32, and an in-house LZW / NETSCAPE writer would be a second codec to maintain (owner accepted the recommendation).
- 2026-10-29: The Decision Log entry for animated export (APNG through `png` 0.18, GIF through `gif` 0.14, one 8-bit palette per frame, alpha threshold 128, delay ≥ 2 cs, animated WebP deferred) and a `pincel.md` §7.5 "Animated export" paragraph are added — AGENTS.md §2.1 requires the entry before code, and §7.3 has no animated format yet (owner accepted the recommendation).
- 2026-10-29: A GIF loop count N means N plays in total: NETSCAPE2.0 loop count N − 1, no NETSCAPE block for N = 1, 0 for forever — one meaning across APNG, GIF and the tag's `repeats`, avoiding browser differences around the repeat field (owner accepted the recommendation).
- 2026-10-29: The 16384 px per-edge cap of `scaled-export` applies with the same error (AC-19); there is no frames × size cap, frames are streamed one at a time instead — one limit shared with `scaled-export`, and streaming bounds memory without a second, arbitrary limit (owner accepted the recommendation).
- 2026-10-29: Per-layer export stays out of scope — it writes one file per layer, which the single-download export flow and dialog don't handle; it belongs with a future split-layers option shared with sprite-sheet export (owner accepted the recommendation).
- 2026-10-29 (spec-build): GIF frames that overflow the 256-entry table are reduced by a small in-house median cut (`codec/animation/quantize.rs`) instead of `gif`'s NeuQuant `Frame::from_rgba_speed` — that function needs `gif`'s `color_quant` feature, beyond the approved `default-features = false` plus `std`, and it treats every non-zero alpha as opaque, contradicting the 128 threshold.
- 2026-10-29 (spec-build): New public `pincel-core` API: `AnimationFormat`, `AnimationOptions`, `AnimationInfo`, `animation_frames`, `animation_info`, `export_animation` and the `ExportError` variants `InvalidScale`, `ScaledTooLarge`, `GifEncode`. `animation_info` gives the dialog its frame count, default loop count and the two GIF notices without encoding — the dialog cannot know the colour count otherwise.
- 2026-10-29 (spec-build): The dialog's loop choice is a "Forever" / "N times" radio pair with a play-count field (1–65535); choosing another frames entry resets it to that entry's default (AC-10). The estimate reads "Frames: n · size: w×h px", which avoids a plural form the i18n layer doesn't have. The UI is test-after (AGENTS.md §2.3); its e2e builds a minimal `.aseprite` in the test to get a tag with a repeat count, which the editor cannot set yet.
- 2026-10-29 (spec-build): The macOS native menu gets File ▸ Export Animation… (`menu:exportAnimation`) next to the sheet export, without an accelerator — the spec names no shortcut.
- 2026-10-29 (spec-build): The shared upscale lives in `codec/scale.rs` (`scaled_size`, `upscale_nearest`, crate-private) for `scaled-export` to reuse; a plain block copy rather than `Raster::resample`, which lives in the command layer.
