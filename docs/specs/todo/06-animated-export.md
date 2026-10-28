---
id: todo-animated-export
title: Animated export (APNG, GIF) in Pixel mode
status: todo
size: L (three M sessions; GIF needs dependency approval)
mode: Pixel
depends_on: []
created: 2026-10-28
---

# Animated export in Pixel mode

## Goal

File ▸ Export ▸ Animation… writes the animation as **APNG** or **GIF**. You can choose:
- all frames or one tag;
- the tag's direction (forward, reverse, ping-pong);
- an integer scale (×1–×10);
- the loop count.

Each frame keeps its own duration.

## Why

Aseprite, Piskel, Pixelorama and Resprite all export animated GIFs, and that's how pixel art is shared (chat, forums, itch.io). Pincel exports only a single PNG frame or a sprite sheet. §7.3 has no animated format.

## Current state (read these first)

**UI.** `ui/src/modes/pixel/file-ops.ts`
- `exportPng()` (≈L297) calls `ed.doc.exportPng(ed.currentFrame)`.
- `exportSheet()` (≈L321) calls `exportAtlas(columns, tag, …)`.
- Writes go through `saveExport`.
- `ui/src/modes/pixel/components/SpriteSheetDialog.svelte`: `SpriteSheetOptions { columns, tag, data }`. Its **tag `<select>` (≈L72–78) is the pattern to reuse**.

**Core.** `crates/pincel-core/src/codec/png.rs`
- `export_frame_png` (≈L211): `compose(ComposeRequest::full(frame, …))`, then `encode_rgba8`.
- `export_atlas_png` (≈L246) with `AtlasOptions::with_tag` (≈L104).
- `select_frames(sprite, tag)` (≈L344) returns `from..=to` forward and **ignores the tag's direction**.

**Model.**
- `Frame.duration_ms: u16` (`document/frame.rs` ≈L17).
- `Tag { from, to, direction: TagDirection { Forward, Reverse, Pingpong, PingpongReverse }, repeats: u16 }` (`document/tag.rs` ≈L8–26).
- The **direction stepping already exists in TS**, for playback: `ui/src/modes/pixel/playback.ts` (≈L6–43). Port it to Rust; don't call into TS.

**wasm.** `crates/pincel-wasm/src/lib.rs`: `exportPng(frame)` (≈L167), `exportAtlas` (≈L190).

**Dependencies.**
- `pincel-core` already depends on `png = 0.18`, which can **write APNG** (`acTL` / `fcTL` through `Encoder::set_animated` and `set_frame_delay`). Check the 0.18 API names when you implement.
- `gif` 0.14 is in `Cargo.lock`, but only as a dependency of `image` (pulled in by `pincel-image-core`). **Adding it to `pincel-core` is a new direct dependency.** Stop and ask (CLAUDE.md §9 / §12), with a Decision Log entry. It adds no new crate to the lockfile.

## Design

**Frame sequence.** `animation_frames(sprite, tag: Option<&str>) -> Vec<FrameIndex>` in `crates/pincel-core/src/codec/`.
- It expands the direction: Reverse, Pingpong (without repeating the end frames, matching Aseprite and `playback.ts`), and PingpongReverse.
- Unit-test it against the TS behavior.
- `select_frames` for sheets keeps its forward order; that's existing behavior, so don't change it.

**Scaling.** Nearest-neighbour integer upscale, applied after compose. Reuse `Raster::resample(w, h, Nearest)` in `command/transform_support.rs` (≈L214), or a plain block copy.

**APNG.**
- All frames at full canvas size with `dispose_op = None`, `blend_op = Source`. Simple and correct; optimizing to sub-rects is a later concern.
- Delay = `duration_ms` / 1000.
- Loop count: 0 = forever. Default is the tag's `repeats` if set, otherwise forever.

**GIF.**
- 8-bit palette per frame. Sprites with 256 colors or fewer (the usual case, and every indexed sprite) are written exactly.
- More colors: quantize with the `gif` crate's `Frame::from_rgba_speed` (NeuQuant) and **warn** in the dialog ("more than 256 colors, GIF will be approximated").
- Transparency is 1-bit. Alpha below 128 is transparent; show a hint when semi-transparent pixels exist.
- Delay is in 1/100 s. Round, with a minimum of 2 (20 ms), because browsers clamp anything faster.
- Disposal: `Background`, so transparent frames don't smear.

**Animated WebP** is out of scope for now. The workspace's `image-webp` encodes still lossless images only. Record that in the Decision Log.

**UI.**
- New `AnimationExportDialog.svelte` in `ui/src/modes/pixel/components/`:
  - format (APNG / GIF), tag (reuse the select), scale, loop (forever or N);
  - a live estimate of frame count and output size (W×scale × H×scale).
- Menu: File ▸ Export ▸ Animation…
- i18n in `en.ts` and `de.ts`.
- File name `<stem>.png` / `<stem>.gif` (APNG keeps `.png` so that every viewer opens it).

## Tasks (one M session each)

1. **`docs(spec)` + core frame sequence + APNG.**
   - §7 gets an "Animated export" subsection plus a Decision Log entry: APNG first, GIF after approval, WebP deferred.
   - Core: `animation_frames`, `export_apng(sprite, opts) -> Vec<u8>`.
   - Tests:
     - `animation_frames_pingpong_skips_repeated_ends`
     - `animation_frames_reverse_tag`
     - `apng_has_actl_with_frame_count_and_loops`
     - `apng_frame_delays_match_durations`
     - `apng_scale_2_doubles_dimensions`
     - a decode round-trip: the decoded first frame equals `compose` of frame 0.
2. **GIF**, after the user approves `gif` as a direct dependency.
   - `export_gif(sprite, opts)`.
   - Tests:
     - `gif_under_256_colors_is_lossless`
     - `gif_alpha_threshold`
     - `gif_delay_rounds_to_centiseconds_min_2`
     - `gif_over_256_colors_reports_quantized`
3. **wasm and UI.**
   - `exportAnimation(format, tag?, scale, loops) -> Vec<u8>` (an options object or positional, following the existing `exportAtlas` style).
   - The dialog, menu entry and i18n.
   - e2e `ui/e2e/export-animation.spec.ts`: a 3-frame sprite, export APNG, assert the download name and the bytes (the PNG signature plus an `acTL` chunk with `num_frames = 3`; for GIF, the `GIF89a` header plus the NETSCAPE2.0 loop extension). Download pattern from `ui/e2e/export.spec.ts`.

## Out of scope

- Animated WebP, MP4 / WebM video and per-layer export.
- Exporting from Image mode (it has no frames).
- Frame-difference optimization (sub-rect frames). Possible later for file size.
