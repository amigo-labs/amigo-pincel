---
status: todo
created: 2026-11-17
size: L
mode: shell
design: [pincel.md §3.3, pincel.md §7.5, pincel.md §15]
---

# Animated image import (GIF, WebP, APNG)

## Context

Opening an animated GIF, WebP or APNG today keeps only its first frame: the file goes to Image mode, whose document has no frames (`image-files` AC-02), and Pixel mode's PNG open reads the first APNG frame only. Pixel mode has frames, durations and a timeline, and already exports APNG and GIF (`animated-export.md`), so it is the natural home for an animation; Image mode stays frame-less.

When a document open meets a file with more than one frame, Pincel asks: open as animation in Pixel mode, or the first frame in Image mode. The animation opens with every frame as the viewer sees it (disposal and blending applied) and every frame's own duration.

Owner decisions of 2026-11-17: ask on open (no fixed routing, no frames in Image mode); GIF, WebP and APNG; animated WebP decoded with the pure-Rust `image-webp` crate as a new direct dependency of `pincel-core` (Decision Log 2026-11-17); for an animation beyond Pixel mode's limits the dialog still appears, with "Open as animation" disabled and the reason, so the first frame in Image mode (or Cancel) remains.

## Scope

### In scope

- Detecting that a GIF, WebP or PNG file holds more than one frame, on every document-open path: File ▸ Open in both modes, Open on the start screen, Open Recent, a file dropped "as document" or onto the start screen, and a file the desktop app is asked to open by the OS
- The choice dialog: "Open as animation (Pixel mode)", "Open first frame", Cancel
- Decoding all frames into a Pixel-mode document: canvas = the animation's canvas size, one image layer, one frame per animation frame, durations from the file
- Size and memory limits with a note in the dialog

### Out of scope

- Frames or a timeline in Image mode — because the owner chose to keep Image mode frame-less; that would be a second timeline in a second core
- Writing GIF / WebP back on Save — because Pixel-mode Save writes `.aseprite`; File ▸ Export ▸ Animation… already writes APNG and GIF, and animated WebP encoding has no pure-Rust encoder
- Importing the GIF's palette into the document palette — because the document stays RGBA like a PNG opened in Pixel mode; the palette panel can load colours from a PNG strip
- Keeping the file's loop count — because a Pixel document has no document-level loop count; the animation export asks for it
- Animated frames when a file is dropped "as layer" onto an open editor — because a layer holds one image; the first frame is added as today
- Downscaling animations above the size limit — because the owner chose the first-frame fallback instead
- Agent / MCP tools — because the agent opens documents through its own tools; a separate spec if wanted

## Acceptance criteria

- [ ] AC-01 (event): When the user opens an animated file (a GIF with more than one image descriptor, a PNG whose `acTL` announces more than one frame, a WebP with more than one animation-frame chunk; this count is also the frame count for the limits of AC-08) through any document-open path (File ▸ Open in either mode, Open on the start screen, Open Recent, a file dropped onto the start screen or dropped "as document" onto an editor, the desktop app's OS open-file), then a dialog asks "Open as animation (Pixel mode)", "Open first frame" or Cancel, and no document opens before the choice.
- [ ] AC-02 (always): A GIF, WebP or PNG file that is not animated in the sense of AC-01 opens as before without the dialog.
- [ ] AC-03 (event): When the user chooses "Open as animation", then a Pixel-mode document opens in its own tab with the animation's canvas width and height, one image layer named "Layer 1" at full opacity and Normal blend, and one frame per animation frame, in file order.
- [ ] AC-04 (always): Every frame of the opened animation holds the full canvas image as the format defines it at that point: the canvas starts fully transparent; each frame's image is placed at its offset (parts beyond the canvas are cut off) and blended (APNG and WebP: source or over, as the frame says; GIF: over, its transparent index transparent); after the frame its disposal applies (none: keep; background: clear the frame's rectangle to fully transparent, in all three formats; previous: restore the rectangle to what it was before the frame). Hand-made fixtures with known expected frames are the reference.
- [ ] AC-05 (always): Every frame's duration is the file's delay in milliseconds (GIF hundredths × 10; APNG numerator / denominator seconds, a denominator of 0 meaning 100 as the APNG format defines; WebP ms), rounded half up to whole ms; then a GIF delay of 10 ms or less and a delay of 0 ms in any format become 100 ms, and a result above 65535 ms becomes 65535 ms.
- [ ] AC-06 (event): When the user chooses "Open first frame", then the file opens as the same open path opens it today: in Image mode with the first frame as one layer, except an APNG opened through Pixel mode's own File ▸ Open, which opens as a one-frame Pixel document as today.
- [ ] AC-07 (event): When the user cancels, then no document opens and the open documents are unchanged.
- [ ] AC-08 (state): While the animation's canvas is wider or taller than 4096 px, or `frames × width × height` exceeds 67 108 864 pixels, "Open as animation" is disabled and the dialog names the exceeded limit together with the file's value (e.g. "5120 × 300 px, the limit is 4096 px per side"); "Open first frame" stays available.
- [ ] AC-09 (error): If a frame after the first cannot be decoded, or the file ends inside a frame or before the frame count of AC-01, then choosing "Open as animation" opens no document and a notice says the animation cannot be read completely; "Open first frame" still works for the same file.
- [ ] AC-10 (always): The animation document is unsaved, named after the file with its extension replaced by `.aseprite` (`walk.gif` → `walk.aseprite`), has the RGBA colour mode and the palette a document from File ▸ New gets, and its first Save asks for a location.
- [ ] AC-11 (always): An animation exported by Pincel's File ▸ Export ▸ Animation… as APNG at scale 1 and opened again as animation gives back the same frame count, the same pixels per frame and the same durations for every frame longer than 0 ms; as GIF the pixels match wherever the export was exact (at most 255 opaque colours per frame and no partial alpha) and the durations match where they are multiples of 10 ms above 10 ms.
- [ ] AC-12 (always): Every user-visible string of the dialog and its notes has a translated key in every UI language's string table (the existing key-parity check passes).
- [ ] AC-13 (error): If the file's first frame cannot be decoded, or its canvas width or height is 0, then no choice dialog appears and the open fails with the notice an unreadable image gets today.
- [ ] AC-14 (always): The document opened from the dialog is made from the file content read when the dialog appeared; moving, deleting or changing the file while the dialog is open changes nothing.
- [ ] AC-15 (state): While a choice dialog is open, it is modal (menus and drops on the editor are blocked), and files the OS asks the app to open wait; after the user makes any choice, Cancel included, they are opened in arrival order, each animated one with its own dialog.

