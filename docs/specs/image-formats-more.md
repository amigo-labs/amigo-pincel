---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §1.1, pincel.md §7.6, pincel.md §11.3, pincel.md §15 (2026-09-06 Two cores, one shell; 2026-10-29 save in the original format; 2026-11-05 MCP tool parity)]
---

# More file formats in Image mode: TGA, DDS, ICO

## Context

paint.net opens and saves TGA, DDS and ICO besides the formats Image mode already handles (PNG, JPEG, WebP, BMP, GIF, TIFF; `image-files`, `save-original-format`). Game-asset and icon work runs into all three: TGA is the classic texture exchange format, DDS the GPU texture container, ICO the Windows icon.

The workspace's `image` crate (0.25.10 in `Cargo.lock`) already contains decoders for all three and encoders for TGA and ICO, behind its features `tga`, `dds` and `ico`. The workspace enables only `png`, `jpeg`, `webp`, `bmp`, `gif` and `tiff` (Decision Log 2026-09-06). None of the three features pulls a new crate into the lockfile (`ico` needs `bmp` and `png`, both already on), but enabling them changes what the dependency compiles; the owner approved them on 2026-11-05 (see `## Decisions`).

This is the first of three specs split from the paint.net parity topic "more file formats and printing":

- `image-formats-more` (this one): TGA, DDS, ICO — no new crate
- `image-formats-more-modern`: AVIF and JPEG XL — new crates, approved by the owner with a wasm size cap
- `image-formats-more-print`: File ▸ Print…

Per Decision Log 2026-11-05 (MCP tool parity), the MCP `doc.open` tool opens every format the editor opens; this spec covers that. Writing the new formats from MCP (flat exports, save in the original format) belongs to the backlog spec `mcp-parity-export`, which already lists "BMP / TIFF, save in the original format".

## Scope

### In scope

- Open TGA, DDS and ICO as a one-layer Image document, through every way into Image mode (Open, drag-and-drop, Open Recent, the desktop file association)
- Format detection: by content where the format has a signature (DDS, ICO), by the `.tga` extension for TGA, which has none
- Save back in place for TGA and ICO, with the existing flatten prompt; Save As offering TGA and ICO
- DDS and over-sized ICO going to Save As with a notice, as GIF does today
- Dropping a TGA, DDS or ICO file onto an open Image document as a layer
- Open-picker filters and desktop file associations for `.tga`, `.dds`, `.ico`
- MCP `doc.open` of the three formats

### Out of scope

- Writing DDS — because `image` has no DDS encoder and a block-compression (BC1–BC7) encoder would be a new crate and a separate owner decision
- DDS variants other than BC1, BC2, BC3 (DXT1, DXT3, DXT5) — because `image`'s DDS decoder reads only those; BC4–BC7 and uncompressed DDS need another decoder crate
- DDS mipmaps, cube maps and texture arrays — because an Image document is one flat canvas; the top-level image of the first surface is what opens
- Multi-image ICO writing (several sizes in one file) and `.cur` cursors — because an Image document has one canvas size; a multi-size icon export is its own feature
- Choosing which entry of a multi-image ICO opens — because paint.net-style editors open the best entry and `image` already picks it (highest bit depth, then largest area)
- MCP export / save of TGA and ICO — because `mcp-parity-export` owns the MCP write tools and their naming
- Pixel mode — because a raster file always opens in Image mode (pincel.md §1.1, mode follows the document); Pixel mode's "Add as layer" stays PNG-only (`file-access`)
- AVIF, JPEG XL, HEIC — because they need new crates (`image-formats-more-modern`; HEIC is out of scope there)

## Acceptance criteria

