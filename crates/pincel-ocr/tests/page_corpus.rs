//! The page corpus (docs/specs/ocr-page.md AC-01 to AC-05, AC-08, AC-14):
//! accuracy, block order, boxes and page-text snapshots of every corpus
//! page, in English and German.
//!
//! `PINCEL_OCR_SNAPSHOTS=update cargo test -p pincel-ocr --test page_corpus`
//! rewrites the snapshots after a deliberate change.

pub mod common;

use common::{DEU, ENG, accuracy, load_png, page_dir};
use pincel_ocr::{Control, Model, PageResult};
use std::sync::OnceLock;

fn model(lang: &str) -> &'static Model {
    static ENG_MODEL: OnceLock<Model> = OnceLock::new();
    static DEU_MODEL: OnceLock<Model> = OnceLock::new();
    let (cell, bytes) = match lang {
        "eng" => (&ENG_MODEL, ENG),
        _ => (&DEU_MODEL, DEU),
    };
    cell.get_or_init(|| Model::from_bytes(bytes).expect("model loads"))
}

fn read(lang: &str, name: &str) -> PageResult {
    let page = load_png(&page_dir(lang).join(format!("{name}.png")));
    model(lang)
        .read_page(page.width, page.height, &page.rgba, &mut |_| {
            Control::Continue
        })
        .expect("corpus page reads")
}

fn reference(lang: &str, name: &str) -> String {
    std::fs::read_to_string(page_dir(lang).join(format!("{name}.txt"))).expect("reference text")
}

/// The accuracy a page must reach (AC-02).
fn required(name: &str) -> f64 {
    if name.starts_with("ui_") { 0.98 } else { 0.95 }
}

