//! Page reading outside the corpus comparison (docs/specs/ocr-page.md
//! AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12).

pub mod common;

use common::{ENG, accuracy, load_png, page_dir};
use pincel_effects::Effect;
use pincel_effects::EffectImage;
use pincel_effects::noise::{AddNoise, NoiseChannels, NoiseType};
use pincel_effects::render::{Clouds, RenderBlend};
use pincel_ocr::{Control, Model, PageError, PageResult, Progress};
use std::sync::OnceLock;

fn eng() -> &'static Model {
    static MODEL: OnceLock<Model> = OnceLock::new();
    MODEL.get_or_init(|| Model::from_bytes(ENG).expect("model loads"))
}

fn read(width: u32, height: u32, rgba: &[u8]) -> Result<PageResult, PageError> {
    eng().read_page(width, height, rgba, &mut |_| Control::Continue)
}

fn solid(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    color.repeat(width as usize * height as usize)
}

fn corpus_page(lang: &str, name: &str) -> common::Line {
    load_png(&page_dir(lang).join(format!("{name}.png")))
}

#[test]
fn read_page_without_text_returns_empty() {
    let white = EffectImage::from_rgba8(512, 512, solid(512, 512, [255; 4])).expect("image");
    let clouds = Clouds::new(
        250.0,
        50.0,
        1.0,
        ([0, 0, 0, 255], [255, 255, 255, 255]),
        RenderBlend::Normal,
    )
    .apply(&white);
    let noise = AddNoise::new(20, NoiseType::Uniform, NoiseChannels::Rgb, 1).apply(&white);
    let cases: Vec<(&str, u32, u32, Vec<u8>)> = vec![
        (
            "single colour",
            300,
            200,
            solid(300, 200, [90, 140, 200, 255]),
        ),
        ("1 x 1", 1, 1, vec![0, 0, 0, 255]),
        ("transparent", 64, 64, solid(64, 64, [0, 0, 0, 0])),
        ("clouds", 512, 512, clouds.into_raw()),
        ("noise", 512, 512, noise.into_raw()),
    ];
    for (name, w, h, rgba) in cases {
        let result = read(w, h, &rgba).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(result.is_empty(), "{name}: found {:?}", result.text);
        assert_eq!(result.text, "", "{name}");
    }
}

#[test]
fn read_page_wrong_buffer_length_returns_bad_image() {
    assert_eq!(read(10, 10, &[0; 399]), Err(PageError::BadImage));
    assert_eq!(read(10, 10, &[0; 401]), Err(PageError::BadImage));
    // width × height × 4 overflows usize.
    assert_eq!(read(u32::MAX, u32::MAX, &[0; 4]), Err(PageError::BadImage));
}

#[test]
fn read_page_beyond_image_mode_limits_returns_too_large() {
    assert_eq!(
        read(16_385, 1, &solid(16_385, 1, [255; 4])),
        Err(PageError::TooLarge)
    );
    assert_eq!(
        read(1, 16_385, &solid(1, 16_385, [255; 4])),
        Err(PageError::TooLarge)
    );
    // 16 384 px on a side is allowed.
    assert!(read(16_384, 1, &solid(16_384, 1, [255; 4])).is_ok());
    // One row more than 8192 × 8192 px of area.
    let n = 8192 * 8193 * 4;
    assert_eq!(read(8192, 8193, &vec![0; n]), Err(PageError::TooLarge));
}

#[test]
fn read_page_zero_width_or_height_returns_empty() {
    assert_eq!(read(0, 0, &[]), Ok(PageResult::default()));
    assert_eq!(read(0, 7, &[]), Ok(PageResult::default()));
    assert_eq!(read(7, 0, &[]), Ok(PageResult::default()));
}

/// Nearest-neighbour rescale of an RGBA image by `num / den`.
fn rescale(page: &common::Line, num: u32, den: u32) -> (u32, u32, Vec<u8>) {
    let (w, h) = (page.width * num / den, page.height * num / den);
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x * den / num, y * den / num);
            let i = ((sy * page.width + sx) * 4) as usize;
            out.extend_from_slice(&page.rgba[i..i + 4]);
        }
    }
    (w, h, out)
}

