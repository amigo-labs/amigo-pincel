---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §1.1, pincel.md §5.1, pincel.md §7.6, pincel.md §15 (2026-09-06 Two cores, one shell; 2026-10-29 save in the original format; 2026-11-05 MCP tool parity)]
depends_on: [image-formats-more]
---

# More file formats in Image mode: AVIF and JPEG XL

## Context

paint.net opens and saves AVIF and JPEG XL. Both are the modern photo and screenshot formats: phones, browsers and image CDNs hand out AVIF, and JPEG XL is spreading through camera and design tools. Image mode opens neither today.

Unlike TGA, DDS and ICO (`image-formats-more`), neither format can be enabled with an `image` feature alone. Research on 2026-11-05 against `image` 0.25.10 (the workspace's `Cargo.lock`) and the crates' published manifests:

| Capability | Crate | License | Pure Rust? | wasm32 | Size estimate (rough, unmeasured; AC-11 measures after `wasm-opt`) |
| ---------- | ----- | ------- | ---------- | ------ | -------------------------------- |
| AVIF encode | `image` feature `avif` → `ravif` 0.13 + `rav1e` 0.8 (+ `rgb`) | BSD-3-Clause / BSD-2-Clause | yes (asm only with feature `nasm`, off) | builds; single-threaded, slow | +1.5 – 2.5 MB |
| AVIF decode, option 1 | `image` feature `avif-native` → `dav1d` 0.11 bindings + `mp4parse` 0.17 | MIT (bindings) / BSD-2 (libdav1d) / MPL-2.0 | no: links the C library libdav1d via the system | not usable for `wasm32-unknown-unknown` | native only |
| AVIF decode, option 2 | `rav1d` 1.1 (Rust port of dav1d, feature `asm` off) + `avif-parse` 1.4 for the container, glue (YUV → RGB, alpha plane) in `pincel-image-core` | BSD-2-Clause / MPL-2.0 | yes | not verified: depends on `libc`, `parking_lot`, threads | +1 – 1.5 MB |
| AVIF decode, option 3 | Browser WebCodecs `ImageDecoder`, pixels copied out of the `VideoFrame` with `copyTo` (no canvas) | — | — (no crate) | — | 0; not available to the headless MCP server, browser coverage varies |
| JPEG XL decode | `jxl-oxide` 0.12 with feature `image` (an `image::ImageDecoder`), default features off (no `rayon`), `lcms2` off (C) | MIT OR Apache-2.0 | yes | yes (upstream ships a wasm build) | +0.7 – 1.2 MB |
| JPEG XL encode | `zune-jpegxl` 0.5 (lossless, modular mode only) | MIT OR Apache-2.0 OR Zlib | yes, no `unsafe` | yes | +0.1 – 0.3 MB |

Size estimates are from the crates' code size, not measured in this workspace. None of these crates is in the lockfile today. Every row is a new dependency (AGENTS.md §9). The owner chose on 2026-11-05 (see `## Decisions`): AVIF decode through `rav1d` + `avif-parse` behind a spike gate, no AVIF encoder, `jxl-oxide` + `zune-jpegxl` for JPEG XL, all in the existing Image-mode wasm module capped at +2.0 MB. Those dependencies are approved.

Per Decision Log 2026-11-05 (MCP tool parity), MCP `doc.open` opens whatever the editor opens (AC-09); MCP write tools belong to `mcp-parity-export`.

## Scope

### In scope

- Open JPEG XL (codestream and container; lossy and lossless) as a one-layer Image document; AVIF only if the spike succeeds (it did not, 2026-11-12)
- Detection by content signature
- Save back and Save As for JPEG XL (lossless)
- Open-picker filters, desktop file associations, drop as layer, MCP `doc.open`
- A measured, bounded growth of the Image-mode wasm module

### Out of scope

- HEIC / HEIF — because the only usable decoders are C libraries under LGPL (libheif, libde265) with HEVC patent licensing, none of them builds for the browser, and a desktop-only format would split the format list by platform
- Writing AVIF — because the pure-Rust encoder adds the most wasm of any row above and encodes a 4K image for several seconds on the single wasm thread, freezing the editor; it fits once exports run off the main thread (`performance-background-jobs`); owner's choice 2026-11-05.
- Lossy JPEG XL encoding — because no pure-Rust lossy JPEG XL encoder exists; libjxl is C++
- Animated AVIF / JPEG XL beyond the first frame — because an Image document is one still canvas; Pixel mode's animation is `.aseprite`-based
- Keeping HDR, wide-gamut or ICC color data — because Image mode is 8-bit sRGB and drops metadata on open for every format (`image-files` AC-02)
- Pixel mode — because raster files open in Image mode (pincel.md §1.1)

## AVIF spike gate

AVIF decoding is built only after a spike succeeds: `rav1d` (feature `asm` off) plus `avif-parse` build for `wasm32-unknown-unknown` with the workspace toolchain, decode an 8-bit, a 10-bit and a 12-bit fixture in a wasm test, and the measured growth of the release Image-mode wasm (after `wasm-opt`) for all codecs of this spec together stays within the AC-11 cap. `spec-build` records the spike result and the sizes under `## Decisions`.

If the spike fails, AVIF decoding is dropped from this spec — no fallback to libdav1d or WebCodecs: AC-01, AC-02 and AC-07 are marked dropped (revision mode), AC-04, AC-08 – AC-10, AC-12 and AC-15 apply to JPEG XL only, and an AVIF file fails to open like any undecodable image (`image-files` AC-13). JPEG XL is built either way.

## Acceptance criteria

- ~~AC-01 (event): When the user opens an AVIF still image (8, 10 or 12 bit, with or without alpha), then an Image document opens with one layer named "Background" at the image's width and height, whose pixels differ from a reference PNG rendering of the same file by a mean absolute error below 2 of 255 in each of the four channels (R, G, B, A); an image without alpha opens fully opaque.~~ dropped: the AVIF spike failed (see "AVIF spike gate" and Decisions, 2026-11-12)
- ~~AC-02 (event): When the user opens an AVIF image sequence, then the document opens from its first frame.~~ dropped: the AVIF spike failed (see "AVIF spike gate" and Decisions, 2026-11-12)
- [ ] AC-03 (event): When the user opens a JPEG XL file (bare codestream or ISOBMFF container, lossy or lossless, with or without alpha), then an Image document opens with one layer at the image's width and height; a lossless file with 8 bits per channel opens with exactly its pixels; a lossy file, or a lossless one with more than 8 bits per channel (down-converted), opens within a mean absolute error below 2 of 255 in each of the four channels of a reference PNG rendering; an animated file opens from its first frame.
- [ ] AC-04 (always): JPEG XL files are recognised by their content whatever their extension; a file named `.jxl` whose content is not JPEG XL is not opened as JPEG XL, and an AVIF file fails to open like any undecodable image (`image-files` AC-13).
- [ ] AC-05 (always): JPEG XL files written by Image mode decode back to exactly the pixels that were written, for any pixel content including fully transparent pixels and a 1 × 1 image.
- [ ] AC-06 (event): When the user saves a document opened from a JPEG XL file, then the file is written back in place as lossless JPEG XL without a quality dialog, following the flatten prompt and saved-state rules of the other writable formats (`save-original-format` AC-01 – AC-06, AC-08).
- ~~AC-07 (event): When the user saves a document opened from an AVIF file, then Save As opens with the notice that this format cannot be written back, as for GIF (`save-original-format` AC-09), and AVIF is not among its formats.~~ dropped: the AVIF spike failed (see "AVIF spike gate" and Decisions, 2026-11-12)
- [ ] AC-08 (error): If a JPEG XL file is truncated, corrupt, or uses a feature the decoder does not support, then opening fails as for any undecodable image (`image-files` AC-13), no document is created, and an already open document stays as it was.
- [ ] AC-09 (event): When an MCP client calls `doc.open` with a workspace-relative `.jxl` file, then an Image document opens with the same size and pixels the editor's Open produces, and the `doc.open` description names JPEG XL; an undecodable file fails with `isError: true` and adds no document.
- [ ] AC-10 (always): The Open picker's "All supported files" and "Images" filters include `.jxl`, the desktop app registers it as a file association that opens in Image mode, and a JPEG XL file dropped onto an open Image document is offered as "Add as layer"; a dropped file that fails per AC-08 or AC-12, or is not recognised per AC-04 (e.g. an AVIF file, or a PNG named `.jxl`), is handled as an undecodable drop (`file-access` AC-26).
- [ ] AC-11 (always): The release Image-mode wasm module (after `wasm-opt`) is at most 2.0 MB larger than the same build at the commit before this spec's first commit (which already contains `image-formats-more`'s codecs); both sizes are recorded under `## Decisions`.
- [ ] AC-12 (error): If a JPEG XL file declares a width or height of 0 or above 32767 px, then opening fails with the size message of `image-files` AC-14 — not the undecodable-image message, also when its pixel data is missing or cut off — and no document is created.
- [ ] AC-13 (event): When the user chooses Save As…, then JPEG XL (`.jxl`) is offered next to the existing formats, and the suggested file name is the document's name with `.jxl`.
- [ ] AC-14 (error): If writing a JPEG XL file fails (Save or Save As), then an error notice says the file could not be written, an existing file at the target is left unchanged, and the document stays marked unsaved.
- [ ] AC-15 (event): When the user drops a JPEG XL file onto an open Image document and chooses "Add as layer", then it lands as a new layer above the active one in one undo step, with the same pixels Open produces.

