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
- [ ] AC-05 (always): JPEG XL files written by Image mode decode back to exactly the pixels that were written, for any pixel content including fully transparent pixels and for any width and height of at least 2 px (smaller canvases: AC-16).
- [ ] AC-06 (event): When the user saves a document opened from a JPEG XL file, then the file is written back in place as lossless JPEG XL without a quality dialog, following the flatten prompt and saved-state rules of the other writable formats (`save-original-format` AC-01 – AC-06, AC-08).
- ~~AC-07 (event): When the user saves a document opened from an AVIF file, then Save As opens with the notice that this format cannot be written back, as for GIF (`save-original-format` AC-09), and AVIF is not among its formats.~~ dropped: the AVIF spike failed (see "AVIF spike gate" and Decisions, 2026-11-12)
- [ ] AC-08 (error): If a JPEG XL file is truncated, corrupt, or uses a feature the decoder does not support, then opening fails as for any undecodable image (`image-files` AC-13), no document is created, and an already open document stays as it was.
- [ ] AC-09 (event): When an MCP client calls `doc.open` with a workspace-relative `.jxl` file, then an Image document opens with the same size and pixels the editor's Open produces, and the `doc.open` description names JPEG XL; an undecodable file fails with `isError: true` and adds no document.
- [ ] AC-10 (always): The Open picker's "All supported files" and "Images" filters include `.jxl`, the desktop app registers it as a file association that opens in Image mode, and a JPEG XL file dropped onto an open Image document is offered as "Add as layer"; a dropped file that fails per AC-08 or AC-12, or is not recognised per AC-04 (e.g. an AVIF file), is handled as an undecodable drop (`file-access` AC-26).
- [ ] AC-11 (always): The release Image-mode wasm module (after `wasm-opt`) is at most 2.0 MB larger than the same build at the commit before this spec's first commit (which already contains `image-formats-more`'s codecs); both sizes are recorded under `## Decisions`.
- [ ] AC-12 (error): If a JPEG XL file declares a width or height of 0 or above 32767 px, then opening fails with the size message of `image-files` AC-14 — not the undecodable-image message, also when its pixel data is missing or cut off — and no document is created.
- [ ] AC-13 (event): When the user chooses Save As…, then JPEG XL (`.jxl`) is offered next to the existing formats, and the suggested file name is the document's name with `.jxl`.
- [ ] AC-14 (error): If writing a JPEG XL file fails (Save or Save As) for another reason than AC-16's size limit, then an error notice says the file could not be written, an existing file at the target is left unchanged, and the document stays marked unsaved.
- [ ] AC-15 (event): When the user drops a JPEG XL file onto an open Image document and chooses "Add as layer", then it lands as a new layer above the active one in one undo step, with the same pixels Open produces.
- [ ] AC-16 (error): If the user saves a document as JPEG XL while its canvas is 1 px wide or 1 px tall, then no file is written, a notice says that JPEG XL needs at least 2 × 2 px (not AC-14's write failure), the document stays marked unsaved and its save target is unchanged; on Save of a JPEG XL-opened document, Save As then opens with PNG preselected and without the notice that the format cannot be written back; when JPEG XL was chosen in Save As, the user can choose Save As again.

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
| JPEG XL opened, canvas then resized to 1 × 16, Save | AC-16: Save As with the 2 px notice |
| 1 × 1 document, Save As ▸ PNG | Writes PNG; only JPEG XL has the 2 px limit |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| JPEG XL saves without a dialog | The only encoder is lossless, like WebP (`save-original-format` AC-08) | Low |
| Reference renderings tolerate 2 of 255 | Different YUV → RGB rounding between decoders; larger errors indicate a wrong matrix or range | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | AVIF spike failed (Decisions, 2026-11-12) | dropped |        |
| AC-02 | AVIF spike failed (Decisions, 2026-11-12) | dropped |        |
| AC-03 | `jxl_lossless_8_bit_opens_exactly`, `jxl_lossy_and_deep_open_within_the_bound`, `jxl_animation_opens_its_first_frame` (crates/pincel-image-core/tests/formats_modern.rs); e2e `a JPEG XL file opens with its pixels and Save writes it back as JPEG XL (AC-03, AC-06)` (ui/e2e/image-formats-modern.spec.ts) | built |        |
| AC-04 | `jxl_is_recognised_by_content_and_avif_fails` (formats_modern.rs); e2e `JPEG XL is told by content: a PNG named .jxl opens as PNG, an AVIF fails (AC-04)` (ui/e2e/image-formats-modern.spec.ts) | built |        |
| AC-05 | `jxl_written_files_decode_back_exactly` (formats_modern.rs, 2×2, 3×7, 64×33 with transparent pixels); `to_jxl_bytes_with_a_1_px_side_returns_the_size_error` (formats_modern.rs, a 2 × 7 image at `JXL_MIN_SIDE`) | built |        |
| AC-06 | e2e `a JPEG XL file opens with its pixels and Save writes it back as JPEG XL (AC-03, AC-06)` (ui/e2e/image-formats-modern.spec.ts); `jobs::tests::encode_jxl_decodes_back_to_the_composite` (pincel-image-core), `export_tests::export_jxl_and_image_to_png_round_trip` (pincel-image-wasm); the `jxl` line of the job `cases.json` (`crates/pincel-image-core/tests/fixtures/jobs/`), checked by `ui/e2e/job-digests.spec.ts` (the native-backend checks went with `performance-desktop-jobs`, dropped 2026-11-13) | built |        |
| AC-07 | AVIF spike failed (Decisions, 2026-11-12) | dropped |        |
| AC-08 | `jxl_truncated_fails` (formats_modern.rs); e2e `a truncated JPEG XL fails as undecodable, a huge one with the size message (AC-08, AC-12)` (ui/e2e/image-formats-modern.spec.ts), which also keeps an open document unchanged | built |        |
| AC-09 | `tests::image_draw::doc_open_reads_jpeg_xl_like_the_editors_open` (crates/pincel-agent/src/tests/image_draw.rs), `image::doc::tests::flat_format_reads_signatures` | built |        |
| AC-10 | e2e `a dropped JPEG XL that fails to open, or an AVIF, is an undecodable drop (AC-10)` (ui/e2e/image-formats-modern.spec.ts); `the open dialog offers TGA, DDS, ICO and JPEG XL under all supported files and images …` (ui/e2e/desktop.spec.ts); ui/e2e/file-associations.spec.ts; ui/e2e/file-access.spec.ts; `tests::nsis_hooks_cover_every_file_association` (pincel-tauri) | built |        |
| AC-11 | measured before `wasm-opt` only (Decisions, 2026-11-12); the after-`wasm-opt` sizes are open | open |        |
| AC-12 | `jxl_oversized_header_fails_with_the_size_error` (formats_modern.rs); e2e `a truncated JPEG XL fails as undecodable, a huge one with the size message (AC-08, AC-12)` (ui/e2e/image-formats-modern.spec.ts) | built |        |
| AC-13 | e2e `Save As offers JPEG XL with the name <name>.jxl (AC-13)` (ui/e2e/image-formats-modern.spec.ts); `Save As offers every writable format …` (ui/e2e/save-original.spec.ts) | built |        |
| AC-14 | e2e `desktop app › a JPEG XL whose write fails says so, keeps the file on disk and the document unsaved (AC-14)` (ui/e2e/image-formats-modern.spec.ts) — a failing write through the mocked Tauri bridge, for Save in place and for Save As over an existing file | built |        |
| AC-15 | e2e `a dropped JPEG XL lands as one layer in one undo step with the pixels of Open (AC-10, AC-15)` (ui/e2e/image-formats-modern.spec.ts) | built |        |
| AC-16 | `to_jxl_bytes_with_a_1_px_side_returns_the_size_error` (formats_modern.rs, 1 × 1, 1 × 8, 8 × 1); e2e `Save As ▸ JPEG XL of a 1-px canvas says JPEG XL needs 2 × 2 px and writes nothing (AC-16)` and `Save of a JPEG XL cut to 1 px tall goes to Save As with the 2 px notice only (AC-16)` (ui/e2e/image-formats-modern.spec.ts) | built |        |

## Decisions

- 2026-11-05: Depends on `image-formats-more` — that spec introduces the decode-with-hint path and the per-format UI plumbing this one extends.
- 2026-11-05: AVIF and JPEG XL are recognised by content only, without an extension fallback, because both always carry a signature: a `.avif` / `.jxl` file without one is something else (HEIC under an `.avif` name is the common case) and cannot be decoded as AVIF / JPEG XL anyway.
- 2026-11-05: The decode error bound covers alpha as well (all four channels), because AVIF alpha planes are usually lossy; an exact alpha would fail on correct decoders.
- 2026-11-05: AVIF decoding through `rav1d` + `avif-parse` with own glue (owner's choice), because it is pure Rust and serves the browser, the desktop app and the headless MCP server alike; libdav1d (C, native only) and WebCodecs (no MCP, uneven coverage) would split the format by platform. Gated by the spike in "AVIF spike gate"; if the spike fails, AVIF decoding is dropped.
- 2026-11-05: No AVIF encoder for now (owner's choice), because `ravif` / `rav1e` add 1.5 – 2.5 MB of wasm and encode for seconds on the main thread; revisit after `performance-background-jobs`.
- 2026-11-05: JPEG XL through `jxl-oxide` (decode) and `zune-jpegxl` (lossless encode) (owner's choice), because both are pure Rust, MIT / Apache licensed and small, and lossless save keeps Save in place working.
- 2026-11-05: The new codecs live in the existing Image-mode wasm module, capped at +2.0 MB after `wasm-opt` (AC-11) (owner's choice), because a separate lazily loaded module would add a crate and a cross-crate boundary for a growth the cap already bounds.
- 2026-11-12 (spec-build): The AVIF spike failed, so AVIF decoding is dropped as "AVIF spike gate" prescribes: `rav1d` 1.1 with default features off (no `asm`) does not build for `wasm32-unknown-unknown` with the workspace toolchain (`libc::ptrdiff_t`, `intptr_t` and `uintptr_t` are unresolved — `libc` defines no C types for that target), and its only public API is `unsafe extern "C" fn dav1d_*`, so the glue would also need new `unsafe` blocks (AGENTS.md §3.3). AC-01, AC-02 and AC-07 are dropped; AC-04, AC-08 – AC-10, AC-12 and AC-15 are reworded to JPEG XL only, as the gate says. Neither `rav1d` nor `avif-parse` joins the workspace. The fixture script keeps no AVIF part.
- 2026-11-12 (spec-build): AC-10's example "a PNG named `.jxl`" (written in the revision above) contradicted AC-04, under which such a file opens as PNG; the example is removed — a PNG dropped under a `.jxl` name lands as a PNG layer.
- 2026-11-12 (spec-build): `.jxl` maps to the format `jxl` by name as well (like ICO and DDS), so a damaged `.jxl` fails as undecodable rather than "not a supported file"; content still wins. The Save As label is the plain string "JPEG XL" like "TGA"; the only new translation is "/ JPEG XL" in `shell.unsupported`. The desktop association is role Editor, "JPEG XL Image", `image/jxl`.
- 2026-11-12 (spec-build): AC-11 sizes of the release Image-mode wasm (`wasm-pack build --release`, before `wasm-opt`, which the crate turns off and only `release.yml` runs): 6 101 625 bytes at 6210c2ce (the commit before this spec's implementation) and 7 774 689 bytes with JPEG XL, +1 673 064 bytes (1.60 MiB). The sizes after `wasm-opt -O3 --enable-simd` (binaryen 133, as `release.yml`) still have to be taken; binaryen is not installed here and was not fetched in this session.
- 2026-11-13 (revision, owner's choice): `zune-jpegxl` 0.5.2, the approved encoder, cannot write a side of 1 px (its `ZeroDimension` guard is `<= 1`, and its frame-size writer asserts `size > 1`). AC-05 now covers widths and heights of at least 2 px, and saving a smaller canvas as JPEG XL ends with its own notice (new AC-16, modelled on ICO's size limit in `image-formats-more` AC-08) instead of the generic write failure; AC-14 is the failure of any other write. No new dependency and no patched or vendored encoder.
- 2026-11-13 (spec-build): AC-16 is checked in the UI before anything is encoded (`sizeNotice` in `ui/src/modes/image/core/save-flow.ts`, shared with ICO's 256 px limit), so neither the save picker nor the encode job runs; Save of a too-small JPEG XL document goes to Save As with PNG preselected, as for ICO. The core refuses the size too, with its own error for every other caller (jobs, MCP): new public `CodecError::JxlTooSmall` and `codec::JXL_MIN_SIDE`. The new string `save.jxlTooSmall` is in all six dictionaries.
- 2026-11-13 (spec-build): AC-14's test used the 1-px encoder failure, which AC-16 now intercepts; it is replaced by a failing write through the mocked Tauri bridge (`writeMode: 'fail'`), which also proves the file on disk stays unchanged. That test was green at once: the write-failure path already behaved as AC-14 says. Red runs: the core test failed with "JPEG XL: The width is less than 2" instead of the size error, and both AC-16 e2e tests failed before the UI change (no notice after Save As; no Save As dialog after Save).