- [ ] AC-01 (event): When the user opens a TGA file (uncompressed or RLE; true-color 24 / 32 bit, grayscale or color-mapped), then an Image document opens with one layer named "Background" holding the image's pixels at its width and height, with its alpha channel kept.
- [ ] AC-02 (event): When the user opens a DDS file compressed as BC1, BC2 or BC3 (FourCC `DXT1` / `DXT3` / `DXT5`, or a DX10 header with one of those formats), then an Image document opens with one layer holding the first surface's top-level image.
- [ ] AC-03 (event): When the user opens an ICO file holding several images, then the document opens from the image with the highest bit depth, and among equal bit depths the largest one.
- [ ] AC-04 (always): DDS and ICO files are recognised by their content whatever their extension; a TGA file is recognised by its `.tga` extension in any letter case, and a TGA file without that extension is not recognised; a file whose content carries another known signature opens as that format even when it is named `.tga`, `.dds` or `.ico`.
- [ ] AC-05 (always): TGA and ICO files written by Image mode decode back to exactly the RGBA pixels that were written — including the color values of fully transparent pixels — for randomly generated images from 1 × 1 to 64 × 64 px with random RGBA values (property test), and for a 1 × 1 image, a fully transparent image and a 256 × 256 px image.
- [ ] AC-06 (event): When the user saves a document opened from a TGA file, then the file is written back in place as TGA under its own name, following the same flatten prompt and saved-state rules as the other writable formats (`save-original-format` AC-01 – AC-06).
- [ ] AC-07 (event): When the user saves a document opened from an ICO file whose canvas is at most 256 × 256 px, then the file is written back in place as an ICO holding one image of the canvas size, following the same flatten prompt and saved-state rules as AC-06.
- [ ] AC-08 (error): If the user saves a document as ICO while its canvas is wider or taller than 256 px, then no file is written, a notice says that ICO holds at most 256 × 256 px, the document stays marked unsaved and its save target is unchanged; on Save of an ICO-opened document, Save As then opens; when ICO was chosen in Save As, the user can choose Save As again.
- [ ] AC-09 (event): When the user saves a document opened from a DDS file, then Save As opens with the notice that this format cannot be written back, as for GIF (`save-original-format` AC-09).
- [ ] AC-10 (event): When the user chooses Save As…, then TGA (`.tga`) and ICO (`.ico`) are offered next to the existing formats, and the suggested file name is the document's name with the chosen extension.
- [ ] AC-11 (error): If a DDS file has a valid header but uses a variant outside BC1 – BC3, then opening fails with a message naming DDS and saying this variant is not supported; if its header declares a width or height that is not a multiple of 4, then opening fails with a message saying DDS width and height must be multiples of 4. In both cases no document is created and an already open document stays as it was.
- [ ] AC-12 (error): If a `.tga`, `.dds` or `.ico` file is truncated or not decodable for any reason other than those of AC-11 (a DDS whose header is valid and supported but whose data is cut off included), then opening fails with the message for any undecodable image (`image-files` AC-13) and no document is created.
- [ ] AC-13 (event): When the user drops a TGA, DDS or ICO file onto an open Image document, then it is offered as "Add as layer" like the other raster formats and, when chosen, lands as a new layer above the active one in one undo step; a dropped file that fails per AC-11 or AC-12, or a TGA without the `.tga` extension, is handled as an undecodable drop (`file-access` AC-26).
- [ ] AC-14 (always): The Open picker's "All supported files" and "Images" filters (browser and desktop) include `.tga`, `.dds` and `.ico`, and the desktop app registers `.tga`, `.dds` and `.ico` as file associations that open in Image mode.
- [ ] AC-15 (event): When an MCP client calls `doc.open` with a workspace-relative `.tga`, `.dds` or `.ico` file (extension in any letter case; DDS and ICO also without extension, recognised as in AC-04), then an Image document opens with the same size and pixels the editor's Open produces for that file, and the `doc.open` tool description names the three formats.
- [ ] AC-16 (error): If MCP `doc.open` gets a file that fails per AC-11 or AC-12, then the call fails with `isError: true` and the same distinct reason the editor shows — unsupported DDS variant, DDS size not a multiple of 4, or undecodable image — and no document is added to the workspace.

## Technical notes

Verified against the code and the `image` 0.25.10 sources on 2026-11-05; line numbers drift.

