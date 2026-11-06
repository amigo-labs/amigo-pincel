// Unit tests of the text tool (`super`): raster, layout and decorations.
use super::*;
use crate::command::Command;
use crate::geometry::Rect;
use crate::selection::SelectionMask;

/// Liberation Sans (SIL OFL 1.1) — the workspace's one copy of the test
/// font lives with `pincel-core`'s fixtures (LICENSE alongside).
const FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../pincel-core/tests/fixtures/LiberationSans-Regular.ttf"
));

fn style(size: f32) -> TextStyle {
    TextStyle {
        size,
        ..TextStyle::default()
    }
}

fn painted_pixels(doc: &Document) -> usize {
    let mut n = 0;
    for y in 0..doc.canvas.height() {
        for x in 0..doc.canvas.width() {
            if doc.layers[0].pixels.get_pixel(x, y).map(|c| c.a) != Some(0) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn renders_in_its_color_next_to_the_placement_point() {
    let red = Color::rgba(220, 20, 30, 255);
    let mut doc = Document::new(200, 100).unwrap();
    let text_style = TextStyle {
        color: red,
        anti_alias: false,
        ..style(24.0)
    };
    let mut cmd = Text::new("Hi", Point::new(60.0, 30.0), text_style)
        .render(0, &doc, FONT)
        .unwrap();
    cmd.apply(&mut doc).unwrap();

    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..100 {
        for x in 0..200 {
            let px = doc.layers[0].pixels.get_pixel(x, y).unwrap();
            if px.a == 0 {
                continue;
            }
            assert_eq!(px, red, "glyph pixel ({x}, {y}) is in the text color");
            (min_x, min_y) = (min_x.min(x), min_y.min(y));
            (max_x, max_y) = (max_x.max(x), max_y.max(y));
        }
    }
    // The glyphs start just right of / below the point and stay within
    // about one em of it.
    assert!((60..66).contains(&min_x), "left edge at {min_x}");
    assert!((30..42).contains(&min_y), "top edge at {min_y}");
    assert!(
        max_x < 60 + 48 && max_y < 30 + 30,
        "ends at ({max_x}, {max_y})"
    );
}

#[test]
fn renders_non_empty_text() {
    let mut doc = Document::new(200, 80).unwrap();
    let text = Text::new("Hello", Point::new(10.0, 10.0), style(32.0));
    let mut cmd = text.render(0, &doc, FONT).unwrap();
    cmd.apply(&mut doc).unwrap();
    assert!(painted_pixels(&doc) > 0, "expected glyphs to paint pixels");
}

#[test]
fn empty_text_returns_none() {
    let doc = Document::new(100, 40).unwrap();
    let text = Text::new("   ", Point::new(5.0, 5.0), style(20.0));
    // Whitespace produces no outlined glyphs.
    assert!(text.render(0, &doc, FONT).is_none());
}

#[test]
fn invalid_font_returns_none() {
    let doc = Document::new(100, 40).unwrap();
    let text = Text::new("Hi", Point::new(5.0, 5.0), style(20.0));
    assert!(text.render(0, &doc, &[0u8, 1, 2, 3]).is_none());
}

#[test]
fn commit_is_undoable() {
    let mut doc = Document::new(200, 80).unwrap();
    let text = Text::new("Ag", Point::new(10.0, 10.0), style(40.0));
    let mut cmd = text.render(0, &doc, FONT).unwrap();
    cmd.apply(&mut doc).unwrap();
    assert!(painted_pixels(&doc) > 0);
    cmd.revert(&mut doc).unwrap();
    assert_eq!(painted_pixels(&doc), 0, "undo must clear all glyph pixels");
}

#[test]
fn larger_size_paints_more_pixels() {
    let render_count = |size: f32| {
        let mut doc = Document::new(400, 200).unwrap();
        let mut cmd = Text::new("W", Point::new(10.0, 10.0), style(size))
            .render(0, &doc, FONT)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        painted_pixels(&doc)
    };
    assert!(
        render_count(60.0) > render_count(20.0),
        "a larger font size should cover more pixels"
    );
}

#[test]
fn bold_paints_at_least_as_many_pixels_as_regular() {
    let mut regular_doc = Document::new(200, 80).unwrap();
    let mut regular = Text::new("ll", Point::new(10.0, 10.0), style(40.0))
        .render(0, &regular_doc, FONT)
        .unwrap();
    regular.apply(&mut regular_doc).unwrap();

    let mut bold_doc = Document::new(200, 80).unwrap();
    let bold_style = TextStyle {
        bold: true,
        ..style(40.0)
    };
    let mut bold = Text::new("ll", Point::new(10.0, 10.0), bold_style)
        .render(0, &bold_doc, FONT)
        .unwrap();
    bold.apply(&mut bold_doc).unwrap();

    assert!(painted_pixels(&bold_doc) > painted_pixels(&regular_doc));
}

#[test]
fn respects_active_selection() {
    // Select only the left strip; text spanning the canvas paints only there.
    let mut doc = Document::new(200, 60).unwrap();
    doc.selection = Some(SelectionMask::rectangle(200, 60, Rect::new(0, 0, 60, 60)));
    let mut cmd = Text::new("WWWWWWWW", Point::new(2.0, 5.0), style(40.0))
        .render(0, &doc, FONT)
        .unwrap();
    cmd.apply(&mut doc).unwrap();
    // No pixel right of the selection boundary is painted.
    for y in 0..doc.canvas.height() {
        for x in 60..doc.canvas.width() {
            assert_eq!(
                doc.layers[0].pixels.get_pixel(x, y).map(|c| c.a),
                Some(0),
                "pixel ({x},{y}) outside the selection was painted"
            );
        }
    }
}

/// Rows `y` where every pixel in `x0..x1` is painted, and the painted
/// columns of row `y`.
fn painted_row(doc: &Document, y: u32, x0: u32, x1: u32) -> bool {
    (x0..x1).all(|x| doc.layers[0].pixels.get_pixel(x, y).map(|c| c.a) == Some(255))
}

fn render_into(doc: &mut Document, text: &str, at: Point, style: TextStyle) {
    let mut cmd = Text::new(text, at, style).render(0, doc, FONT).unwrap();
    cmd.apply(doc).unwrap();
}

fn decorated(underline: bool, strikethrough: bool) -> TextStyle {
    TextStyle {
        underline,
        strikethrough,
        anti_alias: false,
        ..style(40.0)
    }
}

#[test]
fn underline_runs_below_the_baseline_to_the_last_ink() {
    let mut doc = Document::new(300, 100).unwrap();
    render_into(
        &mut doc,
        "ab   ",
        Point::new(10.0, 10.0),
        decorated(true, false),
    );
    let layout = Text::new("ab   ", Point::new(10.0, 10.0), decorated(true, false))
        .layout_lines(FONT)
        .unwrap();
    let line = &layout.lines[0];
    // A fully painted row a few pixels below the baseline ...
    let base = line.baseline as u32;
    let row = (base + 2..base + 10)
        .find(|&y| painted_row(&doc, y, line.left as u32 + 1, line.ink_end as u32 - 1))
        .expect("an underline row");
    // ... that stops at the last letter, not at the trailing spaces.
    let past = line.ink_end.ceil() as u32 + 2;
    assert_eq!(
        doc.layers[0].pixels.get_pixel(past, row).map(|c| c.a),
        Some(0)
    );
}

#[test]
fn strikethrough_crosses_the_lowercase_letters() {
    let mut plain = Document::new(300, 100).unwrap();
    render_into(
        &mut plain,
        "nn",
        Point::new(10.0, 10.0),
        decorated(false, false),
    );
    let mut struck = Document::new(300, 100).unwrap();
    render_into(
        &mut struck,
        "nn",
        Point::new(10.0, 10.0),
        decorated(false, true),
    );
    let layout = Text::new("nn", Point::new(10.0, 10.0), decorated(false, true))
        .layout_lines(FONT)
        .unwrap();
    let line = &layout.lines[0];
    // Mid x-height: the gap between the two n's is painted only when struck.
    let y = (line.baseline - 0.259 * 40.0) as u32;
    let (x0, x1) = (line.left as u32 + 1, line.ink_end as u32 - 1);
    assert!(painted_row(&struck, y, x0, x1));
    assert!(!painted_row(&plain, y, x0, x1));
}

#[test]
fn strikethrough_stops_at_the_last_ink() {
    let mut doc = Document::new(300, 100).unwrap();
    let s = decorated(false, true);
    render_into(&mut doc, "nn   ", Point::new(10.0, 10.0), s.clone());
    let layout = Text::new("nn   ", Point::new(10.0, 10.0), s)
        .layout_lines(FONT)
        .unwrap();
    let line = &layout.lines[0];
    let y = (line.baseline - STRIKE_CENTER * 40.0) as u32;
    assert!(painted_row(
        &doc,
        y,
        line.left as u32 + 1,
        line.ink_end as u32 - 1
    ));
    // The trailing spaces are not struck through.
    assert!(line.left + line.width > line.ink_end + 4.0);
    let past = line.ink_end.ceil() as u32 + 2;
    assert_eq!(
        doc.layers[0].pixels.get_pixel(past, y).map(|c| c.a),
        Some(0)
    );
}

/// Document rows `[floor(y0), ceil(y1))`.
fn rows(y0: f32, y1: f32) -> std::ops::Range<u32> {
    y0.floor() as u32..y1.ceil() as u32
}

#[test]
fn a_whitespace_only_line_gets_no_decoration() {
    // A text of only whitespace paints nothing at all ...
    let doc = Document::new(100, 40).unwrap();
    let text = Text::new(
        "  
 ",
        Point::new(5.0, 5.0),
        decorated(true, true),
    );
    assert!(text.render(0, &doc, FONT).is_none());

    // ... and between lines with letters a line of spaces stays bare.
    let size = 40.0;
    let at = Point::new(10.0, 4.0);
    let mut doc = Document::new(200, 160).unwrap();
    let s = decorated(true, true);
    render_into(
        &mut doc,
        "ab
   
cd",
        at,
        s.clone(),
    );
    let layout = Text::new(
        "ab
   
cd",
        at,
        s,
    )
    .layout_lines(FONT)
    .unwrap();
    let [first, blank, last] = &layout.lines[..] else {
        panic!("three lines");
    };
    // The lines with letters are underlined (the test is not vacuous).
    for line in [first, last] {
        let base = line.baseline as u32;
        assert!((base + 2..base + 10).any(|y| painted_row(
            &doc,
            y,
            line.left as u32 + 1,
            line.ink_end as u32 - 1
        )));
    }
    assert!(blank.width > 4.0, "the blank line has a width");
    let under_top = blank.baseline + UNDERLINE_TOP * size;
    let under = rows(under_top, under_top + UNDERLINE_THICKNESS * size);
    let strike_c = blank.baseline - STRIKE_CENTER * size;
    let strike_h = STRIKE_THICKNESS * size;
    let strike = rows(strike_c - strike_h / 2.0, strike_c + strike_h / 2.0);
    let xs = blank.left.floor() as u32..(blank.left + blank.width).ceil() as u32;
    for y in under.chain(strike) {
        for x in xs.clone() {
            assert_eq!(
                doc.layers[0].pixels.get_pixel(x, y).map(|c| c.a),
                Some(0),
                "pixel ({x}, {y}) of the blank line is painted"
            );
        }
    }
}

#[test]
fn a_feathered_selection_scales_the_text_alpha_by_its_coverage() {
    let at = Point::new(10.0, 10.0);
    let s = style(40.0);
    let mut full = Document::new(200, 80).unwrap();
    render_into(&mut full, "Hi", at, s.clone());
    let mut half = Document::new(200, 80).unwrap();
    half.selection = SelectionMask::from_raw(200, 80, vec![128; 200 * 80]);
    render_into(&mut half, "Hi", at, s);

    let mut checked = 0;
    for y in 0..80 {
        for x in 0..200 {
            let a = full.layers[0].pixels.get_pixel(x, y).unwrap().a;
            let b = half.layers[0].pixels.get_pixel(x, y).unwrap().a;
            let expected = (a as f32 * 128.0 / 255.0).round() as i32;
            assert!(
                (b as i32 - expected).abs() <= 1,
                "({x}, {y}): alpha {b}, expected about {expected} (unclipped {a})"
            );
            if a == 255 {
                checked += 1;
            }
        }
    }
    assert!(checked > 20, "the text has solid pixels to compare");
}

#[test]
fn decorations_combine_with_bold_italic_and_alignment() {
    for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
        let mut doc = Document::new(300, 120).unwrap();
        let s = TextStyle {
            bold: true,
            italic: true,
            align,
            ..decorated(true, true)
        };
        render_into(&mut doc, "ab\nc", Point::new(150.0, 10.0), s.clone());
        let layout = Text::new("ab\nc", Point::new(150.0, 10.0), s)
            .layout_lines(FONT)
            .unwrap();
        for line in &layout.lines {
            let (x0, x1) = (line.left as u32 + 1, line.ink_end as u32 - 1);
            let base = line.baseline as u32;
            assert!(
                (base + 2..base + 10).any(|y| painted_row(&doc, y, x0, x1)),
                "{align:?}: underline under every line"
            );
            // With both on, the strikethrough is there too.
            let strike = (line.baseline - STRIKE_CENTER * 40.0) as u32;
            assert!(
                painted_row(&doc, strike, x0, x1),
                "{align:?}: strikethrough through every line"
            );
        }
    }
}

#[test]
fn layout_carets_split_a_line_without_changing_its_pixels() {
    let s = TextStyle {
        anti_alias: false,
        ..style(32.0)
    };
    let at = Point::new(12.0, 8.0);
    let mut whole = Document::new(260, 60).unwrap();
    render_into(&mut whole, "HI LT", at, s.clone());
    let layout = Text::new("HI LT", at, s.clone())
        .layout_lines(FONT)
        .unwrap();
    let line = &layout.lines[0];
    assert_eq!(line.carets.len(), 6, "one caret per character boundary");
    for split in [1, 2, 3, 4] {
        let mut parts = Document::new(260, 60).unwrap();
        let (left, right) = "HI LT".split_at(split);
        render_into(&mut parts, left, Point::new(line.left, at.y), s.clone());
        if !right.trim().is_empty() {
            render_into(
                &mut parts,
                right,
                Point::new(line.carets[split], at.y),
                s.clone(),
            );
        }
        assert_eq!(
            parts.layers[0].pixels, whole.layers[0].pixels,
            "split at {split}"
        );
    }
}

#[test]
fn layout_reports_aligned_lines_inside_one_box() {
    let s = TextStyle {
        align: TextAlign::Right,
        ..style(30.0)
    };
    let layout = Text::new("wide line\nab", Point::new(200.0, 5.0), s)
        .layout_lines(FONT)
        .unwrap();
    let [first, second] = &layout.lines[..] else {
        panic!("two lines")
    };
    assert!((first.left + first.width - 200.0).abs() < 1e-3);
    assert!((second.left + second.width - 200.0).abs() < 1e-3);
    assert!((layout.width - first.width).abs() < 1e-3);
    assert!(second.top > first.top);
    assert!((second.top - first.top - layout.line_height).abs() < 1e-3);
}
