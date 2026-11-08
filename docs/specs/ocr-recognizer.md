---
status: todo
created: 2026-11-08
size: L
mode: Image
design: [pincel.md §2.1, pincel.md §2.3, pincel.md §15 (2026-11-08 OCR as a Rust port of Tesseract's LSTM recognizer)]
---

# OCR line recognizer (`pincel-ocr`)

## Context

The owner wants text recognition (OCR) in Pincel: read the text in a screenshot, a scan or a mockup and copy it. The owner chose to port Tesseract's recognizer to Rust instead of shipping `tesseract.js` (Tesseract compiled to WASM): one pure Rust library runs the same in the PWA, in the desktop app and, later, in the agent tools and the CLI, and it reads Tesseract's own trained language models (Decision Log 2026-11-08). This follows the Vectorizer precedent, where an existing engine was ported into the workspace crate `pincel-trace`.

OCR is split into three specs:

1. **`ocr-recognizer`** (this spec): the crate `pincel-ocr` loads a Tesseract model and turns **one text line** into text.
2. **`ocr-page`**: finds the text lines in a whole image and assembles the page text.
3. **`ocr-dialog`**: Image mode's Tools ▸ Recognize Text… dialog and the model download.

Terms used below:

- **Model**: a `.traineddata` file of the `tessdata_fast` release 4.1.0 (Apache-2.0). Its LSTM network has 8-bit integer weights. v1 uses `eng` (English) and `deu` (German).
- **Line image**: an RGBA image that contains a single line of text, as `tesseract --psm 7` expects it.
- **Recognized line**: the recognizer's result for a line image: the text, the words with their bounding boxes in line-image pixels, and one confidence per word from 0 to 100.
- **Reference Tesseract**: the Tesseract 5.5 command-line tool, with the exact version recorded next to the golden corpus. It runs with the same model, in single-line mode (`--psm 7`), with the LSTM engine only (`--oem 1`) and with its dictionaries switched off.
- **Golden corpus**: committed line images, each with the reference Tesseract's text and its word boxes (TSV output) for that image. The tests compare against these files, so CI needs no Tesseract.

## Scope

### In scope

- A new workspace crate `pincel-ocr`: a pure library with no platform dependencies and no file I/O (models and images come in as bytes or pixels)
- Reading `tessdata_fast` models: the LSTM network, its character set (unicharset) and its recoder
- The integer LSTM network as Tesseract evaluates it, with the same input normalization to the model's line height
- Dictionary-free beam-search decoding into text, word boxes and word confidences
- The golden corpus (at least 30 English and 30 German lines) and the script that regenerates its expected output with the reference Tesseract
- The `eng` and `deu` models committed to the repository, as the test input and as the single source the website build serves (`ocr-dialog`)

### Out of scope

- Dictionaries (word lists, DAWGs) and the language model in the beam search — because the owner chose v1 without them; they make little difference on screenshots and print, and they would add a second L-sized port. A follow-up spec can add them, since the models already contain them
- `tessdata_best` and `tessdata` (float and legacy models) — because only the fast models are downloaded (`ocr-dialog`); they are rejected with an error
- Tesseract's legacy (non-LSTM) engine — because `tessdata_fast` has no legacy data and the LSTM engine is better
- Finding lines in a page — because `ocr-page` owns layout
- Training or fine-tuning models — because Pincel only recognizes text
- Scripts written right to left or vertically — because v1 ships Latin-script models only; the crate reads any `tessdata_fast` model but the corpus proves only `eng` and `deu`

## Acceptance criteria

- [ ] AC-01 (event): When the recognizer is given the bytes of the `eng` or `deu` model, then it loads the model and reports the language code and the number of characters in its character set; both equal what the model's unicharset lists.
- [ ] AC-02 (error): If the bytes are not a `.traineddata` file, are cut off, have no LSTM component, or hold float (`tessdata_best`) weights, then loading fails with an error that names the reason (not a model, truncated, no LSTM network, unsupported weight format), and nothing panics.
- [ ] AC-03 (always): For every line of the golden corpus, the recognized line's text equals the reference Tesseract's text for that line image, character for character.
- [ ] AC-04 (always): For every line of the golden corpus, the recognized line has the same words as the reference Tesseract's TSV output, and each word's box is within 2 px of the reference box on every side.
- [ ] AC-05 (always): Every word confidence lies between 0 and 100, and word boxes are ordered left to right without overlapping.
- [ ] AC-06 (event): When a line image is empty (zero width or height) or holds no ink (one colour throughout), then the recognized line has no words and empty text, without an error.
- [ ] AC-07 (always): A line image of any height from 8 px to 400 px and any width up to 20 000 px is recognized without an error. The golden corpus contains lines with x-heights of about 6, 10, 16 and 32 px.
- [ ] AC-08 (always): For random line images, and for every model with random bytes changed, loading and recognizing return a result or an error and never panic. A property test generates the inputs.
- [ ] AC-09 (always): Recognizing the same line image with the same model twice gives the same result.

## Technical notes

- **Format**: a `.traineddata` file is Tesseract's `TessdataManager` archive: an offset table followed by components. The recognizer needs `TESSDATA_LSTM` (the serialized `LSTMRecognizer` with its network), `TESSDATA_LSTM_UNICHARSET` and `TESSDATA_LSTM_RECODER`, and reads `TESSDATA_VERSION`. It ignores the DAWG components.
- **Network**: the fast models use a network spec of the form `[1,36,0,1 Ct3,3,16 Mp3,3 Lfys48 Lfx96 Lrx96 Lfx192 O1c…]`. The port covers the layer types those specs use: input, convolve, max-pool, the LSTM variants (forward, reversed, y-summarizing), fully connected and softmax output. The int8 matrix multiply, the activation lookup tables and the rounding follow Tesseract's generic (non-SIMD) code path, so the result does not depend on the CPU.
- **Decoder**: Tesseract's recode beam search without a dictionary, including the conversion of beam output into words, boxes and confidences.
- **License**: Tesseract and `tessdata_fast` are Apache-2.0. As a derivative work, `pincel-ocr` is licensed Apache-2.0 only, not `MIT OR Apache-2.0`, and keeps Tesseract's copyright and NOTICE (Decision Log 2026-11-08).
- **Dependencies**: no new third-party crate is needed; `image` or `png` decoding of the corpus stays in `[dev-dependencies]`, which the workspace already has.
- **Corpus**: lines are rendered from DejaVu Sans, Liberation Serif and Noto Sans at several sizes, plus crops of Pincel's own UI screenshots. The text covers ä ö ü Ä Ö Ü ß, digits, punctuation, € and quotation marks. A script under `crates/pincel-ocr/tests/golden/` regenerates the expected TSV with the reference Tesseract and records its version.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Light text on a dark background | Recognized like dark on light; the line is normalized to dark-on-light before the network, as Tesseract does |
| Line image with a transparent background | Pixels are composited over white before recognition |
| A single character or a single digit | Recognized as one word |
| Characters missing from the model's character set (e.g. CJK in `eng`) | The closest characters the model knows, as the reference Tesseract outputs them; no error |
| Very wide line (20 000 px) | Recognized; memory grows with the width, not with its square |
| Model of a script other than Latin (e.g. `ara` from `tessdata_fast`) | Loads; recognition runs, but its correctness is not part of v1 |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Following the generic integer path closely enough makes the text identical (AC-03) | Tesseract's fast models are integer networks with lookup-table activations, so the arithmetic is deterministic | A float step in the normalization may round differently; then AC-03 needs a documented tolerance, which is a spec revision |
| Word confidences need not match the reference exactly | Tesseract derives them from beam certainties in several steps; the dialog shows no confidences in v1 | `ocr-page` may want them for filtering; it would then add a golden check |
| About 6 MB of models in the repository is acceptable | One source for tests and website; avoids network access in tests | Repository size grows once; a model update adds another copy to history |

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

## Decisions

- 2026-11-08: Port Tesseract's LSTM recognizer instead of using `tesseract.js` or `ocrs` — owner's choice; one Rust engine for every surface, with Tesseract's mature German and English models.
- 2026-11-08: No dictionaries in v1 — owner's choice (see Out of scope).
