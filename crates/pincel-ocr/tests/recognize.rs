//! Recognition behaviour outside the golden comparison
//! (docs/specs/ocr-recognizer.md AC-05, AC-06, AC-07, AC-08, AC-10).

pub mod common;

use common::{DEU, ENG, corpus, load_png};
use pincel_ocr::{Model, RecognizeError, RecognizedLine};

fn recognize(model: &Model, path: &std::path::Path) -> RecognizedLine {
    let line = load_png(path);
    model
        .recognize_line(line.width, line.height, &line.rgba)
        .expect("valid corpus line")
}

#[test]
fn recognize_line_confidences_in_range_and_words_ordered() {
    for (model, lang) in [
        (Model::from_bytes(ENG), "eng"),
        (Model::from_bytes(DEU), "deu"),
    ] {
        let model = model.expect("model loads");
        for path in corpus(lang) {
            let line = recognize(&model, &path);
            assert!(line.words.iter().all(|w| w.confidence <= 100), "{path:?}");
            assert!(
                line.words
                    .windows(2)
                    .all(|p| p[0].bbox.left <= p[1].bbox.left),
                "{path:?}: words not ordered by left edge"
            );
            let joined: Vec<&str> = line.words.iter().map(|w| w.text.as_str()).collect();
            assert_eq!(line.text, joined.join(" "));
        }
    }
}

#[test]
fn recognize_line_empty_or_uniform_returns_no_words() {
    let model = Model::from_bytes(ENG).expect("eng loads");
    let empty = RecognizedLine::default();
    assert_eq!(model.recognize_line(0, 0, &[]), Ok(empty.clone()));
    assert_eq!(model.recognize_line(0, 17, &[]), Ok(empty.clone()));
    assert_eq!(model.recognize_line(23, 0, &[]), Ok(empty.clone()));
    for pixel in [
        [255, 255, 255, 255],
        [0, 0, 0, 255],
        [40, 90, 200, 255],
        [9, 9, 9, 0],
    ] {
        let rgba = pixel.repeat(120 * 30);
        assert_eq!(model.recognize_line(120, 30, &rgba), Ok(empty.clone()));
    }
    // Different colours that flatten to the same one: fully transparent
    // pixels of any colour.
    let mut rgba = [0u8, 0, 0, 0].repeat(64 * 16);
    rgba[4 * 100] = 255;
    assert_eq!(model.recognize_line(64, 16, &rgba), Ok(empty));
}

#[test]
fn recognize_line_beyond_int16_returns_no_words() {
    // Tesseract refuses images wider or higher than 32 767 px.
    let model = Model::from_bytes(ENG).expect("eng loads");
    let width = 32_768u32;
    let rgba: Vec<u8> = (0..width * 2)
        .flat_map(|i| if i % 7 < 3 { [0, 0, 0, 255] } else { [255; 4] })
        .collect();
    assert_eq!(
        model.recognize_line(width, 2, &rgba),
        Ok(RecognizedLine::default())
    );
}

#[test]
fn recognize_line_wrong_buffer_length_returns_bad_image() {
    let model = Model::from_bytes(ENG).expect("eng loads");
    assert_eq!(
        model.recognize_line(10, 10, &[255; 399]),
        Err(RecognizeError::BadImage)
    );
    assert_eq!(
        model.recognize_line(10, 10, &[255; 401]),
        Err(RecognizeError::BadImage)
    );
    // width × height × 4 overflows usize on 32- and 64-bit targets alike.
    assert_eq!(
        model.recognize_line(u32::MAX, u32::MAX, &[]),
        Err(RecognizeError::BadImage)
    );
}

#[test]
fn recognize_line_one_to_seven_pixels_high_returns_without_error() {
    let model = Model::from_bytes(DEU).expect("deu loads");
    let source = load_png(&corpus("deu")[0]);
    for height in 1..=7u32 {
        // A slice of a real line, and a striped pattern.
        let w = source.width;
        let start = ((source.height / 2) * w * 4) as usize;
        let rows = (height.min(source.height) * w * 4) as usize;
        let mut slice = source.rgba[start..start + rows.min(source.rgba.len() - start)].to_vec();
        slice.resize((w * height * 4) as usize, 255);
        assert!(model.recognize_line(w, height, &slice).is_ok());
        let stripes: Vec<u8> = (0..60 * height)
            .flat_map(|i| {
                if (i % 60) % 4 < 2 {
                    [0, 0, 0, 255]
                } else {
                    [255; 4]
                }
            })
            .collect();
        assert!(model.recognize_line(60, height, &stripes).is_ok());
    }
}

#[test]
fn recognize_line_shared_model_on_threads_matches_sequential() {
    let model = Model::from_bytes(DEU).expect("deu loads");
    let paths: Vec<_> = corpus("deu").into_iter().take(8).collect();
    let alone: Vec<RecognizedLine> = paths.iter().map(|p| recognize(&model, p)).collect();
    let together: Vec<RecognizedLine> = std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .iter()
            .map(|p| s.spawn(|| recognize(&model, p)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("thread does not panic"))
            .collect()
    });
    assert_eq!(alone, together);
}
