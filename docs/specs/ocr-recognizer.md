---
status: implemented
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

- **Model**: a `.traineddata` file of the `tessdata_fast` release 4.1.0 (Apache-2.0), the latest release of that repository and the one Tesseract 5 still uses. Its LSTM network has 8-bit integer weights. v1 uses `eng` (English) and `deu` (German).
- **Character set size**: the entry count on the first line of the model's LSTM unicharset component, which includes Tesseract's special entries (space, `Joined`, and so on).
- **Line image**: an RGBA buffer of `width × height × 4` bytes that holds a single line of text, the kind of image `tesseract --psm 7` reads.
- **Flattening**: before anything else, each pixel is composited over a background that is white or black, whichever contrasts more with the line's ink. That is black when the alpha-weighted mean luminance of the line's pixels is above 50 % (light text on transparency) and white otherwise. A fully opaque line image is unchanged by flattening.
- **Recognized line**: the recognizer's result for a line image: the words in order, each with its axis-aligned bounding box in line-image pixels and an integer confidence from 0 to 100, and the line's text, which is its words joined by single spaces.
- **Reference Tesseract**: the Tesseract 5.5 command-line tool (current stable; the patch version is recorded with the golden corpus and changes only by regenerating it), run on the flattened line image with the same model as `tesseract <line.png> - --psm 7 --oem 1 -c lstm_use_matrix=0 -c dotproduct=generic tsv`. `--oem 1` selects the LSTM engine only, `lstm_use_matrix=0` keeps Tesseract from loading the dictionary into the LSTM decoder, and `dotproduct=generic` makes it use its generic arithmetic instead of the CPU's SIMD path. The **reference text** of a line is the text of its TSV word rows (level 5), joined by single spaces.
- **Golden corpus**: committed line images, each with the reference Tesseract's TSV for that image. The tests compare against these files, so CI needs no Tesseract.

## Scope

### In scope

- A new workspace crate `pincel-ocr`: a pure library with no platform dependencies and no file I/O (models and images come in as bytes)
- Reading `tessdata_fast` models: the LSTM network, its unicharset and its recoder
- Recognizing a line image exactly as the reference Tesseract does in single-line mode, without a dictionary: text, word boxes and word confidences
- Flattening transparent line images
- The golden corpus and the script that regenerates its expected output with the reference Tesseract
- The `eng` and `deu` models committed to the repository, as the test input and as the single source the website build serves (`ocr-dialog`)

### Out of scope

- Dictionaries (word lists, DAWGs) in decoding — because the owner chose v1 without them to keep the port to one L spec; the models contain them, so a follow-up spec can add them
- `tessdata_best` models (float weights) — because only the fast models are downloaded (`ocr-dialog`); they are rejected with an error
- Models from the main `tessdata` repository — because nothing ships them; they may load, but no test covers them
- Tesseract's legacy (non-LSTM) engine — because `tessdata_fast` has no legacy data
- Finding lines in a page, and images with several lines — because `ocr-page` owns layout; a multi-line image is read as one line, with whatever text results
- Training or fine-tuning models — because Pincel only recognizes text
- Scripts other than Latin — because v1 ships only `eng` and `deu` and no other model is in the repository to test with

## Acceptance criteria

