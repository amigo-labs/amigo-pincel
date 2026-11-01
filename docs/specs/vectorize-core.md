---
status: todo
created: 2026-11-01
size: L
mode: both
design: [pincel.md §2.1, pincel.md §2.3, pincel.md §4.5, pincel.md §15 (2026-11-01 Vectorizer core as pincel-trace), pincel.md §15 (2026-09-06 Fineliner feature parity)]
---

# Vectorize core (`pincel-trace`)

## Context

The owner's separate app Vectorizer (`daniel-rck/Vectorizer`, a React PWA) turns raster images into SVG: a self-written implementation of Selinger's Potrace algorithm, a colour mode that reduces the image to a few colours and traces one layer per colour, a monochrome mode with an Otsu threshold, despeckle, and a pinned palette. Its core is about 1 500 lines of DOM-free TypeScript with 22 tests whose expected values come from a verified prototype.

Pincel takes these functions over and the Vectorizer repository is deleted afterwards (Decision Log 2026-11-01). Following the Fineliner precedent (Decision Log 2026-09-06), the core becomes a Rust crate in this workspace, `pincel-trace`, a pure library that both modes, the desktop app, the agent tools and a later CLI can call. This spec covers that crate only. The export dialog that turns a document into an SVG file is the follow-up spec `vectorize-dialog`. The only product of vectorizing is an SVG export: the result never goes back into the document.

On top of the Vectorizer's functions the crate gains a pixel-exact mode. Potrace smooths pixel staircases even with every corner kept, so pixel art needs outlines that follow the pixel edges exactly.

Terms used below:

- **Image**: an RGBA8 buffer with a width and height, row-major, 4 bytes per pixel, not premultiplied.
- **Session**: the object that holds one image and traces it any number of times with different options. Creating it validates the image (AC-18); every trace reads the same image.
- **Modes**: the two **Potrace modes** — *colour* and *monochrome* — and *pixel-exact*.
- **Opaque pixel** (colour mode): alpha ≥ 128; a pixel with alpha < 128 is ignored. Monochrome mode instead blends each pixel towards white by its transparency (AC-09); pixel-exact mode keeps every pixel with alpha > 0 (AC-12).
- **Layer**: one colour, the number of pixels assigned to it (its **area**), and the vector outlines of those pixels as **path data** (the content of an SVG `d` attribute).
- **Outline**: one closed path in the path data, written as an absolute move, relative steps and `z`.
- **Minimum area** (Potrace modes): an outline whose enclosed area is at most this many px² is dropped. This is Potrace's `turdsize`: the area enclosed by the traced pixel boundary, so a single pixel encloses 1 px² and a hole is an outline of its own, measured on its own.
- **Segment** (statistics): in the Potrace modes one Bézier curve or one corner (a corner is written as two line steps); in pixel-exact mode one line step. **Curve segments** are Bézier curves; **straight segments** are corners in the Potrace modes and every line step in pixel-exact mode.
- **Default options**: colour mode with 6 colours, no pinned palette, despeckle off; Potrace parameters turn policy *minority*, minimum area 2, corner threshold (`alphamax`) 1.0, curve optimisation on, optimisation tolerance 0.2 — the reference implementation's defaults. Monochrome defaults: automatic threshold, invert off, fill `#000000`.
- **Reference implementation**: Vectorizer at commit `32dd6c8` (`src/lib/potrace/*.ts`, `src/worker/trace.worker.ts`, `src/app/lib/svg.ts`).
- **Golden corpus**: the test images and option sets of AC-01, with the reference implementation's output, generated once and committed into this repository.
- **Test images**: the procedurally generated images in the table under Technical notes, named in *italics* below.

## Scope

### In scope

- The two Potrace modes of the reference implementation: colour (2–16 colours, or a pinned palette) and monochrome (manual or automatic threshold, invert, fill colour), each with optional despeckle and the Potrace parameters (turn policy, minimum area, corner threshold, curve optimisation and its tolerance)
- Pixel-exact mode: one layer per distinct colour, outlines along pixel edges, at most 256 colours
- The trace result: layers in drawing order, statistics, and the threshold used
- Building the SVG document from a result
- Tracing one image repeatedly with changing options (the live-preview use the dialog will need)
- The golden corpus and the benchmark against the reference implementation

### Out of scope