## Technical notes

- `image` 0.25.10: `avif = ["dep:ravif", "dep:rgb"]` is the encoder only; the decoder is `avif-native = ["dep:mp4parse", "dep:dav1d"]` (`make_decoder` in `io/image_reader_type.rs`). `guess_format` knows AVIF only by `\0\0\0\0ftypavif` (masked size); an `avis` sequence or another major brand with `avif` as a compatible brand needs Pincel's own check.
- Signatures for `sniffFormat`: AVIF `ftyp` at offset 4 with major or compatible brand `avif` / `avis`; JPEG XL codestream `FF 0A`, container `00 00 00 0C 4A 58 4C 20 0D 0A 87 0A`. ISOBMFF also carries HEIC (`heic`, `mif1` without `avif`), which must stay unrecognised.
- `jxl-oxide` 0.12.6 depends on `image` `^0.25.8` with `default-features = false`, so it joins the existing `image` 0.25.10. Its default feature `rayon` must be off for wasm.
- AVIF decode glue for option 2 lives in `pincel-image-core::codec` (no platform deps, `Read`-based, §5.1). Its color conversion (BT.601 / BT.709 matrix, limited / full range, from the `colr` box) is what AC-01's error bound checks.
- Reference PNGs for AC-01 / AC-03 are rendered once with the format's reference tools (`avifdec`, `djxl`) and committed with the fixtures; `.avif` and `.jxl` go into `.gitattributes` as binary in the same commit.
- The UI plumbing is the same list of places as `image-formats-more` (Technical notes), plus `mimeFor` `image/avif` and `image/jxl`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Any AVIF file, or HEIC named `photo.avif` | Fails to open like an undecodable file (AC-04) |
| PNG opened, then Save As ▸ JPEG XL | Writes `.jxl`; later Saves write that file in place |
| 16-bit lossless JPEG XL | Opens down-converted to 8 bit per channel; not pixel-exact, but within AC-03's lossy bound |
| JPEG XL that is a recompressed JPEG (JPEG bitstream reconstruction box) | Opens as its decoded pixels; Save writes lossless JPEG XL, not the original JPEG |
| JPEG XL declaring 40000 × 10 px with no pixel data | AC-12: size message |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| JPEG XL saves without a dialog | The only encoder is lossless, like WebP (`save-original-format` AC-08) | Low |
| Reference renderings tolerate 2 of 255 | Different YUV → RGB rounding between decoders; larger errors indicate a wrong matrix or range | Low |