- [x] AC-01 (event): When the recognizer is given the bytes of the `eng` or `deu` model, then it loads the model and reports its character set size and its network's input height. Both equal what the reference Tesseract's `combine_tessdata -d` and the unicharset show for that file.
- [x] AC-02 (error): If the model bytes are unusable, then loading fails with an error kind the caller can tell apart, and nothing panics. The kinds are: not a `.traineddata` file; truncated (a component's offset or size points past the end; a size field claiming gigabytes gives this error instead of exhausting memory or aborting the process); no LSTM network; unicharset or recoder missing or unreadable; unicharset or recoder not matching the network's output size; a network layer type the crate does not implement; float weights (`tessdata_best`).
- [x] AC-03 (always): For every line of the golden corpus, the recognized line's text equals the line's reference text, character for character.
- [x] AC-04 (always): For every line of the golden corpus, the recognized line has the same number of words as the reference TSV's word rows, with the same text in the same order, and each word's box is within 2 px of the reference box on every side. The 2 px cover the rounding when boxes are scaled back from the network's input height.
- [x] AC-05 (always): Word confidences are integers from 0 to 100, and words are ordered by the left edge of their box.
- [x] AC-06 (event): When a line image has zero width or height, or all its pixels have the same colour after flattening, then the recognized line has no words and empty text, without an error.
- [x] AC-07 (error): If the pixel buffer's length is not `width × height × 4`, or that product overflows the platform's integer size (as it can on wasm32), then recognition fails with a "bad image" error.
- [x] AC-08 (always): Line images 1 to 7 px high return a result without an error, possibly with no words. The golden corpus has lines 8 px and 400 px high, and a line 20 000 px wide made by repeating a corpus line, and these are recognized per AC-03.
- [x] AC-09 (always): A seeded property test finds no input on which loading or recognizing panics or aborts. Its inputs: 1 000 random line images 1–512 px wide and 1–128 px high with random pixels, and 1 000 copies of each committed model with 1–16 random bytes changed.
- [x] AC-10 (always): One loaded model can be used for several recognitions at the same time on different threads, and each gives the same result as when run alone.
- [x] AC-11 (always): A light-on-dark line and a line of white text on a transparent background in the golden corpus are recognized per AC-03. The reference runs on the flattened image.

## Technical notes

- **Format**: a `.traineddata` file is Tesseract's `TessdataManager` archive: an offset table followed by components. The recognizer needs `TESSDATA_LSTM` (the serialized `LSTMRecognizer` with its network), `TESSDATA_LSTM_UNICHARSET` and `TESSDATA_LSTM_RECODER`, and reads `TESSDATA_VERSION` for diagnostics only (an unknown version string is not an error). It ignores the DAWG components.
- **What AC-03 implies**: the port reproduces what `--psm 7` does with a line image before the network (finding the text bounds, cropping, grey normalization to the input height, inverting light-on-dark lines), the network layers the fast models use (input, convolve, max-pool, the LSTM variants, fully connected, softmax), and the dictionary-free recode beam search with its conversion to words, boxes and confidences. The int8 matrix multiply, the activation lookup tables and the rounding follow Tesseract's generic (non-SIMD) path, so results do not depend on the CPU. `ocr-dialog` checks that the wasm build gives the same text.
- **License**: Tesseract and `tessdata_fast` are Apache-2.0. As a derivative work, `pincel-ocr` is licensed Apache-2.0 only, not `MIT OR Apache-2.0`, and keeps Tesseract's copyright and NOTICE (Decision Log 2026-11-08).
- **Dependencies**: no new third-party runtime crate. Decoding the corpus PNGs and the property test use `[dev-dependencies]` the workspace already has.
- **Corpus**: at least 30 lines per language, so that each combination of font (DejaVu Sans, Liberation Serif and Noto Sans — freely licensed, and typical UI and print faces) and x-height (6, 10, 16 and 32 px, ±1 px) appears at least twice. Add crops of Pincel's own UI screenshots, light-on-dark lines, a transparent line, single-character and single-digit lines, and a line with characters outside the `eng` character set (for example `ß` and `€` read with `eng`). The text covers ä ö ü Ä Ö Ü ß, digits, punctuation, € and quotation marks. A script under `crates/pincel-ocr/tests/golden/` renders the lines, runs the reference Tesseract and records its version.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Light text on a dark background | Recognized like dark on light (AC-11) |
| White text on a transparent background | Flattened over black, then recognized (AC-11) |
| A single character or a single digit | Recognized as one word (corpus lines, AC-03) |
| Characters missing from the model's character set (`ß` with `eng`) | The characters the reference Tesseract outputs for them (corpus line, AC-03) |
| Very wide line (20 000 px) | Recognized (AC-08) |
| Line 1–7 px high | A result without an error, usually empty (AC-08) |
| Image with two lines of text | Read as one line; the text is whatever the network makes of it, without an error |
| Model with a corrupt size field that claims gigabytes | "truncated" error before any allocation (AC-02) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Following the generic integer path makes the text identical (AC-03) | Tesseract's fast models are integer networks with lookup-table activations, and the reference runs with `dotproduct=generic`, so both sides use the same arithmetic | A float step in the normalization may round differently; then AC-03 needs a documented tolerance, which is a spec revision |
| `-c lstm_use_matrix=0` is the setting that keeps Tesseract 5.5's LSTM decoder free of dictionaries | With it off, Tesseract loads the LSTM recognizer without its language dictionary | If it does not, the corpus script uses the setting that does and records it; the spec's intent (no dictionary) stays |
| Dropping dictionaries costs little on screenshots and print | Owner's decision; LSTM recognition on clean text depends little on the dictionary | Poor scans lose some accuracy; `ocr-page` measures it on its corpus |
| Word confidences need not match the reference | The dialog shows no confidences in v1 | `ocr-page` may want them for filtering; it would then add a golden check |
| About 6 MB of models in the repository is acceptable | One source for tests and website; tests need no network | Repository size grows once; a model update adds another copy to history |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status   | Commit  |
| ----- | ---- | -------- | ------- |
| AC-01 | `from_bytes_eng_reports_charset_size_and_input_height`, `from_bytes_deu_reports_charset_size_and_input_height` (`crates/pincel-ocr/tests/load.rs`) | verified | b0025d2 |
| AC-02 | `from_bytes_text_returns_not_traineddata`, `from_bytes_cut_short_returns_truncated`, `from_bytes_huge_size_field_returns_truncated`, `from_bytes_without_lstm_returns_no_lstm`, `from_bytes_without_charsets_returns_charset_unreadable`, `from_bytes_charsets_of_other_model_returns_charset_mismatch`, `from_bytes_unknown_layer_returns_unsupported_layer`, `from_bytes_float_weights_returns_float_weights` (`tests/load.rs`) | verified | b0025d2 |
| AC-03 | `recognize_line_deu_corpus_matches_reference`, `recognize_line_eng_corpus_matches_reference` (`crates/pincel-ocr/tests/golden.rs`) | verified | ed397e2 |
| AC-04 | `recognize_line_deu_corpus_matches_reference`, `recognize_line_eng_corpus_matches_reference` (`tests/golden.rs`): word count, text and boxes within 2 px | verified | ed397e2 |
| AC-05 | `recognize_line_confidences_in_range_and_words_ordered` (`tests/recognize.rs`) | verified | b0025d2 |
| AC-06 | `recognize_line_empty_or_uniform_returns_no_words` (`tests/recognize.rs`) | verified | b0025d2 |
| AC-07 | `recognize_line_wrong_buffer_length_returns_bad_image` (`tests/recognize.rs`) | verified | b0025d2 |
| AC-08 | `recognize_line_one_to_seven_pixels_high_returns_without_error` (`tests/recognize.rs`); corpus lines `short_8`, `tall_400`, `wide_20000` in `tests/golden.rs` | verified | ed397e2 |
| AC-09 | `recognize_line_random_images_never_panics`, `from_bytes_corrupted_models_never_panic` (`tests/robustness.rs`) | verified | b0025d2 |
| AC-10 | `recognize_line_shared_model_on_threads_matches_sequential` (`tests/recognize.rs`) | verified | b0025d2 |
| AC-11 | corpus lines `dark_white`, `dark_blue`, `transparent` in `tests/golden.rs` | verified | ed397e2 |

## Summary

> Completed: 2026-11-09

- New workspace crate `pincel-ocr` (Apache-2.0 only, with Tesseract's NOTICE): a pure Rust port of Tesseract's LSTM line recognizer with no I/O and no new runtime dependency; public API `Model` (`from_bytes`, `character_set_size`, `input_height`, `version`, `recognize_line`), `RecognizedLine`, `Word`, `WordBox`, `LoadError`, `RecognizeError`.
- Reads the committed `tessdata_fast` 4.1.0 `eng` and `deu` models and refuses unusable ones with distinct error kinds, without panicking on corrupt data.
- Recognizes one line as Tesseract 5.5 `--psm 7` does without a dictionary — the `--psm 7` page layout, int8 network, activation tables and recode beam search ported exactly — giving text, word boxes and confidences; transparent lines are flattened over white or black first.
- Golden corpus of 71 lines (`eng` 36, `deu` 35) with the reference Tesseract's TSV and the script that regenerates it, plus robustness, threading and edge-size tests.

## Decisions

- 2026-11-08: Port Tesseract's LSTM recognizer instead of using `tesseract.js` or `ocrs`. This was the owner's choice: one Rust engine for every surface, with Tesseract's mature German and English models.
- 2026-11-08: No dictionaries in v1. This was the owner's choice (see Out of scope).
- 2026-11-08 (clarification pass): Transparent pixels are flattened over white or black, whichever contrasts more. This was the owner's choice, because white text on transparency is common in dark UI exports. AC-01 dropped the language code: a model carries none, and only the file name holds it. AC-08 changed from a range claim to concrete corpus lines. AC-07, AC-10 and AC-11 were added. `tessdata` main-repository models moved from "rejected" to "untested". AC IDs were reassigned before any build: the old AC-07 (heights and widths) is now AC-08, the property test is AC-09, and the old AC-09 (same result twice) is covered by AC-10 and by `ocr-dialog` AC-17 (wasm equals native).
- 2026-11-08 (build): The reference is Tesseract 5.5.4 with Leptonica 1.82.0, built from source (`tests/golden/reference.txt`). The corpus PNGs are RGB, never grey or RGBA, so the reference takes the same Leptonica 32-bpp path (opaque alpha byte) that the recognizer follows for its RGBA input; the transparent line is stored as RGBA and flattened by `make_corpus.py` with the recognizer's own integer arithmetic before the reference reads it.
- 2026-11-08 (build): Flattening uses Rec. 601 luma (299/587/114 per mille) for the alpha-weighted mean and rounds the composite as `(c·a + bg·(255 − a) + 127) / 255`. The spec names no formula; this is the common one.
- 2026-11-08 (build): Loading maps a malformed network (impossible sizes, a null char outside the softmax, old float-scale weight format) to `NoLstm`, and a big-endian file to `NotTraineddata`; every published model is little-endian.
- 2026-11-08 (build): The network, the activation tables, the recode beam search, the confidences, the Otsu binarization and Leptonica's scaling are exact ports (network outputs bit-identical with an instrumented Tesseract; the scaling matched Leptonica 1.82 in 119 578 cases). Tesseract's `--psm 7` page layout is ported too (`src/textord/`: outline extraction, blob filtering, straight and spline baselines, x-height, `tospace` word spacing, noise cleanup, word combos, crop and `ReplaceCurrentWord` word boxes), after a first approximation missed 5 of 71 corpus lines on tiny text; the owner chose the exact port. Every intermediate stage matched an instrumented Tesseract on 119 lines. Not ported: fixed-pitch word segmentation (`compute_fixed_pitch`), so monospace rows that Tesseract judges fixed pitch may get different word boxes (1 of 400 random English lines in a check), and the rotation before the underline test, which is exact for unrotated lines.
- 2026-11-08 (build): Images wider or higher than 32 767 px return no words, as Tesseract's thresholder refuses them; the page layout keeps Tesseract's int16 coordinate range.
- 2026-11-08 (build): The property test spreads its 1 000 images over threads (each image has its own seed) to stay inside the test budget (AGENTS.md §7.5); the crate itself stays single-threaded.
- 2026-11-08 (build): New public API: `Model` (`from_bytes`, `character_set_size`, `input_height`, `version`, `recognize_line`), `RecognizedLine`, `Word`, `WordBox`, `LoadError`, `RecognizeError`.
