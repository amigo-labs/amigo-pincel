//! The golden comparison against the reference Tesseract's TSV output
//! (docs/specs/ocr-recognizer.md AC-03, AC-04, AC-08, AC-11).

pub mod common;

use common::{DEU, ENG, corpus, load_png, read_tsv};
use pincel_ocr::Model;

/// Checks every corpus line of a language; returns the mismatches.
fn mismatches(model: &Model, lang: &str) -> Vec<String> {
    let paths = corpus(lang);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut found = Vec::new();
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|start| {
                let paths = &paths;
                s.spawn(move || {
                    let mut bad = Vec::new();
                    for path in paths.iter().skip(start).step_by(threads) {
                        let line = load_png(path);
                        let got = model
                            .recognize_line(line.width, line.height, &line.rgba)
                            .expect("valid corpus line");
                        let want = read_tsv(&path.with_extension("tsv"));
                        let want_text: Vec<&str> = want.iter().map(|w| w.text.as_str()).collect();
                        if got.text != want_text.join(" ") {
                            bad.push(format!(
                                "{path:?}: text {:?} != {:?}",
                                got.text,
                                want_text.join(" ")
                            ));
                            continue;
                        }
                        if got.words.len() != want.len() {
                            bad.push(format!(
                                "{path:?}: {} words != {}",
                                got.words.len(),
                                want.len()
                            ));
                            continue;
                        }
                        for (g, w) in got.words.iter().zip(&want) {
                            let ours = [
                                g.bbox.left as i32,
                                g.bbox.top as i32,
                                (g.bbox.left + g.bbox.width) as i32,
                                (g.bbox.top + g.bbox.height) as i32,
                            ];
                            let theirs = [
                                w.bbox[0],
                                w.bbox[1],
                                w.bbox[0] + w.bbox[2],
                                w.bbox[1] + w.bbox[3],
                            ];
                            if ours.iter().zip(&theirs).any(|(a, b)| (a - b).abs() > 2) {
                                bad.push(format!(
                                    "{path:?}: {:?} box {ours:?} != {theirs:?}",
                                    g.text
                                ));
                            }
                        }
                    }
                    bad
                })
            })
            .collect();
        for h in handles {
            found.extend(h.join().expect("worker thread"));
        }
    });
    found
}

#[test]
fn recognize_line_deu_corpus_matches_reference() {
    let model = Model::from_bytes(DEU).expect("deu loads");
    let bad = mismatches(&model, "deu");
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn recognize_line_eng_corpus_matches_reference() {
    let model = Model::from_bytes(ENG).expect("eng loads");
    let bad = mismatches(&model, "eng");
    assert!(bad.is_empty(), "{bad:#?}");
}