**`image` 0.25.10 features.** `tga = []`, `dds = []`, `ico = ["bmp", "png"]` — no optional crate behind any of them. Encoders exist for TGA (`TgaEncoder`, RLE on by default) and ICO (`IcoEncoder`, PNG-compressed entries, 1–256 px per edge); `ImageFormat::Dds.writing_enabled()` is `false`. The DDS decoder (`codecs/dds.rs`) reads `DXT1/3/5` FourCC and DX10 DXGI formats 70–78 only, refuses everything else as `Unsupported`, and `DxtDecoder::new` refuses dimensions that are not multiples of 4. The ICO decoder picks `best_entry` by (bit depth, area).

**TGA has no signature.** `image::guess_format` documents "TGA is not supported by this function", so `pincel-image-core::codec::decode` (`load_from_memory`, auto-detect) cannot open TGA. The decoder needs a format hint. `pincel-image-wasm::open_image(data, _mime_type)` already takes a MIME type and ignores it; the controller already passes `mimeFor(...)` from the file name (`ui/src/modes/image/core/controller.ts` `openBytes`). Signature first, hint second keeps AC-04. `paste_image` (drop as layer) needs the same hint.

**Places that list formats today** (each needs the three new ones):

- Workspace `Cargo.toml` `image` features
- `crates/pincel-image-core/src/codec/mod.rs` — `decode`, new `to_tga_bytes` / `to_ico_bytes`
- `crates/pincel-image-wasm/src/lib.rs` — `open_image`, `paste_image`, new `export_tga` / `export_ico`
- `ui/src/lib/fs/index.ts` — `FileFormat`, `IMAGE_TYPES`, `IMAGE_EXTENSIONS`, `ImageSaveFormat`, `SAVE_TYPES` / `SAVE_FILTERS`, `sniffFormat` (DDS `DDS `, ICO `00 00 01 00`), `formatFromName`, `mimeFor` (`image/x-targa`, `image/vnd-ms.dds`, `image/x-icon` — the names `image::ImageFormat::from_mime_type` maps)
- `ui/src/modes/image/core/document-file.ts` — `DocFormat`, `isWritable` (DDS not writable), `encode`, `EXTENSION`; the 256 px ICO check before writing
- `ui/src/modes/image/ImageEditor.svelte` — `layerFormats`
- `src-tauri/tauri.conf.json` — `bundle.fileAssociations`
- `crates/pincel-agent/src/image/doc.rs` `open` and the `doc.open` description in `crates/pincel-agent/src/pixel/doc.rs`
- `ui/src/lib/i18n/en.ts` / `de.ts` — the ICO size notice (the "cannot be written back" notice `save.noEncoder` exists)

**Size.** The three codecs are `image`'s own code; expected growth of the Image-mode wasm is in the tens of KB (estimate, not measured). `spec-build` records the measured before / after size of `pincel_image_wasm_bg.wasm` (release, after `wasm-opt`) under `## Decisions`.