## Open questions

- AC-05 asks that a 1 × 1 image round-trips. `zune-jpegxl` 0.5.2, the approved encoder, refuses any side of 1 px (its `ZeroDimension` guard is `<= 1`), and with that guard removed its frame-size writer asserts `size > 1` (tried 2026-11-12), so it cannot write such a file. Until settled, saving a document with a 1-px side as JPEG XL fails per AC-14. Choices: reword AC-05 to sides of at least 2 px and keep the AC-14 failure (perhaps with its own notice); another encoder (no pure-Rust one is known); or a patched / vendored `zune-jpegxl`.

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | AVIF spike failed (Decisions, 2026-11-12) | dropped |        |
| AC-02 | AVIF spike failed (Decisions, 2026-11-12) | dropped |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 | AVIF spike failed (Decisions, 2026-11-12) | dropped |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |

## Decisions

- 2026-11-05: Depends on `image-formats-more` — that spec introduces the decode-with-hint path and the per-format UI plumbing this one extends.
- 2026-11-05: AVIF and JPEG XL are recognised by content only, without an extension fallback, because both always carry a signature: a `.avif` / `.jxl` file without one is something else (HEIC under an `.avif` name is the common case) and cannot be decoded as AVIF / JPEG XL anyway.
- 2026-11-05: The decode error bound covers alpha as well (all four channels), because AVIF alpha planes are usually lossy; an exact alpha would fail on correct decoders.
- 2026-11-05: AVIF decoding through `rav1d` + `avif-parse` with own glue (owner's choice), because it is pure Rust and serves the browser, the desktop app and the headless MCP server alike; libdav1d (C, native only) and WebCodecs (no MCP, uneven coverage) would split the format by platform. Gated by the spike in "AVIF spike gate"; if the spike fails, AVIF decoding is dropped.
- 2026-11-05: No AVIF encoder for now (owner's choice), because `ravif` / `rav1e` add 1.5 – 2.5 MB of wasm and encode for seconds on the main thread; revisit after `performance-background-jobs`.
- 2026-11-05: JPEG XL through `jxl-oxide` (decode) and `zune-jpegxl` (lossless encode) (owner's choice), because both are pure Rust, MIT / Apache licensed and small, and lossless save keeps Save in place working.
- 2026-11-05: The new codecs live in the existing Image-mode wasm module, capped at +2.0 MB after `wasm-opt` (AC-11) (owner's choice), because a separate lazily loaded module would add a crate and a cross-crate boundary for a growth the cap already bounds.
- 2026-11-12 (spec-build): The AVIF spike failed, so AVIF decoding is dropped as "AVIF spike gate" prescribes: `rav1d` 1.1 with default features off (no `asm`) does not build for `wasm32-unknown-unknown` with the workspace toolchain (`libc::ptrdiff_t`, `intptr_t` and `uintptr_t` are unresolved — `libc` defines no C types for that target), and its only public API is `unsafe extern "C" fn dav1d_*`, so the glue would also need new `unsafe` blocks (AGENTS.md §3.3). AC-01, AC-02 and AC-07 are dropped; AC-04, AC-08 – AC-10, AC-12 and AC-15 are reworded to JPEG XL only, as the gate says. Neither `rav1d` nor `avif-parse` joins the workspace. The fixture script keeps no AVIF part.