/// A rectangle of an RGBA image.
fn crop(page: &common::Line, x0: u32, y0: u32, w: u32, h: u32) -> common::Line {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in y0..y0 + h {
        let i = ((y * page.width + x0) * 4) as usize;
        rgba.extend_from_slice(&page.rgba[i..i + (w * 4) as usize]);
    }
    common::Line {
        width: w,
        height: h,
        rgba,
    }
}

#[test]
fn read_page_text_of_any_height_returns_without_error() {
    let page = corpus_page("eng", "ui_light");
    // Text 2 px high and less.
    for den in [3, 5, 8] {
        let (w, h, rgba) = rescale(&page, 1, den);
        read(w, h, &rgba).unwrap_or_else(|e| panic!("1/{den}: {e}"));
    }
    // A heading scaled to letters of about 700 and 1600 px.
    let heading = crop(&page, 320, 70, 230, 40);
    for num in [30, 70] {
        let (w, h, rgba) = rescale(&heading, num, 1);
        read(w, h, &rgba).unwrap_or_else(|e| panic!("x{num}: {e}"));
    }
    // One-pixel strips.
    let row = crop(&page, 0, 145, 1920, 1);
    read(row.width, row.height, &row.rgba).expect("one row");
    let column = crop(&page, 600, 0, 1, 1080);
    read(column.width, column.height, &column.rgba).expect("one column");
}

#[test]
fn read_page_reports_layout_progress_then_each_line() {
    let page = corpus_page("eng", "ui_dark");
    let mut reports = Vec::new();
    let result = eng()
        .read_page(page.width, page.height, &page.rgba, &mut |p| {
            reports.push(p);
            Control::Continue
        })
        .expect("page reads");
    let pixels = u64::from(page.width) * u64::from(page.height);
    let layout: Vec<(u64, u64)> = reports
        .iter()
        .filter_map(|p| match *p {
            Progress::Layout { done, total } => Some((done, total)),
            Progress::Lines { .. } => None,
        })
        .collect();
    let first_line = reports
        .iter()
        .position(|p| matches!(p, Progress::Lines { .. }))
        .expect("line reports");
    assert_eq!(first_line, layout.len(), "layout reports come first");
    let total = layout[0].1;
    assert!(total >= pixels);
    let mut prev = 0;
    for &(done, t) in &layout {
        assert_eq!(t, total);
        assert!(done > prev && done - prev <= 1_000_000, "{prev} -> {done}");
        prev = done;
    }
    assert_eq!(prev, total);
    let lines: Vec<(usize, usize)> = reports[first_line..]
        .iter()
        .map(|p| match *p {
            Progress::Lines { done, total } => (done, total),
            Progress::Layout { .. } => panic!("layout report after line reports"),
        })
        .collect();
    let n = lines[0].1;
    assert!(n >= result.blocks.iter().map(|b| b.lines.len()).sum::<usize>());
    let expected: Vec<(usize, usize)> = (0..=n).map(|k| (k, n)).collect();
    assert_eq!(lines, expected);
}

#[test]
fn read_page_without_lines_reports_zero_of_zero() {
    let rgba = solid(64, 48, [255; 4]);
    let mut reports = Vec::new();
    eng()
        .read_page(64, 48, &rgba, &mut |p| {
            reports.push(p);
            Control::Continue
        })
        .expect("page reads");
    assert_eq!(reports.last(), Some(&Progress::Lines { done: 0, total: 0 }));
}

