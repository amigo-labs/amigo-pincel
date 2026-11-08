---
status: todo
created: 2026-11-08
size: L
mode: Image
design: [pincel.md §2.3, pincel.md §4.5, pincel.md §15 (2026-11-08 OCR as a Rust port of Tesseract's LSTM recognizer)]
depends_on: [ocr-recognizer]
---

# OCR page text (`pincel-ocr`)

## Context

`ocr-recognizer` turns one text line into text. A user's image is a screenshot, a scanned page or a mockup with many lines, columns, buttons and pictures. This spec adds the step in between: from a whole image, find the text lines, recognize each one, and assemble the page text in reading order. It is the second of three OCR specs. The third, `ocr-dialog`, puts it in Image mode.

Tesseract's own page layout analysis (tab stops, column finding, table and image detection) is large. This spec builds a smaller one aimed at what Pincel users read: screenshots and mockups (UI text in light and dark themes, coloured buttons, anti-aliased text) and clean prints or scans. The owner chose this target; photos of text in a scene are not part of it.

Terms used below:

- **Page image**: the RGBA buffer of `width × height × 4` bytes to read, at any size Image mode can open, read with one loaded model.
- **Flattening**: transparent pixels are composited over white or black, whichever contrasts more with the page. That is black when the alpha-weighted mean luminance of the page is above 50 %, white otherwise. This is the same rule as `ocr-recognizer`, applied to the whole page. A pixel counts as transparent in proportion to its alpha.
- **Box**: an axis-aligned rectangle in page-image pixels, with inclusive edges.
- **Line**: text whose characters sit on one baseline. Its box encloses all its words.
- **Text block**: lines that overlap horizontally, with a vertical gap between neighbours of at most one line height. A horizontal gap of at least two x-heights between ink separates blocks on the same row. So a paragraph is one block, a menu is one block with one line per item, and buttons in a row are one block each.
- **Column gap**: a vertical strip at least two x-heights wide, free of ink, that runs the full height of the blocks on both sides of it.
- **Reading order**: the order a recursive cut gives. A region with a column gap is split at it into a left and a right part. Otherwise a region is split at the widest full-width horizontal gap into a top and a bottom part. Parts are read left before right and top before bottom, and a region that cannot be split further is one block.
- **Page text**: the result as one string. Words are separated by one space, lines by a line break, and text blocks by an empty line, in reading order.
- **Page result**: the page text plus every line and word with its box. A line in which the recognizer finds no words is left out.
- **Character accuracy**: computed on Unicode code points after NFC normalization. In both the page text and the reference, every run of whitespace (including line breaks) becomes one space, and leading and trailing whitespace is removed. The value is 1 − (edit distance) / (code-point length of the reference).
- **Page corpus**: committed images with hand-checked reference texts (see Technical notes), read with the model of their language.

## Scope

### In scope

- Reading text in both polarities, also mixed on one page
- Pages skewed by up to ±5°
- Finding lines, text blocks and columns, and the reading order
- Recognizing each line with `ocr-recognizer`, and assembling the page result and the page text
- Progress reporting and cancellation, which `ocr-dialog` needs
- The page corpus and its accuracy test

### Out of scope

- Detecting pages turned by 90° or 180°, and skew beyond ±5° — because screenshots are upright and scans are usually close to straight; the user can rotate the image first. Such pages give garbled or empty text without an error
- Columns whose gap is narrower than two x-heights — because a narrower gap cannot be told apart from a word gap in UI layouts. Such columns are read as one block, line by line across both
- Table structure (cells, rows as a table) — because the result is plain text; cells come out as text blocks
- Handwriting, text along curves, scene text in photos — because the models and the simple layout are not made for them
- Right-to-left and vertical scripts — because v1 ships Latin-script models only (`ocr-recognizer`)
- Choosing the model by the page's language — because the caller picks the language (`ocr-dialog`); a page read with the other language's model still gives text, at lower accuracy
- Running the work in a separate thread — because the crate stays platform-free; `ocr-dialog` runs it off the main thread

## Acceptance criteria

- [ ] AC-01 (event): When a page image is read, then the result holds the page text and every line and word with its box. Every word box lies inside its line's box, and the page text is exactly the result's words joined per the page-text rules.
- [ ] AC-02 (always): On the page corpus, character accuracy is at least 98 % on every page of the screenshot group and at least 95 % on every page of the print group, for `eng` and for `deu`.
- [ ] AC-03 (always): On every corpus page, the blocks of the page text appear in the order of the reference text's blocks. In the test, each reference block is matched to the page-text block with which it has the highest character accuracy, and the matched blocks must keep the reference order.
- [ ] AC-04 (always): The dark-theme screenshot, the mixed-polarity mockup and the transparent page (white text on transparency) reach the accuracy of AC-02.
- [ ] AC-05 (always): The page rotated by 3° reaches the accuracy of AC-02, and its boxes enclose their words in the image as given, not in a straightened copy.
- [ ] AC-06 (event): When the page image has no text, then the result is empty and no error occurs. The no-text images are: a single colour, a 1 × 1 px image, a fully transparent image, and the output of Pincel's Clouds and Add Noise effects (`pincel-effects`) at a fixed seed.
- [ ] AC-07 (error): If the pixel buffer's length is not `width × height × 4`, then reading fails with a "bad image" error. A width or height of 0 gives an empty result.
- [ ] AC-08 (always): The corpus page with x-heights from 6 px (footnote) to 300 px (heading) reaches the accuracy of AC-02. Text below 6 px may be missing or garbled; text of any height produces no error.
- [ ] AC-09 (event): While a page is read, the reader reports progress: during layout analysis at least once per million pixels processed, then "0 of n lines" when the layout is done (also when n is 0), then after each line.
- [ ] AC-10 (event): When the caller answers a progress report with "cancel", then the reader stops before the next line or the next layout step and returns a "cancelled" error, not partial text.
- [ ] AC-11 (always): One loaded model can be used to read several pages at the same time on different threads, and each gives the same result as when read alone.
- [ ] AC-12 (always): An 8000 × 8000 px page with three text lines on white is read without an error and reaches the accuracy of AC-02.
- [ ] AC-13 (always, manual: reference machine): Reading the light 1920 × 1080 screenshot of the corpus in a native release build of `pincel-ocr` takes at most 3 s on the reference machine (pincel.md §4.5). That is about what the Tesseract command line needs for it, and the port should not be slower. The benchmark `ocr-page-screenshot` reports the time.

## Technical notes

- **Page corpus**, per language, made in-house (no third-party image licenses):
  - Screenshot group: light UI screenshot at 1920 × 1080 (about 50 lines); dark UI screenshot; mockup with coloured buttons, a menu and mixed polarity; white text on a transparent background.
  - Print group: a single-column page rendered at 300 dpi with underlined and struck-through words; a two-column page with a gutter of at least three x-heights; the single-column page rotated by 3°; the single-column page with blur and noise (scan-like); a page with a 300 px heading, body text and a 6 px footnote.
- **API sketch**: `read_page(model, rgba, width, height, progress: &mut dyn FnMut(Progress) -> Control) -> Result<PageResult, OcrError>`, where `Control` is `Continue` or `Cancel` and `OcrError` gains `Cancelled`. The final shape is the implementer's choice, but it must be callable from wasm without threads.
- **Suggested approach**: a local adaptive threshold (Sauvola-style) per region of similar background, connected components, a text-height estimate, lines from vertically overlapping components of similar height, a skew estimate from the baselines, and blocks and order from the definitions above. Lines taller than the recognizer handles well are scaled down before recognition.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Underlined or struck-through words | The words are recognized; the lines are not read as characters (single-column corpus page, AC-02) |
| Icons next to labels (UI) | No characters, or stray characters within AC-02's accuracy (screenshot pages) |
| Columns with a gutter of 1–2 x-heights | Read as one block, line by line across both columns (Out of scope) |
| A heading and body text side by side on one row | Separate blocks, read left to right |
| Text on a photo background (banner) | No accuracy guarantee; no error |
| Page turned by 90° | Garbled or empty text, no error (Out of scope) |
| `deu` page read with the `eng` model | Text with `ß` and umlauts misread; no error |
| Cancel during layout of an 8000 × 8000 px page | "cancelled" within one million pixels of work (AC-09, AC-10) |
| Layout finds no lines | Progress reports "0 of 0 lines", then an empty result |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A simplified layout is enough for screenshots and prints | Those are the owner's targets; Tesseract's full layout analysis would be another XL port | Magazine layouts with figures in the text flow come out in an odd order |
| 98 % / 95 % accuracy is reachable without dictionaries | Tesseract's LSTM on clean text usually exceeds 99 %; the gap leaves room for layout mistakes and icons | On the scan-like page, the threshold may need a revision once the corpus exists |
| Flattening per page picks the right background for transparent pixels | Transparent images are usually exported UI elements with one text colour | A transparent image with both light and dark text loses one of them |

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

## Decisions

- 2026-11-08: A smaller layout analysis of its own instead of porting Tesseract's. Screenshots and prints are the target, and porting the full analysis would double the work.
- 2026-11-08 (clarification pass): Transparency follows the contrast rule (owner's choice); the "always white" rule contradicted the dark-UI case. Text block, column gap, reading order, box and character accuracy are now defined terms. The corpus is listed page by page, with two accuracy groups. Cancel is an error kind, and progress also comes during layout. AC IDs were reassigned before any build: AC-11 (concurrency) and AC-12 (large page) are new, and the performance check moved to AC-13.