**Fixtures.** Small hand-made files under the core's test fixtures: TGA uncompressed / RLE / grayscale / color-mapped, DDS BC1 / BC2 / BC3 / DX10-BC1 plus one BC7 and one 30 × 30 file for AC-11, a multi-entry ICO. `.tga`, `.dds`, `.ico` go into `.gitattributes` as binary in the same commit (AGENTS.md §6.4).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| PNG renamed to `photo.tga` | Opens as PNG (AC-04); Save writes PNG, the format it came from |
| TGA named `.TGA` (upper case) | Recognised like `.tga` |
| A file without an extension holding TGA data (e.g. dropped from a browser) | Not recognised: opening fails like an undecodable file — TGA has no signature |
| TGA stored bottom-up or top-down (origin bit) | Opens upright in both cases |
| TGA without alpha (24 bit) | Opens fully opaque |
| ICO whose best entry is a BMP entry with an AND mask | Opens with the mask as transparency |
| ICO exactly 256 × 256 px | Saves as ICO |
| ICO opened, canvas then resized to 300 × 300, Save | AC-08: Save As with the 256 px notice |
| DDS 30 × 30 px (not a multiple of 4) | AC-11, size message |
| BC1 DDS cut off after the header | AC-12, generic undecodable message |
| MCP `doc.open` of `ICON` (no extension, ICO content) | Opens (content recognised) |
| MCP `doc.open` of `texture` (no extension, TGA content) | Fails per AC-16 as undecodable |
| Grayscale TGA opened and saved | Written as 32-bit RGBA TGA |
| DDS with mipmaps | Opens the full-size top level only |
| DDS from Pixel mode's Open | Opens in Image mode, as every raster file does |
| TGA or ICO larger than 32767 px on an edge | Refused (`image-files` AC-14) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| ICO is written as one PNG-compressed entry | `IcoEncoder`'s only mode for RGBA; Windows Vista+ and every browser read PNG entries | Low: Windows XP-era readers need BMP entries |
| An opened DDS is treated as sRGB | Image mode has no color management; metadata is dropped on open for every format (`image-files` AC-02) | Low: linear-space textures look darker than in a game engine |
| No quality or options dialog for TGA and ICO | Both are lossless; matches WebP's dialog-free save (`save-original-format` AC-08) | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `crates/pincel-image-core/tests/formats_more.rs` `tga_uncompressed_rle_grayscale_and_color_mapped_open_with_a_hint_only`; `ui/e2e/image-formats-more.spec.ts` "a TGA opens with its pixels and is saved back as TGA (AC-01, AC-06)" | built |        |
| AC-02 | `crates/pincel-image-core/tests/formats_more.rs` `dds_bc1_bc2_bc3_and_dx10_open_whatever_the_extension`; `ui/e2e/image-formats-more.spec.ts` "a DDS opens whatever its extension; Save goes to Save As (AC-02, AC-04, AC-09)" | built |        |
| AC-03 | `crates/pincel-image-core/tests/formats_more.rs` `ico_opens_the_deepest_then_largest_entry`; `ui/e2e/image-formats-more.spec.ts` "an ICO opens its deepest, then largest entry and saves back as ICO (AC-03, AC-07)" | built |        |
| AC-04 | `crates/pincel-image-core/tests/formats_more.rs` `tga_uncompressed_rle_grayscale_and_color_mapped_open_with_a_hint_only`, `dds_bc1_bc2_bc3_and_dx10_open_whatever_the_extension`; `ui/e2e/image-formats-more.spec.ts` "a TGA without its extension is refused; another signature wins over .tga (AC-04, AC-12)" | built |        |
| AC-05 | `crates/pincel-image-core/tests/formats_more.rs` `tga_and_ico_round_trip_exactly` (proptest), `tga_and_ico_round_trip_a_single_transparent_pixel`; `tga_and_ico_round_trip_a_256_px_image_exactly` | built |        |
| AC-06 | `ui/e2e/image-formats-more.spec.ts` "a TGA opens with its pixels and is saved back as TGA (AC-01, AC-06)" | built |        |
| AC-07 | `ui/e2e/image-formats-more.spec.ts` "an ICO opens its deepest, then largest entry and saves back as ICO (AC-03, AC-07)" | built |        |
| AC-08 | `ui/e2e/image-formats-more.spec.ts` "Save As offers TGA and ICO; ICO refuses more than 256 px (AC-08, AC-10)"; `crates/pincel-image-core/tests/formats_more.rs` `ico_refuses_more_than_256_px_and_garbage_fails_to_decode`; `ui/e2e/image-formats-more.spec.ts` "Save of an ICO grown past 256 px goes to Save As with the 256 px notice only (AC-08)" | built |        |
| AC-09 | `ui/e2e/image-formats-more.spec.ts` "a DDS opens whatever its extension; Save goes to Save As (AC-02, AC-04, AC-09)" | built |        |
| AC-10 | `ui/e2e/image-formats-more.spec.ts` "Save As offers TGA and ICO; ICO refuses more than 256 px (AC-08, AC-10)"; `ui/e2e/save-original.spec.ts` "Save As offers every writable format and writes the chosen one" | built |        |
| AC-11 | `crates/pincel-image-core/tests/formats_more.rs` `dds_with_an_unsupported_variant_or_size_fails_with_its_own_reason`; `ui/e2e/image-formats-more.spec.ts` "DDS variants outside BC1 – BC3 and odd sizes fail with their own reason (AC-11)" | built |        |
| AC-12 | `crates/pincel-image-core/tests/formats_more.rs` `dds_with_an_unsupported_variant_or_size_fails_with_its_own_reason` (cut-off data), `ico_refuses_more_than_256_px_and_garbage_fails_to_decode`; `ui/e2e/image-formats-more.spec.ts` "a TGA without its extension is refused; another signature wins over .tga (AC-04, AC-12)" | built |        |
| AC-13 | `ui/e2e/image-formats-more.spec.ts` "a dropped TGA, DDS or ICO lands as a new layer (AC-13)"; "a dropped TGA, DDS or ICO that fails to open is an undecodable drop (AC-13)" | built |        |
| AC-14 | `ui/e2e/file-access.spec.ts` "AC-01: Open… shows the picker restricted to the document types" (browser picker); desktop associations in `src-tauri/tauri.conf.json`, checked by `nsis_hooks_cover_every_file_association` and `mime_types_match_tauri_conf` (pincel-tauri, CI); `ui/e2e/desktop.spec.ts` "the open dialog offers TGA, DDS and ICO under all supported files and images (image-formats-more AC-14)"; `ui/e2e/file-associations.spec.ts` | built |        |
| AC-15 | `crates/pincel-agent/src/tests/image_draw.rs` `doc_open_reads_tga_dds_and_ico_with_the_editors_reasons` | built |        |
| AC-16 | `crates/pincel-agent/src/tests/image_draw.rs` `doc_open_reads_tga_dds_and_ico_with_the_editors_reasons` | built |        |