/// Checks the AC-01 invariants: boxes nest, and the page text is exactly
/// the blocks, lines and words joined.
fn check_structure(result: &PageResult) -> Result<(), String> {
    let mut blocks = Vec::new();
    for block in &result.blocks {
        let mut lines = Vec::new();
        for line in &block.lines {
            if !block.bbox.contains(&line.bbox) {
                return Err(format!(
                    "line box {:?} outside block {:?}",
                    line.bbox, block.bbox
                ));
            }
            for word in &line.words {
                if !line.bbox.contains(&word.bbox) {
                    return Err(format!("word {:?} outside its line", word.text));
                }
                if word.text.is_empty() || word.text.contains(char::is_whitespace) {
                    return Err(format!("bad word {:?}", word.text));
                }
            }
            if line.words.is_empty() {
                return Err("empty line".into());
            }
            lines.push(
                line.words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        if lines.is_empty() {
            return Err("empty block".into());
        }
        blocks.push(lines.join("\n"));
    }
    if result.text != blocks.join("\n\n") {
        return Err("page text is not the blocks joined".into());
    }
    Ok(())
}

/// AC-03: every reference block matches a different page-text block, in
/// reference order.
fn check_order(text: &str, reference: &str) -> Result<(), String> {
    let ours: Vec<&str> = text.split("\n\n").collect();
    let mut last: Option<usize> = None;
    for block in reference.trim_end().split("\n\n") {
        let scores: Vec<f64> = ours.iter().map(|b| accuracy(b, block)).collect();
        let top = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        // Among equally good blocks (a block text that occurs twice), the
        // first one after the previous match.
        let tied: Vec<usize> = (0..ours.len()).filter(|&k| scores[k] == top).collect();
        let best = tied
            .iter()
            .copied()
            .find(|&k| last.is_none_or(|l| k > l))
            .unwrap_or(tied[0]);
        if last.is_some_and(|l| best <= l) {
            return Err(format!(
                "reference block {block:?} matches page block {best}, after {last:?}"
            ));
        }
        last = Some(best);
    }
    Ok(())
}

/// AC-14: the page text equals the committed snapshot.
fn check_snapshot(lang: &str, name: &str, text: &str) -> Result<(), String> {
    let path = page_dir(lang).join(format!("{name}.snapshot.txt"));
    let current = format!("{text}\n");
    if std::env::var("PINCEL_OCR_SNAPSHOTS").is_ok_and(|v| v == "update") {
        std::fs::write(&path, &current).expect("snapshot written");
        return Ok(());
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    if committed != current {
        return Err(format!(
            "page text differs from {path:?}:\n--- snapshot\n{committed}--- now\n{current}"
        ));
    }
    Ok(())
}

fn check_page(name: &str) {
    let mut failures = Vec::new();
    for lang in ["eng", "deu"] {
        let result = read(lang, name);
        let want = reference(lang, name);
        let acc = accuracy(&result.text, &want);
        println!("{lang}/{name}: accuracy {acc:.4}");
        if acc < required(name) {
            failures.push(format!(
                "{lang}/{name}: accuracy {acc:.4} < {}",
                required(name)
            ));
        }
        let checks = [
            check_structure(&result),
            check_order(&result.text, &want),
            check_snapshot(lang, name, &result.text),
        ];
        for e in checks.into_iter().filter_map(Result::err) {
            failures.push(format!("{lang}/{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn read_page_ui_light_meets_accuracy_and_order() {
    check_page("ui_light");
}

#[test]
fn read_page_ui_dark_meets_accuracy_and_order() {
    check_page("ui_dark");
}

#[test]
fn read_page_ui_mockup_meets_accuracy_and_order() {
    check_page("ui_mockup");
}

#[test]
fn read_page_ui_transparent_meets_accuracy_and_order() {
    check_page("ui_transparent");
}

#[test]
fn read_page_print_single_meets_accuracy_and_order() {
    check_page("print_single");
}

#[test]
fn read_page_print_columns_meets_accuracy_and_order() {
    check_page("print_columns");
}

#[test]
fn read_page_print_scan_meets_accuracy_and_order() {
    check_page("print_scan");
}

#[test]
fn read_page_print_sizes_meets_accuracy_and_order() {
    check_page("print_sizes");
}

#[test]
fn read_page_print_rotated_meets_accuracy_and_order() {
    check_page("print_rotated");
}

/// AC-05: on the page rotated by 3°, each correctly recognized word's box
/// holds the centre of the box the corpus script recorded for that word.
#[test]
fn read_page_rotated_page_word_boxes_hold_recorded_centres() {
    for lang in ["eng", "deu"] {
        let recorded: Vec<(String, f64, f64)> =
            std::fs::read_to_string(page_dir(lang).join("print_rotated.words.tsv"))
                .expect("recorded word boxes")
                .lines()
                .skip(1)
                .map(|l| {
                    let f: Vec<&str> = l.split('\t').collect();
                    let n = |i: usize| f[i].parse::<f64>().expect("number");
                    (f[0].to_owned(), (n(1) + n(3)) / 2.0, (n(2) + n(4)) / 2.0)
                })
                .collect();
        let result = read(lang, "print_rotated");
        let mut checked = 0;
        for word in result
            .blocks
            .iter()
            .flat_map(|b| &b.lines)
            .flat_map(|l| &l.words)
        {
            let b = word.bbox;
            let (cx, cy) = (
                f64::from(b.left + b.right) / 2.0,
                f64::from(b.top + b.bottom) / 2.0,
            );
            // The recorded box of the same word nearest to ours.
            let Some(&(_, rx, ry)) = recorded.iter().filter(|r| r.0 == word.text).min_by(|p, q| {
                let d = |r: &&(String, f64, f64)| (r.1 - cx).powi(2) + (r.2 - cy).powi(2);
                d(p).total_cmp(&d(q))
            }) else {
                continue;
            };
            assert!(
                f64::from(b.left) <= rx
                    && rx <= f64::from(b.right)
                    && f64::from(b.top) <= ry
                    && ry <= f64::from(b.bottom),
                "{lang}: box {b:?} of {:?} misses the recorded centre ({rx}, {ry})",
                word.text
            );
            checked += 1;
        }
        assert!(
            checked * 10 >= recorded.len() * 9,
            "{lang}: only {checked} words checked"
        );
    }
}