## Technical notes

- Decoding belongs in `pincel-core` (its codec module) behind the Pixel wasm adapter: GIF through the `gif` crate it already depends on, APNG through `png`, WebP through `image-webp` (new direct dependency, Decision Log 2026-11-17). The frame count / size probe used by the dialog comes from the same decoders without decoding every frame where the format allows.
- Routing: the shell (`App.svelte` / `lib/fs`) asks before choosing the editor; `sniffFormat` already tells GIF, WebP and PNG apart.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| GIF frame smaller than the logical screen at an offset | Composited onto the full canvas at its offset (AC-04) |
| GIF with "restore to previous" disposal | The frame after it starts from the canvas as it was before the disposed frame |
| APNG whose default image is not part of the animation (IDAT outside `fcTL`) | Only the animation frames count; the default image is no frame |
| Animated WebP with a background colour in its `ANIM` chunk | Disposal to background clears to transparent, as browsers do |
| A 1-frame animated WebP / APNG (has `ANIM` / `acTL`, one frame) | Treated as static, no dialog (AC-02), because one frame needs no timeline |
| GIF frames with their own local colour table and transparent index | Each frame decoded with its own table and index |
| 5000-frame 64 × 64 GIF that takes seconds to decode | The app shows its busy state until the document opens; no cancel (see Assumptions) |
| File extension says `.gif` but the content is a PNG | The content decides (`sniffFormat`), as for every open |
| 5000-frame 64 × 64 GIF | Under the pixel budget (20 480 000 px) → opens as animation |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Pixel budget of 67 108 864 decoded pixels (256 MiB RGBA) | Keeps an animation well inside the 4 GiB wasm32 address space next to undo history and the render buffers | Low; pixel-art animations are far below, long video-like GIFs are refused with the reason |
| Delays of 0–10 ms become 100 ms | What browsers do, so the animation plays as the user saw it | Low |
| The choice is asked every time, not remembered | No owner request for a remembered answer; the drop dialog's "remember" exists and can be extended later | Low |
| No undo history for the open itself | Opening a document never is an undo step | None |
| No partial open of a broken or truncated animation (AC-09) | A silently shortened animation would lose frames unnoticed; the first-frame option stays | Low |
| GIF delays of 10 ms or less become 100 ms; APNG and WebP keep short delays | Browsers play such GIFs at 100 ms, so the GIF plays as the user saw it; for APNG and WebP that rule is not established, so their delays stay exact except 0 ms, which a Pixel frame cannot sensibly hold | Low |
| Durations above 65535 ms are clamped | A Pixel frame duration is 16 bit, as in the Aseprite format | None in practice |
| RGBA, not Indexed, even for a GIF | Pixel mode cannot edit indexed pixels yet (Phase 2) | Low |
| No cancel while frames decode | The pixel budget bounds the work; a cancel needs a background job the open path does not have | Medium on slow devices; a later spec can add it |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

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
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |

## Decisions

- 2026-11-17: Ask on open; GIF, WebP and APNG; `image-webp` as WebP decoder in `pincel-core`; first-frame fallback above the size limit (owner).
