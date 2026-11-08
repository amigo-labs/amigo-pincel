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

`ocr-recognizer` turns one text line into text. A user's image is a screenshot, a scanned page or a mockup with many lines, columns, buttons and pictures. This spec adds the step in between: from a whole image, find the text lines in reading order, recognize each line and assemble the page text. It is the second of three OCR specs. The third, `ocr-dialog`, puts it in Image mode.

Tesseract's own page layout analysis (tab stops, column finding, table and image detection) is large. This spec deliberately builds a smaller analysis aimed at what Pincel users read: screenshots and mockups (UI text in light and dark themes, coloured buttons, anti-aliased text) and clean prints or scans. Photos of text in a scene are not a target.

Terms used below:

- **Page image**: the RGBA image to read, at any size Image mode can open.
- **Text block**: a group of lines that belong together, such as a paragraph, a column, a button label or a menu.
- **Page text**: the result as one string. Words are separated by one space, lines by a line break, and text blocks by an empty line. Blocks are in reading order: top to bottom, and in columns that sit side by side, the left column first.
- **Page result**: the page text plus every line and word with its box in page-image pixels.
- **Page corpus**: committed page images, each with a hand-checked reference text. It has at least 8 pages per language (`eng`, `deu`): light and dark UI screenshots, a two-column print, a scan at 300 dpi, a mockup with coloured buttons, and a page rotated by 3°.
- **Character accuracy**: 1 − (edit distance between page text and reference text) / (length of the reference text), compared after collapsing runs of whitespace into one.

## Scope

### In scope

- Turning a page image into a binary image, with the polarity decided per region (dark text on light and light text on dark)
- Correcting skew up to ±5°
- Finding text lines and text blocks, including side-by-side columns separated by a clear vertical gap
- Recognizing each line with `ocr-recognizer`, and assembling the page result and the page text
- Progress reporting and cancellation, which `ocr-dialog` needs
- The page corpus and its accuracy test

### Out of scope

- Detecting pages turned by 90° or 180°, and skew beyond ±5° — because screenshots are upright and scans are usually close to straight; the user can rotate the image first
- Table structure (cells, rows as a table) — because the result is plain text; the cells come out as text blocks
- Handwriting, text along curves, scene text in photos — because the models and the simple layout are not made for them
- Right-to-left and vertical scripts — because v1 ships Latin-script models only (`ocr-recognizer`)
- Running the work in a separate thread — because the crate stays platform-free; `ocr-dialog` runs it in a worker

## Acceptance criteria

- [ ] AC-01 (event): When a page image is read with a loaded model, then the result holds the page text and every line and word with its box in page-image pixels; each word lies inside its line's box.
- [ ] AC-02 (always): On the page corpus, character accuracy is at least 98 % on every screenshot and mockup page and at least 95 % on every scan and print page, for each of `eng` and `deu`.
- [ ] AC-03 (always): On every page of the page corpus, the text blocks appear in the reading order of the reference text: in the two-column print the whole left column comes before the right one, and on a UI screenshot each button label is its own block.
- [ ] AC-04 (always): Light text on a dark background is recognized with the accuracy of AC-02. The corpus has a dark-theme screenshot and a page that mixes both polarities.
- [ ] AC-05 (always): The page rotated by 3° is recognized with the accuracy of AC-02, and its line boxes refer to the image as given, not to a straightened copy.
- [ ] AC-06 (event): When the page image holds no text (a single colour, a photo of a landscape, noise), then the page text is empty and there are no lines, without an error.
- [ ] AC-07 (always): Transparent pixels count as white. A page image of 1 × 1 px, or one that is fully transparent, gives an empty result without an error.
- [ ] AC-08 (always): Text whose x-height lies between 6 px and 300 px is recognized. Lines taller than the recognizer handles are scaled down before recognition, and word boxes still refer to the page image.
- [ ] AC-09 (event): While a page is being read, the reader reports progress as lines done out of lines found, with a report after layout analysis and after each line.
- [ ] AC-10 (event): When the caller cancels, the reader stops after the line in progress and returns a "cancelled" result, not partial text.
- [ ] AC-11 (always, manual: reference machine): Reading the corpus's 1920 × 1080 light screenshot (about 50 lines) in a native release build takes at most 3 s on the reference machine (pincel.md §4.5). The benchmark reports the time.

## Technical notes

- **Binarization**: a local adaptive threshold (Sauvola-style) on luminance, with the polarity decided per connected region from its border.
- **Layout**: connected components → text-height estimate → lines (components that overlap vertically and have similar heights) → text blocks (vertical gaps over about 1.5 line heights, horizontal gutters over about 2 x-heights that span the block). The thresholds are tuned on the page corpus.
- **Skew**: estimated from the line baselines, between −5° and +5°; a line image is cut from a rotated copy, and its boxes are rotated back.
- **API sketch**: `read_page(model, rgba, width, height, progress: &mut dyn FnMut(Progress) -> Control) -> Result<PageResult, OcrError>`, where `Control` is `Continue` or `Cancel`. The final shape is the implementer's choice, but it must stay callable from wasm without threads.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Text on a photo background (e.g. a banner) | Recognized where binarization separates it; no accuracy guarantee beyond the corpus |
| Underlined or struck-through text | Underline and strike line are not read as characters; the words are recognized |
| Icons next to labels (UI) | An icon yields no characters, or a single stray character that the reference text tolerates within AC-02's accuracy |
| Huge image (e.g. 8000 × 8000 px) | Read without error; memory stays proportional to the image |
| A single large heading (x-height 250 px) | Scaled down and recognized (AC-08) |
| Two columns with a very narrow gutter (< 1 x-height) | Read as one block, line by line across both columns; documented limitation |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A simplified layout is enough for screenshots and prints | Those are the owner's targets; Tesseract's full layout analysis would be another XL port | Magazine layouts with figures in the text flow come out in an odd order |
| 98 % / 95 % accuracy is reachable without dictionaries | Tesseract's LSTM on clean text usually exceeds 99 %; the gap covers layout mistakes | On scans, the threshold may need a revision once the corpus exists |
| Page corpus images are made in-house (own screenshots, rendered pages) | No third-party image licenses in the repository | Less variety than real-world scans |

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

## Decisions

- 2026-11-08: Own, smaller layout analysis instead of porting Tesseract's — screenshots and prints are the target, and porting the full analysis would double the work.