- The export dialog, menu entries, preview, file saving and the wasm or Tauri bindings — because they are the follow-up spec `vectorize-dialog`, and the crate must stand and be tested on its own first
- Vector layers or placing the result back into the document — because the owner decided that an SVG export is the only output
- Anchor points for an on-canvas overlay — because the reference implementation needs them only for its own preview; the dialog spec decides whether its preview needs them, and adding them later does not change existing results
- Downscaling large images before tracing — because the reference implementation does that in its app, not its core; whether Pincel downscales is a dialog decision, and pixel art must not be resampled
- Centerline tracing, batch export, Sauvola thresholding — because they are on the Vectorizer's backlog, not built there either
- Running the trace off the main thread or cancelling it midway — because that is the caller's concern (`vectorize-dialog`, `performance-background-jobs`); the crate stays free of threads and platform APIs like `pincel-effects`
- A time budget for images other than AC-20's three — because the reference implementation measured only those; `vectorize-dialog` sets what the preview needs

## Acceptance criteria

- [ ] AC-01 (always): For every image and option set in the golden corpus, the result has the same number of layers in the same order, each with the same colour and area as the reference implementation's output, the same statistics and threshold used, and path data with the same command sequence whose numbers differ by at most 0.001 (one step of the 3-decimal output).
- [ ] AC-02 (always): The *disc* traced in monochrome mode with default options gives one outline of at most 6 segments, and no point of its curves (sampled at t = 0, 0.1, …, 1 per curve) lies more than 1.5 px from the circle of radius 30.
- [ ] AC-03 (always): The *ring* traced in monochrome mode with default options gives two outlines; with the even-odd rule, every pixel centre at least 1.5 px from both circles lies inside the filled area exactly when its pixel is filled.
- [ ] AC-04 (always): The *rectangle* traced in monochrome mode with default options has its corners at exactly (10, 10), (70, 10), (70, 50) and (10, 50), and its path data starts with `M10 30`, as in the reference implementation; no path data in any result contains `NaN`, `inf` or an empty number.
- [ ] AC-05 (option): Where a Potrace mode traces with minimum area n, outlines enclosing n px² or less are dropped: with n = 2 the *square with a stray pixel* gives one outline, with n = 0 it gives two.
- [ ] AC-06 (always): Every outline is one absolute move, then relative line and curve steps, then `z`, so the file stays small (the reference measured about 35 % less than absolute coordinates); summing an outline's relative steps returns to its move point within 0.002 px.
- [ ] AC-07 (event): When the *four-colour image* is traced in colour mode with 6 colours, then the result holds exactly the colours `#f4efe6`, `#2e86ab`, `#c0392b` and `#27ae60`, one layer each, the largest area first, and the areas sum to 30 000 (all its pixels are opaque).
- [ ] AC-08 (always): In colour mode, pixels with alpha < 128 belong to no layer, and the 1 × 1 fully transparent image traces to zero layers without an error.
- [ ] AC-09 (option): Where monochrome mode is set, a pixel is filled when its brightness 0.299 R + 0.587 G + 0.114 B, blended towards 255 by its transparency (brightness × a + 255 × (1 − a), a = alpha / 255), is below the threshold — or not below it when invert is set; the one layer carries the fill colour; a manual threshold is reported unchanged as the threshold used; an automatic threshold reports the threshold it chose, which lies between 100 and 160 for the *noisy two-tone image* (reference: 130); when no pixel is filled, the result has zero layers.
- [ ] AC-10 (option): Where despeckle is set, the *speckled two-tone image* traced in monochrome mode with manual threshold 130 and minimum area 0 gives fewer than a fifth of the outlines it gives without despeckle (reference: 596 → 12).
- [ ] AC-11 (option): Where colour mode has a pinned palette, the colour count is ignored; every pixel with alpha ≥ 128 goes to the pinned colour nearest in CIELAB, the first one listed on a tie (so a repeated colour never receives pixels); layers carry exactly the pinned colours that received pixels, in order of area; pinned colours are never merged with each other; an empty pinned palette gives zero layers.
- [ ] AC-12 (option): Where pixel-exact mode is set, every distinct RGBA colour with alpha > 0 becomes one layer covering exactly its pixels: a pixel centre lies inside the layer's outlines (even-odd) if and only if the pixel has that colour; every step is horizontal or vertical between integer coordinates; consecutive steps never run in the same direction (a filled rectangle is one outline with four corners); the minimum area is ignored, so a single pixel keeps its outline; a fully transparent image gives zero layers.
- [ ] AC-13 (option): Where pixel-exact mode is set, a layer whose colour has alpha below 255 carries the opacity alpha / 255, colours that differ only in alpha are separate layers, and layers are ordered by area, the largest first, ties by the colour that appears first in row-major order.
- [ ] AC-14 (event): When an SVG document is built from a result, then it is well-formed XML with `viewBox="0 0 W H"`, `width="W"` and `height="H"` of the traced image, one `path` per layer in result order with the layer's fill colour as `#rrggbb`, a `fill-opacity` where below 1, and the even-odd fill rule; layers from colour mode additionally carry a stroke of 0.5 px in their own colour with round joins, as in the reference implementation, so no hairline gap shows between neighbouring colours; monochrome and pixel-exact layers carry no stroke; a result with zero layers gives a valid document with no `path`.
- [ ] AC-15 (always): The statistics report the number of outlines, segments, curve segments and straight segments over all layers; curve segments plus straight segments equal segments; in pixel-exact mode there are no curve segments.
- [ ] AC-16 (always): Tracing the same image with the same options twice, in the same or a new session, gives byte-identical SVG documents; in colour mode, layers of equal area keep the order of their colours in the reduced palette.
- [ ] AC-17 (event): When the *four-colour image* is traced in one session with this sequence — default options; 3 colours; despeckle on; pinned palette `#000000`, `#ffffff`; monochrome automatic; pixel-exact; corner threshold 0; back to default options — then each result equals the result of tracing the image once with the same options in a new session.
- [ ] AC-18 (error): If the buffer length is not width × height × 4, the width or height is 0, or either exceeds 32 767 (Pincel's largest canvas side), then creating the session fails with an error naming the cause, and no session exists.
- [ ] AC-19 (error): If an option is out of range — colour count outside 2–16 without a pinned palette, a pinned palette with more than 16 colours, corner threshold outside 0–1.34 or not finite, optimisation tolerance negative or not finite — then tracing fails with an error naming the option, nothing is returned, and the session traces normally with valid options afterwards.
- [ ] AC-20 (error): If pixel-exact mode meets more than 256 distinct colours with alpha > 0, then tracing fails with an error that names the colour count, and the session traces normally with other options afterwards.
- [ ] AC-21 (always): No image accepted by AC-18 and no option set accepted by AC-19 makes tracing or building the SVG panic; this is checked on every test image, on 1 × 1, 1 × N, N × 1 and 2 × 2 images, and on a 3 × 3 checkerboard, each in every mode, with and without despeckle.
- [ ] AC-22 (always, manual: benchmark outside the 60 s gate): For each of the three benchmark images, the median time of a release build is no higher than the median time of the reference implementation under Node, both measured on the same machine in the same run that generates the golden corpus; both medians are recorded in this spec's Evidence.

## Technical notes

- **Crate:** `crates/pincel-trace`, workspace licence, `thiserror` as its only dependency (the colour conversion, median cut and Otsu need nothing beyond `std`), `criterion` as dev-dependency for the benchmark. No dependency on `pincel-core` or `pincel-image-core`, no threads, no I/O — the rules of `pincel-effects` and AGENTS.md §5.1. Errors: one `TraceError` enum.
- **API sketch** (shape, not a contract; the dialog spec fixes the wasm surface):
  - `TraceSession::new(rgba: &[u8], width, height) -> Result<TraceSession, TraceError>`; the length check uses checked arithmetic so `width × height × 4` cannot overflow (wasm32 has a 32-bit `usize`).
  - `session.trace(&TraceOptions) -> Result<TraceResult, TraceError>` takes `&mut self` (it fills caches); `TraceSession` is `Send`, so a caller may move it to a worker, but it is not shared during a trace.
  - `TraceOptions { mode: Color { colors: u8, pinned: Option<Vec<Rgb>> } | Mono { threshold: Auto | Manual(u8), invert: bool, fill: Rgb } | PixelExact, despeckle: bool, potrace: PotraceParams { turn_policy, min_area: u32, alpha_max: f64, opt_curve: bool, opt_tolerance: f64 } }`. Typed values replace the reference implementation's strings (hex colours, turn-policy names): parsing user text belongs to the dialog and MCP.
  - `TraceResult { width, height, layers: Vec<Layer { color: Rgba, area, path_data }>, stats, threshold_used: Option<u8> }`; `to_svg(&TraceResult) -> String`.
- **Caches** as in the reference worker: the despeckled copy; the colour reduction per (colour count or pinned palette, despeckle); the Otsu threshold per despeckle. They make a preview that changes only Potrace parameters skip the colour reduction; AC-17 makes sure they never leak one option set into another.
- **Algorithms to keep** (reasons, since the reference repository will be gone):
  - Median cut over the histogram of distinct colours (5-bit bins), not over pixels — over pixels, a dominant background eats the colour budget and small colours vanish (AC-07).
  - Pixel-to-colour assignment and merging of near-duplicate palette colours (ΔE below 2.5) in CIELAB — in RGB, black and dark blue count as close and swap.
  - Otsu picks the middle of a plateau found by exact comparison of the variance — an epsilon is meaningless at variances around 10¹², below their last representable digit.
  - Despeckle is a 3 × 3 median per channel; border pixels are copied unchanged, so images narrower or shorter than 3 px pass through as they are.
  - A path whose curve optimisation fails keeps its unoptimised curve (the reference skips the optimisation silently); this is part of the behaviour AC-01 compares.
- **Golden corpus:** generated once with Node 22 (`--experimental-strip-types`) from the reference implementation, before the Vectorizer repository is deleted, and committed under the crate's tests as JSON, together with the generator script and the reference commit hash. Option sets: each test image in its natural mode with default options; the *four-colour image* with 2, 3, 6 and 16 colours, despeckle, the pinned palettes `#f4efe6`/`#2e86ab` and `#000000`/`#ffffff`, and monochrome with invert; each of the six turn policies, corner thresholds 0 and 1.34, curve optimisation off, on the *ring*; the 1 × 1 transparent image. The same run measures the reference medians for AC-22.
- **Test images** (generated identically by the script and by Rust; noise uses the reference implementation's generator `seed = (seed × 1103515245 + 12345) & 0x7fffffff`, `rnd = seed / 0x7fffffff`, reproduced with JavaScript's arithmetic — an `f64` product, then ToInt32 — because the exact product exceeds 2⁵³):

  | Name | Size | Content |
  | ---- | ---- | ------- |
  | *disc* | 100 × 100 | filled where (x − 50)² + (y − 50)² ≤ 30² |
  | *ring* | 120 × 120 | filled where 20² ≤ (x − 60)² + (y − 60)² ≤ 45² |
  | *rectangle* | 80 × 60 | filled where 10 ≤ x < 70 and 10 ≤ y < 50 |
  | *square with a stray pixel* | 60 × 60 | filled where 10 ≤ x < 40 and 10 ≤ y < 40, plus (50, 50) |
  | *four-colour image* | 200 × 150 | `#f4efe6` background; disc r 40 at (60, 75) `#2e86ab`; 120 < x < 175: 20 < y < 70 `#c0392b`, 90 < y < 130 `#27ae60`; all opaque |
  | *noisy two-tone image* | 200 × 150 | grey 40 inside the disc r 40 at (60, 75), 220 outside, each pixel ± up to 15 from `rnd` (seed 42), opaque |
  | *speckled two-tone image* | 200 × 150 | the noisy image, then 4 % of pixels set to 0 or 255 (seed 7), as in the reference test |
  | *line art* (benchmark) | 1600 × 1600 | black strokes on white: 40 concentric circles and 40 straight lines of 3 px width |
  | *text scan* (benchmark) | 1200 × 1200 | 1 500 small dark glyph-like blobs on a light, slightly noisy background (seed 1) |
  | *noise* (benchmark) | 800 × 800 | each pixel black or white from `rnd` (seed 3); traced with minimum area 0 |

- **Pixel-exact outlines:** traced along pixel edges per colour mask, collinear steps merged; holes and nested islands rely on the even-odd rule like the Potrace modes.
- **Benchmark:** a criterion bench `pincel-trace/benches/trace.rs` beside the existing compose benches (`performance-benchmarks`), not part of the PR gate (AGENTS.md §7.5).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 1 × 1 fully transparent image, colour mode | Zero layers, no error (AC-08) |
| Image with a single opaque colour, colour mode with 6 colours | One layer of that colour (median cut cannot split one colour) |
| Fewer distinct colours than requested | As many layers as colours that received pixels, never an empty layer |
| Monochrome on an all-white image, or with manual threshold 0 | Zero layers; the threshold used is still reported (AC-09) |
| Monochrome with invert on a fully transparent image | Every pixel blends to white, so every pixel is filled: one layer covering the image (AC-09) |
| Empty histogram in Otsu | Threshold 128, no crash (reference behaviour) |
| Shape touching the image border | Outline closes along the border; no coordinate outside 0 … W and 0 … H |
| Checkerboard in pixel-exact mode | AC-12's pixel-centre test holds, whether diagonal neighbours share an outline or not |
| Pixel-exact on an image with more than 256 colours | Error naming the count (AC-20) |
| Colour count 50 together with a pinned palette | Accepted; the count is ignored (AC-11, AC-19) |
| Same colour pinned twice | The second never receives pixels, so it gives no layer (AC-11) |
| Pinned colour far from every image colour | No layer for it (AC-11) |
| Options changed back and forth between free and pinned palette | Results as from a new session each time (AC-17) |
| Despeckle on a 2 × 2 or 1 × N image | Pixels unchanged (border rule), no error (AC-21) |
| 32 767 × 1 image | Accepted; 32 768 on a side is refused (AC-18) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Monochrome mode drops its layer when nothing is filled, while the reference implementation emits a layer with empty path data | An empty `<path d="">` is noise in an exported file; the golden corpus therefore contains no all-unfilled monochrome case | Low |
| Out-of-range options are errors, not clamped (AC-19) | The reference implementation relied on its UI to keep values in range; a library called from MCP and a CLI must say what is wrong | Low |
| Pixel-exact mode treats every alpha > 0 as visible and carries the alpha as opacity | Pixel art uses semi-transparent pixels on purpose; the 128 cut-off of colour mode would lose them | Low |
| Node and Rust agree within AC-01's tolerance | Both compute in IEEE 754 doubles; basic arithmetic and `sqrt` are exactly rounded in both, but the CIELAB conversion uses `pow` and `cbrt`, which may differ in the last bit, and output is rounded to 3 decimals, so a value on a rounding boundary may differ by one step (inside the 0.001 tolerance). A last-bit difference in CIELAB changes a colour assignment only for a colour exactly between two palette colours | Medium: a corpus mismatch is investigated, not covered by raising the tolerance |
| AC-02, AC-03, AC-05, AC-09 and AC-10 keep the reference tests' own bounds next to AC-01's exact comparison | They state the behaviour in words that survive a deliberate future change of the algorithm, when the corpus would be regenerated | Low |

## Open questions

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
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |

## Decisions

- 2026-11-01 — Port to Rust instead of copying the TypeScript into `ui/`: logic belongs in a core crate (AGENTS.md §5), Pincel has no TypeScript unit-test runner, and the agent tools and CLI are Rust.
- 2026-11-01 — Export only: no vector layers, the result never returns into the document (owner's decision).
- 2026-11-01 — The code is the owner's own: it takes the workspace licence and needs no licence notice (owner's decision).
- 2026-11-01 — Proof of the port is the golden corpus from the reference implementation (AC-01) plus the ported reference tests (AC-02 – AC-11), because the Vectorizer repository will be deleted.
- 2026-11-01 — Pixel-exact mode belongs in the core, with exact colours (no reduction, alpha as opacity): pixel art loses its look under Potrace smoothing and under colour merging. It refuses more than 256 colours (AC-20): enough for any indexed sprite, and photos belong in colour mode instead of producing hundreds of thousands of layers.
- 2026-11-01 — The pinned palette is ported although the first dialog does not use it: it is small, already tested, and keeps the later dialog step free of core changes.
- 2026-11-01 — Size limit is Pincel's largest canvas side, 32 767 (AC-18): the core refuses nothing a document can hold; downscaling or warning about slow traces is the dialog's call.
- 2026-11-01 — Speed budget: not slower than the reference implementation measured on the same machine in the same run (AC-22), not the Vectorizer's documented numbers from an unknown machine.
- 2026-11-01 — Clarification pass: terms (session, minimum area, segment, default options, test images) defined; tie orders, pinned-palette duplicates, invert on transparency, the despeckle border, overflow-safe size checks and a no-panic criterion (AC-21) added; the reference's random generator fixed for both sides of the corpus.