#[test]
fn read_page_cancel_returns_cancelled_without_further_reports() {
    let page = corpus_page("eng", "ui_mockup");
    // Cancel at the first layout report, at "0 of n lines" and after the
    // second line.
    let stops: [&dyn Fn(&Progress) -> bool; 3] = [
        &|p| matches!(p, Progress::Layout { .. }),
        &|p| matches!(p, Progress::Lines { done: 0, .. }),
        &|p| matches!(p, Progress::Lines { done: 2, .. }),
    ];
    for (k, stop) in stops.iter().enumerate() {
        let mut calls_after = 0;
        let mut cancelled = false;
        let result = eng().read_page(page.width, page.height, &page.rgba, &mut |p| {
            if cancelled {
                calls_after += 1;
            }
            if stop(&p) {
                cancelled = true;
                Control::Cancel
            } else {
                Control::Continue
            }
        });
        assert_eq!(result, Err(PageError::Cancelled), "case {k}");
        assert!(cancelled, "case {k}");
        assert_eq!(calls_after, 0, "case {k}: reports after cancel");
    }
}

#[test]
fn read_page_shared_model_on_threads_matches_alone() {
    let pages: Vec<common::Line> = ["ui_light", "ui_dark", "ui_mockup", "print_columns"]
        .iter()
        .map(|n| corpus_page("eng", n))
        .collect();
    let alone: Vec<PageResult> = pages
        .iter()
        .map(|p| read(p.width, p.height, &p.rgba).expect("page reads"))
        .collect();
    let model = eng();
    std::thread::scope(|s| {
        let handles: Vec<_> = pages
            .iter()
            .map(|p| {
                s.spawn(move || {
                    model
                        .read_page(p.width, p.height, &p.rgba, &mut |_| Control::Continue)
                        .expect("page reads")
                })
            })
            .collect();
        for (h, want) in handles.into_iter().zip(&alone) {
            assert_eq!(&h.join().expect("thread"), want);
        }
    });
}

#[test]
fn read_page_8192_square_page_with_three_lines_reads() {
    const SIDE: u32 = 8192;
    let mut rgba = solid(SIDE, SIDE, [255; 4]);
    let dir = page_dir("large");
    for (k, (x0, y0)) in [(200u32, 300u32), (3000, 4000), (5200, 7900)]
        .iter()
        .enumerate()
    {
        let line = load_png(&dir.join(format!("line_{}.png", k + 1)));
        for y in 0..line.height {
            let src = ((y * line.width) * 4) as usize;
            let dst = (((y0 + y) * SIDE + x0) * 4) as usize;
            rgba[dst..dst + (line.width * 4) as usize]
                .copy_from_slice(&line.rgba[src..src + (line.width * 4) as usize]);
        }
    }
    let result = read(SIDE, SIDE, &rgba).expect("large page reads");
    let want = std::fs::read_to_string(dir.join("lines.txt")).expect("reference");
    let acc = accuracy(&result.text, &want);
    assert!(acc >= 0.98, "accuracy {acc}: {:?}", result.text);
}

/// AC-12 in the wasm32 build: builds `examples/wasm_large_page.rs` for
/// `wasm32-unknown-unknown` (its own target directory, so it does not wait
/// for this test run's lock) and runs its `main` in Node, where a failed
/// check traps.
#[test]
fn read_page_8192_square_page_reads_in_wasm32() {
    let target = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("wasm32");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = std::process::Command::new(cargo)
        .args([
            "build",
            "-p",
            "pincel-ocr",
            "--example",
            "wasm_large_page",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .env("CARGO_TARGET_DIR", &target)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("cargo runs (the wasm32-unknown-unknown target is installed)");
    assert!(status.success(), "wasm32 build failed");
    let wasm = target.join("wasm32-unknown-unknown/debug/examples/wasm_large_page.wasm");
    let script = "const fs = require('fs');\
        const m = new WebAssembly.Module(fs.readFileSync(process.argv[1]));\
        new WebAssembly.Instance(m, {}).exports.main(0, 0);";
    let out = std::process::Command::new("node")
        .args(["-e", script])
        .arg(&wasm)
        .output()
        .expect("node runs");
    assert!(
        out.status.success(),
        "wasm32 large page failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