## Decisions

- 2026-11-05: Split from the paint.net parity topic into three specs (this, `image-formats-more-modern`, `image-formats-more-print`) — the formats without new crates carry the least risk, AVIF / JPEG XL bring new crates and a size budget, and printing touches neither codec nor core.
- 2026-11-05: Format detection by content first, file extension second — keeps today's `sniffFormat ?? formatFromName` order and makes a mislabelled file open as what it really is.
- 2026-11-05: TGA is always written as 32-bit true-color RGBA with RLE, whatever the source (24-bit, grayscale or color-mapped), because the document is RGBA and keeping the alpha channel matters more than reproducing the source's layout; RLE is `TgaEncoder`'s default and every TGA reader handles it.
- 2026-11-05: DDS size and variant errors get their own messages, because "not supported" for a merely mis-sized texture sends the user looking for the wrong fix; anything else is the generic undecodable-image message.
- 2026-11-05: Enable the `image` features `tga`, `dds` and `ico` (owner's choice), because they add no crate to the lockfile, cost a few tens of KB of wasm, and DDS read covers the common legacy texture files paint.net users expect in the Open dialog.
- 2026-11-08 (spec-build) — AC-11's two reasons come from a header pre-check in `pincel-image-core::codec` (`check_dds`: FourCC / DXGI format and width / height at their DDS offsets) before the `image` decoder runs, because the decoder reports both as the same generic `Unsupported` error.
- 2026-11-08 (spec-build) — The editor passes the file name as the decoder's hint when the name ends in `.tga` (Open), and a dropped file's format comes from its content, else its name (Drop): browsers report an empty or generic MIME type for TGA, DDS and ICO.
- 2026-11-08 (spec-build) — A dropped TGA / DDS / ICO is first converted to PNG by the core (`image_to_png`), so the existing paste path — which sizes the image through the browser — and its expand prompt work unchanged.
- 2026-11-08 (spec-build) — The desktop file associations use the shared-mime-info names (`image/x-tga`, `image/x-dds`, `image/vnd.microsoft.icon`), so Linux desktops recognise them; the in-app decoder hints keep the `image` crate's names (`image/x-targa`, `image/vnd-ms.dds`, `image/x-icon`).
- 2026-11-08 (spec-build) — The wasm size before / after was not measured: this container has no `wasm-opt`, and a size without it would not be the figure the Technical notes ask for. The Cloudflare build (which runs `wasm-opt`) reports the release size on the next deploy.
- 2026-11-08 (spec-build) — The fixtures are built byte by byte inside the tests (Rust and e2e) instead of committed binary files, so no `.gitattributes` change was needed.

