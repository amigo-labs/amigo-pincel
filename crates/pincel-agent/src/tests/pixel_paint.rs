//! `draw.stroke`, `draw.shape`, `draw.text` and `draw.gradient` on Pixel
//! documents, per-mode arguments and the selection rules
//! (mcp-parity-drawing AC-01 … AC-24).

use pincel_core::{
    AnyCommand, BrushShape, Bus, Cel, ColorMode, DitherMatrix, DrawDitherGradient, DrawShape,
    DrawText, FrameIndex, GradientKind, Ink, LayerId, Mirror, PencilStroke, PixelBrush,
    PixelBuffer, Point, Rgba, ShapeKind, ShapeMode, ShapeStyle, TextAlign, TextStyle, Wrap,
    layer_canvas,
};
use serde_json::{Value, json};

use super::{S, px};
use crate::list_tools;

const L0: LayerId = LayerId(0);
const F0: FrameIndex = FrameIndex(0);

fn undo_depth(s: &mut S) -> u64 {
    s.ok("doc.info", json!({}))["undo_depth"]
        .as_u64()
        .expect("depth")
}

/// Layer 0 at frame 0 of the document, canvas-sized RGBA.
fn canvas(s: &S, id: &str) -> Vec<u8> {
    let d = s.doc(id);
    layer_canvas(&d.sprite, &d.cels, L0, F0).data
}

/// Layer 0 at frame 0 after the editor runs `cmd` (and continues a
/// stroke through `rest`) on a copy of the document.
fn editor(s: &S, id: &str, cmd: AnyCommand, rest: &[Point]) -> Vec<u8> {
    let (mut sprite, mut cels) = s.state(id);
    if cels.get(L0, F0).is_none() {
        let (w, h) = (sprite.width, sprite.height);
        cels.insert(Cel::image(
            L0,
            F0,
            PixelBuffer::empty(w, h, ColorMode::Rgba),
        ));
    }
    let mut bus = Bus::new();
    bus.execute(cmd, &mut sprite, &mut cels)
        .expect("editor command");
    for &p in rest {
        bus.continue_stroke(p, 1.0, &mut cels).expect("continue");
    }
    layer_canvas(&sprite, &cels, L0, F0).data
}

fn pencil(color: Rgba, size: u8, shape: BrushShape, pp: bool, start: (i32, i32)) -> PencilStroke {
    PencilStroke::new(
        L0,
        F0,
        color,
        PixelBrush::new(size, shape),
        pp,
        Point::new(start.0, start.1),
        1.0,
    )
}

fn pts(list: &[(i32, i32)]) -> Vec<Point> {
    list.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

/// A 16×12 Pixel document with a pattern on layer 0.
fn pixel_doc(s: &mut S) -> String {
    let id = s.new_doc(16, 12);
    s.ok(
        "draw.rect",
        json!({ "x0": 2, "y0": 2, "x1": 9, "y1": 7, "color": { "index": 4 }, "filled": true }),
    );
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 5, "y": 5, "color": { "index": 6 } }, { "x": 6, "y": 5, "color": { "index": 8 } }] }),
    );
    id
}

const BLACK: Rgba = Rgba::new(0, 0, 0, 255);

#[test]
fn drawing_tools_describe_both_modes() {
    // (tool, facts both modes share, Image facts, Pixel facts): every
    // argument of each mode with its range and default, found in the part
    // of the description that belongs to that mode (or the shared start).
    type Facts = (
        &'static str,
        &'static [&'static str],
        &'static [&'static str],
        &'static [&'static str],
    );
    let table: [Facts; 4] = [
        (
            "draw.stroke",
            &[
                "`points`",
                "`tool` brush (default)",
                "`tool: \"eraser\"`",
                "`color` default #000000",
            ],
            &[
                "`size` 1–500 px (default 8)",
                "`hardness` and `opacity` in % (default 100)",
                "`shape` hard_round (default) / soft_round / flat",
                "`angle` 0–180",
                "(default 45)",
                "`eraser_mode` to_transparent (default) / to_background",
                "`background` (default #FFFFFF)",
            ],
            &[
                "integer points",
                "`size` 1–8 (default 1)",
                "`shape` square (default) / round",
                "`pixel_perfect` (default false)",
                "`ink` simple (default) / shading",
                "`ramp_from` / `ramp_to`",
                "default the whole palette",
                "`step` 1 (default) / -1",
                "`mirror` x / y / xy (default none)",
                "`axis_x2` / `axis_y2`",
                "default the centre",
                "`wrap` x / y / xy (default none)",
            ],
        ),
        (
            "draw.shape",
            &["`stroke`", "`fill`"],
            &[
                "`shape` rect, rounded, ellipse or line",
                "(x0, y0) and (x1, y1)",
                "polygon takes the center",
                "`corner_radius` (default 16)",
                "center (`cx`, `cy`)",
                "`radius`",
                "`sides` 3–100 (default 5)",
                "`rotation` in degrees (default 0)",
                "`stroke_width` 1–500 (default 3)",
                "`dash` solid (default) / dashed / dotted",
                "`anti_alias` (default true)",
            ],
            &[
                "`shape` rect, rounded, ellipse or polygon",
                "integer corners (x0, y0)–(x1, y1)",
                "`corner_radius` 0–256 (default 4)",
                "`sides` 3–64 (default 6)",
                "`stroke_width` 1–64 (default 1)",
            ],
        ),
        (
            "draw.text",
            &[
                "`text`",
                "(x, y)",
                "`align` left (default) / center / right",
                "`color` default #000000",
                "`bold` and `italic` (default false)",
            ],
            &[
                "`underline` and `strikethrough` (default false)",
                "`size` 1–999 px (default 48)",
                "`anti_alias` (default true)",
            ],
            &[
                "integer x, y",
                "`size` 1–2000 (default 12)",
                "`anti_alias` (default false)",
            ],
        ),
        (
            "draw.gradient",
            &["(x0, y0) → (x1, y1)", "`from`", "`to`"],
            &[
                "`kind` linear (default) / reflected / radial / conical / diamond",
                "`channel` color (default) / transparency",
                "`opacity` in % (default 100)",
            ],
            &[
                "integer corners",
                "`kind` linear (default) / radial",
                "`dither` 0 (two hard bands), 2, 4 (default) or 8",
            ],
        ),
    ];
    let tools = list_tools();
    for (name, shared, image, pixel) in table {
        assert_eq!(
            tools.iter().filter(|t| t.name == name).count(),
            1,
            "exactly one {name}"
        );
        let d = tools
            .iter()
            .find(|t| t.name == name)
            .expect("listed")
            .description;
        let (i, p) = (
            d.find("Image documents").expect("Image part"),
            d.find("Pixel documents").expect("Pixel part"),
        );
        assert!(i < p, "{name}: the Image part comes first");
        let (start, image_part, pixel_part) = (&d[..i], &d[i..p], &d[p..]);
        for fact in shared {
            assert!(start.contains(fact), "{name}: shared part misses {fact}");
        }
        for fact in image {
            assert!(
                start.contains(fact) || image_part.contains(fact),
                "{name}: Image part misses {fact}"
            );
        }
        for fact in pixel {
            assert!(
                start.contains(fact) || pixel_part.contains(fact),
                "{name}: Pixel part misses {fact}"
            );
        }
        // Every argument of the schema but the document id (which every
        // tool takes) is named in the description.
        let schema = &tools
            .iter()
            .find(|t| t.name == name)
            .expect("listed")
            .input_schema;
        let props = schema["properties"].as_object().expect("properties");
        let words: Vec<&str> = d
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .collect();
        for key in props.keys().filter(|k| *k != "doc") {
            assert!(
                words.contains(&key.as_str()),
                "{name}: description does not name `{key}`"
            );
        }
    }
}

/// The bullet of the guide's "Drawing in each mode" section that starts
/// with `start`, with its line breaks folded into single spaces.
fn drawing_bullet(start: &str) -> String {
    let guide = crate::read_resource("pincel://guide").expect("guide");
    let section = "## Drawing in each mode";
    let body = &guide[guide.find(section).expect(section) + section.len()..];
    let body = &body[..body
        .find(
            "
## ",
        )
        .unwrap_or(body.len())];
    body.split(
        "
- ",
    )
    .map(|b| b.split_whitespace().collect::<Vec<_>>().join(" "))
    .find(|b| b.trim_start_matches("- ").starts_with(start))
    .unwrap_or_else(|| panic!("no bullet starts with {start}"))
}

#[test]
fn guide_lists_each_modes_drawing_arguments_defaults_and_selection_rule() {
    for (start, facts) in [
        (
            "`draw.stroke`:",
            &[
                "`points` (`[[x, y], …]`, one point is one stamp)",
                "`tool` `brush` (default) or `eraser`",
                "integer points",
                "`size` 1–8 (default 1)",
                "`shape` `square` (default) or `round`",
                "`pixel_perfect` (default false)",
                "`ink: \"shading\"` (default `simple`)",
                "`ramp_from` / `ramp_to` (default the whole palette)",
                "`step` 1 (default) / -1",
                "`mirror` `x` / `y` / `xy` (default none)",
                "`axis_x2` / `axis_y2` in half pixels (default the centre)",
                "`wrap` `x` / `y` / `xy` (tiled mode, default none)",
                "`size` 1–500 (default 8)",
                "`hardness` and `opacity` in % (default 100)",
                "`shape` `hard_round` (default), `soft_round` or `flat`",
                "`tool: \"eraser\"`",
                "`color` default `#000000`",
            ][..],
        ),
        (
            "`draw.shape`:",
            &[
                "`stroke`",
                "`fill`",
                "`rect`, `rounded` (`corner_radius` 0–256, default 4), `ellipse` and `polygon` (`sides` 3–64, default 6",
                "integer corners",
                "`stroke_width` 1–64 (default 1)",
                "`line`",
                "`corner_radius` at least 0 (default 16)",
                "`cx` / `cy` / `radius` / `rotation` (default 0)",
                "`sides` 3–100 (default 5)",
                "`dash` `solid` (default)",
                "`anti_alias` (default true)",
                "`stroke_width` 1–500 (default 3)",
            ][..],
        ),
        (
            "`draw.text`:",
            &[
                "`text` at `x`, `y` (the top of the first line)",
                "`color` default `#000000`",
                "`bold` and `italic` (default false)",
                "`align` `left` (default)",
                "Pixel `size` 1–2000 (default 12)",
                "integer `x` / `y`",
                "`anti_alias` default false",
                "Image `size` 1–999 (default 48)",
                "`anti_alias` default true",
                "`underline` and `strikethrough` (default false)",
            ][..],
        ),
        (
            "`draw.gradient`:",
            &[
                "from `x0`, `y0` to `x1`, `y1`, `from` color to `to` color",
                "RGBA only",
                "integer corners",
                "`kind` `linear` (default) / `radial`",
                "`dither` 0, 2, 4 (default) or 8",
                "`kind` `linear` (default), `reflected`, `radial`, `conical` or `diamond`",
                "`channel` `color` (default) or `transparency`",
                "`opacity` in % (default 100)",
            ][..],
        ),
        (
            "The selection:",
            &[
                "in Pixel documents `draw.gradient`, `draw.text` and `draw.fill` stay inside it",
                "while `draw.stroke`, `draw.shape`, `draw.pixels`, `draw.line`, `draw.rect`, `draw.ellipse` and `draw.clear` ignore it",
                "Pixel `draw.fill` clips to the selection although the Pixel editor's Bucket does not",
                "In Image documents every drawing tool stays inside the selection (`draw.stroke`, `draw.shape`, `draw.text`, `draw.fill`, `draw.gradient`, `draw.recolor`, `draw.smudge`, `draw.clone`)",
            ][..],
        ),
    ] {
        let bullet = drawing_bullet(start);
        for fact in facts {
            assert!(bullet.contains(fact), "{start} misses {fact}: {bullet}");
        }
    }
}

#[test]
fn pixel_stroke_equals_the_editors_pencil() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    let path = [(1, 1), (8, 3), (8, 9)];
    let want = editor(
        &s,
        &id,
        pencil(BLACK, 1, BrushShape::Square, false, path[0]).into(),
        &pts(&path[1..]),
    );
    s.ok("draw.stroke", json!({ "points": [[1, 1], [8, 3], [8, 9]] }));
    assert_eq!(canvas(&s, &id), want);

    let red = Rgba::new(255, 0, 0, 255);
    let want = editor(
        &s,
        &id,
        pencil(red, 3, BrushShape::Round, false, (12, 4)).into(),
        &[],
    );
    s.ok(
        "draw.stroke",
        json!({ "points": [[12, 4]], "color": "#ff0000", "size": 3, "shape": "round" }),
    );
    assert_eq!(canvas(&s, &id), want, "one point is one stamp");

    // A palette index paints that entry's color, as `draw.pixels` does.
    let entry = s.doc(&id).sprite.palette.colors[6].rgba;
    let want = editor(
        &s,
        &id,
        pencil(entry, 2, BrushShape::Square, false, (3, 10)).into(),
        &pts(&[(9, 10)]),
    );
    s.ok(
        "draw.stroke",
        json!({ "points": [[3, 10], [9, 10]], "color": { "index": 6 }, "size": 2 }),
    );
    assert_eq!(canvas(&s, &id), want, "palette index");
}

#[test]
fn pixel_perfect_strokes_equal_the_editors() {
    let path = [(0, 0), (1, 0), (1, 1), (2, 1), (2, 2), (5, 4)];
    for size in [1, 3] {
        let mut s = S::new();
        let id = s.new_doc(8, 8);
        let want = editor(
            &s,
            &id,
            pencil(BLACK, size, BrushShape::Square, true, path[0]).into(),
            &pts(&path[1..]),
        );
        let points: Vec<Value> = path.iter().map(|&(x, y)| json!([x, y])).collect();
        s.ok(
            "draw.stroke",
            json!({ "points": points.clone(), "size": size, "pixel_perfect": true }),
        );
        assert_eq!(canvas(&s, &id), want, "size {size}");
        if size > 1 {
            let mut t = S::new();
            let tid = t.new_doc(8, 8);
            t.ok("draw.stroke", json!({ "points": points, "size": size }));
            assert_eq!(
                canvas(&t, &tid),
                want,
                "pixel-perfect is a no-op above size 1"
            );
        }
    }
}

#[test]
fn pixel_eraser_stroke_equals_the_editors_eraser() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    let want = editor(
        &s,
        &id,
        pencil(Rgba::TRANSPARENT, 2, BrushShape::Square, false, (2, 2)).into(),
        &pts(&[(9, 7)]),
    );
    s.ok(
        "draw.stroke",
        json!({ "points": [[2, 2], [9, 7]], "tool": "eraser", "size": 2 }),
    );
    assert_eq!(canvas(&s, &id), want);
    assert_eq!(px(&canvas(&s, &id), 16, 2, 2), [0, 0, 0, 0]);
    // The same shape: a round eraser.
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    let want = editor(
        &s,
        &id,
        pencil(Rgba::TRANSPARENT, 4, BrushShape::Round, false, (6, 5)).into(),
        &[],
    );
    s.ok(
        "draw.stroke",
        json!({ "points": [[6, 5]], "tool": "eraser", "size": 4, "shape": "round" }),
    );
    assert_eq!(canvas(&s, &id), want, "round eraser");
}

#[test]
fn pixel_shading_stroke_equals_the_editors_shading_ink() {
    for (args, from, to, step) in [
        (json!({}), 0usize, 31usize, 1i8),
        // The default ramp is the whole palette in both directions: the
        // first and the last entry lie on the path.
        (json!({ "step": -1 }), 0, 31, -1),
        (
            json!({ "ramp_from": 3, "ramp_to": 7, "step": -1 }),
            3,
            7,
            -1,
        ),
    ] {
        let mut s = S::new();
        let id = pixel_doc(&mut s);
        assert_eq!(s.doc(&id).sprite.palette.colors.len(), 32);
        s.ok(
            "draw.pixels",
            json!({ "pixels": [{ "x": 2, "y": 5, "color": { "index": 0 } }, { "x": 3, "y": 5, "color": { "index": 31 } }] }),
        );
        let ramp: Vec<Rgba> = s.doc(&id).sprite.palette.colors[from..=to]
            .iter()
            .map(|e| e.rgba)
            .collect();
        let cmd = pencil(Rgba::TRANSPARENT, 1, BrushShape::Square, false, (1, 5))
            .with_ink(Ink::Shading { ramp, step });
        let want = editor(&s, &id, cmd.into(), &pts(&[(10, 5), (1, 5)]));
        let mut a = args.clone();
        a["points"] = json!([[1, 5], [10, 5], [1, 5]]);
        a["ink"] = json!("shading");
        s.ok("draw.stroke", a);
        assert_eq!(canvas(&s, &id), want, "{args}");
    }
}

#[test]
fn bad_shading_arguments_fail_naming_the_argument() {
    let mut s = S::new();
    let id = pixel_doc(&mut s);
    for (args, needle) in [
        (json!({ "ramp_from": 40, "ramp_to": 41 }), "`ramp_from`"),
        (json!({ "ramp_from": 5, "ramp_to": 2 }), "`ramp_from`"),
        (json!({ "ramp_from": 5 }), "`ramp_to`"),
        (json!({ "ramp_to": 5 }), "`ramp_from`"),
        (json!({ "ramp_from": 0, "ramp_to": 32 }), "`ramp_to`"),
        (json!({ "step": 2 }), "`step`"),
        (json!({ "tool": "eraser" }), "`tool`"),
    ] {
        let mut a = args.clone();
        a["points"] = json!([[1, 1]]);
        a["ink"] = json!("shading");
        let before = s.state(&id);
        let e = s.err("draw.stroke", a);
        assert!(e.contains(needle), "{args}: {e}");
        assert_eq!(s.state(&id), before);
    }
    s.ws.get_mut(&id).expect("doc").sprite.palette = pincel_core::Palette::from_entries(Vec::new());
    let before = s.state(&id);
    let e = s.err(
        "draw.stroke",
        json!({ "points": [[1, 1]], "ink": "shading" }),
    );
    assert!(e.contains("palette is empty"), "{e}");
    assert_eq!(s.state(&id), before);
}

#[test]
fn pixel_mirror_and_wrap_equal_the_editors_symmetry_and_tiled_mode() {
    let mut s = S::new();
    let id = s.new_doc(16, 12);
    let cmd = pencil(BLACK, 1, BrushShape::Square, false, (2, 1)).with_mirror(Mirror {
        axis_x2: Some(16),
        axis_y2: Some(9),
    });
    let want = editor(&s, &id, cmd.into(), &pts(&[(5, 3)]));
    s.ok(
        "draw.stroke",
        json!({ "points": [[2, 1], [5, 3]], "mirror": "xy", "axis_y2": 9 }),
    );
    assert_eq!(canvas(&s, &id), want);
    // Each axis alone at its default (the centre), a mirror image beyond
    // the canvas skipped without `wrap`, and wrapping along y alone.
    for (args, mirror, wrap) in [
        (
            json!({ "mirror": "x" }),
            Some(Mirror {
                axis_x2: Some(16),
                axis_y2: None,
            }),
            Wrap { x: false, y: false },
        ),
        (
            json!({ "mirror": "y" }),
            Some(Mirror {
                axis_x2: None,
                axis_y2: Some(12),
            }),
            Wrap { x: false, y: false },
        ),
        (
            json!({ "mirror": "x", "axis_x2": 3 }),
            Some(Mirror {
                axis_x2: Some(3),
                axis_y2: None,
            }),
            Wrap { x: false, y: false },
        ),
        (json!({ "wrap": "y" }), None, Wrap { x: false, y: true }),
    ] {
        let mut s = S::new();
        let id = s.new_doc(16, 12);
        let mut cmd = pencil(BLACK, 1, BrushShape::Square, false, (2, 1)).with_wrap(wrap);
        if let Some(m) = mirror {
            cmd = cmd.with_mirror(m);
        }
        let want = editor(&s, &id, cmd.into(), &pts(&[(5, 14)]));
        let mut a = args.clone();
        a["points"] = json!([[2, 1], [5, 14]]);
        s.ok("draw.stroke", a);
        assert_eq!(canvas(&s, &id), want, "{args}");
    }

    // AC-08's example: a 64-px-wide canvas, (60, 0)–(70, 0), wrap x.
    let mut s = S::new();
    let id = s.new_doc(64, 4);
    s.ok(
        "draw.stroke",
        json!({ "points": [[60, 0], [70, 0]], "wrap": "x" }),
    );
    let c = canvas(&s, &id);
    let painted: Vec<u32> = (0..64).filter(|&x| px(&c, 64, x, 0)[3] == 255).collect();
    let want: Vec<u32> = (0..=6).chain(60..=63).collect();
    assert_eq!(painted, want);
    // Mirror and wrap together equal the editor's stroke with both aids.
    let mut s = S::new();
    let id = s.new_doc(16, 8);
    let cmd = pencil(BLACK, 2, BrushShape::Square, false, (14, 2))
        .with_mirror(Mirror {
            axis_x2: Some(4),
            axis_y2: None,
        })
        .with_wrap(Wrap { x: true, y: true });
    let want = editor(&s, &id, cmd.into(), &pts(&[(17, 9)]));
    s.ok(
        "draw.stroke",
        json!({ "points": [[14, 2], [17, 9]], "size": 2, "mirror": "x", "axis_x2": 4, "wrap": "xy" }),
    );
    assert_eq!(canvas(&s, &id), want);
}

#[test]
fn bad_axes_and_fractional_pixel_coordinates_fail_naming_the_argument() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    for (tool, args, needle) in [
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "mirror": "x", "axis_x2": 17 }),
            "`axis_x2`",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "mirror": "x", "axis_y2": 4 }),
            "`axis_y2`",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "mirror": "y", "axis_x2": 4 }),
            "`axis_x2`",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "mirror": "xy", "axis_y2": 17 }),
            "`axis_y2`",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "mirror": "x", "axis_x2": -1 }),
            "`axis_x2`",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1], [2, 1], [3, 1.5]] }),
            "points[2]",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1], [2.5, 1]] }),
            "points[1]",
        ),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0.5, "x1": 3, "y1": 3, "fill": "#000000" }),
            "`y0`",
        ),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3.5, "y1": 0, "from": "#000000", "to": "#ffffff" }),
            "`x1`",
        ),
        ("draw.text", json!({ "text": "A", "x": 0.5, "y": 0 }), "`x`"),
        ("draw.text", json!({ "text": "A", "x": 0, "y": 0.5 }), "`y`"),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(e.contains(needle), "{tool} {args}: {e}");
        assert_eq!(s.state(&id), before);
    }
    // Every corner of the Pixel shape and gradient.
    for key in ["x0", "y0", "x1", "y1"] {
        for (tool, mut args) in [
            (
                "draw.shape",
                json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000" }),
            ),
            (
                "draw.gradient",
                json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 2, "from": "#000000", "to": "#ffffff" }),
            ),
        ] {
            args[key] = json!(1.5);
            let before = s.state(&id);
            let e = s.err(tool, args.clone());
            assert!(e.contains(&format!("`{key}`")), "{tool} {args}: {e}");
            assert_eq!(s.state(&id), before);
        }
    }
    // The axes reach from the canvas edge (0) to twice its side.
    for args in [
        json!({ "mirror": "x", "axis_x2": 0 }),
        json!({ "mirror": "x", "axis_x2": 16 }),
        json!({ "mirror": "y", "axis_y2": 0 }),
        json!({ "mirror": "y", "axis_y2": 16 }),
    ] {
        let mut a = args.clone();
        a["points"] = json!([[1, 1]]);
        s.ok("draw.stroke", a);
    }
    // Image documents keep fractional coordinates.
    s.new_image(8, 8);
    s.ok(
        "draw.stroke",
        json!({ "points": [[1.5, 1.5], [6.25, 2.5]] }),
    );
    s.ok(
        "draw.text",
        json!({ "text": "A", "x": 0.5, "y": 0.5, "size": 6 }),
    );
    s.ok(
        "draw.shape",
        json!({ "shape": "rect", "x0": 0.5, "y0": 0.5, "x1": 5.5, "y1": 5.5, "fill": "#000000" }),
    );
    s.ok(
        "draw.gradient",
        json!({ "x0": 0.5, "y0": 0, "x1": 6.5, "y1": 0.5, "from": "#000000", "to": "#ffffff" }),
    );
}

#[test]
fn pixel_shapes_equal_the_editors_shape_tools() {
    let blue = Rgba::new(0, 0, 255, 255);
    let red = Rgba::new(255, 0, 0, 255);
    for (args, kind, mode, width) in [
        (
            json!({ "shape": "rect", "stroke": "#0000ff" }),
            ShapeKind::Rectangle,
            ShapeMode::Outline,
            1,
        ),
        (
            json!({ "shape": "rounded", "fill": "#ff0000", "corner_radius": 3 }),
            ShapeKind::RoundedRectangle { radius: 3.0 },
            ShapeMode::Fill,
            1,
        ),
        (
            json!({ "shape": "rounded", "stroke": "#0000ff" }),
            ShapeKind::RoundedRectangle { radius: 4.0 },
            ShapeMode::Outline,
            1,
        ),
        (
            json!({ "shape": "ellipse", "stroke": "#0000ff", "fill": "#ff0000", "stroke_width": 2 }),
            ShapeKind::Ellipse,
            ShapeMode::FillOutline,
            2,
        ),
        (
            json!({ "shape": "polygon", "stroke": "#0000ff" }),
            ShapeKind::Polygon {
                sides: 6,
                rotation_deg: 0.0,
            },
            ShapeMode::Outline,
            1,
        ),
        (
            json!({ "shape": "polygon", "fill": "#ff0000", "sides": 5 }),
            ShapeKind::Polygon {
                sides: 5,
                rotation_deg: 0.0,
            },
            ShapeMode::Fill,
            1,
        ),
    ] {
        let mut s = S::new();
        let id = pixel_doc(&mut s);
        let style = ShapeStyle {
            mode,
            stroke_width: width,
            stroke: if args.get("stroke").is_some() {
                blue
            } else {
                Rgba::TRANSPARENT
            },
            fill: if args.get("fill").is_some() {
                red
            } else {
                Rgba::TRANSPARENT
            },
        };
        let want = editor(
            &s,
            &id,
            DrawShape::new(L0, F0, (13, 10), (3, 1), kind, style).into(),
            &[],
        );
        let mut a = args.clone();
        for (k, v) in [("x0", 13), ("y0", 10), ("x1", 3), ("y1", 1)] {
            a[k] = json!(v);
        }
        s.ok("draw.shape", a);
        assert_eq!(canvas(&s, &id), want, "{args}");
    }
}

#[test]
fn pixel_shape_line_and_polygon_center_point_elsewhere() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    for (args, needle, arg) in [
        (
            json!({ "shape": "line", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "stroke": "#000000" }),
            "draw.line",
            "shape",
        ),
        (
            json!({ "shape": "polygon", "cx": 4, "cy": 4, "radius": 3, "stroke": "#000000" }),
            "(x0, y0)–(x1, y1)",
            "cx",
        ),
        (
            json!({ "shape": "polygon", "x0": 0, "y0": 0, "x1": 5, "y1": 5, "cy": 4, "stroke": "#000000" }),
            "(x0, y0)–(x1, y1)",
            "cy",
        ),
        (
            json!({ "shape": "polygon", "x0": 0, "y0": 0, "x1": 5, "y1": 5, "radius": 3, "stroke": "#000000" }),
            "(x0, y0)–(x1, y1)",
            "radius",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err("draw.shape", args.clone());
        assert!(
            e.contains(needle) && e.contains(&format!("`{arg}`")),
            "{args}: {e}"
        );
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn pixel_gradient_equals_the_editors_dithered_gradient() {
    for (args, kind, n) in [
        (json!({}), GradientKind::Linear, 4),
        (
            json!({ "kind": "radial", "dither": 8 }),
            GradientKind::Radial,
            8,
        ),
        (json!({ "dither": 0 }), GradientKind::Linear, 0),
        (json!({ "dither": 2 }), GradientKind::Linear, 2),
    ] {
        let mut s = S::new();
        let id = pixel_doc(&mut s);
        let colors = (Rgba::new(255, 0, 0, 255), Rgba::new(0, 0, 255, 255));
        let cmd = DrawDitherGradient::new(
            L0,
            F0,
            (1, 2),
            (14, 9),
            kind,
            DitherMatrix::from_size(n).expect("m"),
            colors,
        )
        .expect("gradient");
        let want = editor(&s, &id, cmd.into(), &[]);
        let mut a = args.clone();
        for (k, v) in [("x0", 1), ("y0", 2), ("x1", 14), ("y1", 9)] {
            a[k] = json!(v);
        }
        a["from"] = json!("#ff0000");
        a["to"] = json!("#0000ff");
        s.ok("draw.gradient", a);
        assert_eq!(canvas(&s, &id), want, "{args}");
    }
}

#[test]
fn pixel_gradient_refuses_indexed_and_grayscale_documents() {
    for mode in [
        ColorMode::Indexed {
            transparent_index: 0,
        },
        ColorMode::Grayscale,
    ] {
        let mut s = S::new();
        let id = s.new_doc(8, 8);
        s.ws.get_mut(&id).expect("doc").sprite.color_mode = mode;
        let before = s.state(&id);
        let e = s.err(
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 7, "y1": 0, "from": "#000000", "to": "#ffffff" }),
        );
        assert!(e.contains("RGBA documents only"), "{e}");
        assert_eq!(s.state(&id), before);
    }
}

#[test]
fn pixel_text_equals_the_editors_text_tool() {
    for (args, size, aa, bold, align) in [
        (json!({}), 12.0, false, false, TextAlign::Left),
        (
            json!({ "size": 9, "anti_alias": true, "bold": true, "align": "center" }),
            9.0,
            true,
            true,
            TextAlign::Center,
        ),
        (
            json!({ "size": 10, "italic": true, "align": "right", "color": "#ff0000" }),
            10.0,
            false,
            false,
            TextAlign::Right,
        ),
    ] {
        let mut s = S::new();
        let id = s.new_doc(40, 20);
        let italic = args.get("italic").is_some();
        let color = if args.get("color").is_some() {
            Rgba::new(255, 0, 0, 255)
        } else {
            BLACK
        };
        let style = TextStyle {
            size,
            color,
            bold,
            italic,
            anti_alias: aa,
            align,
        };
        let cmd = DrawText::new(L0, F0, (10, 2), "Hi", crate::image::FONT.to_vec(), style);
        let want = editor(&s, &id, cmd.into(), &[]);
        let mut a = args.clone();
        a["text"] = json!("Hi");
        a["x"] = json!(10);
        a["y"] = json!(2);
        let out = s.ok("draw.text", a);
        assert_eq!(out["changed"], true);
        assert_eq!(canvas(&s, &id), want, "{args}");
    }
}

#[test]
fn text_that_lands_nowhere_and_zero_length_gradients_change_nothing() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    s.ok(
        "draw.pixels",
        json!({ "pixels": [{ "x": 1, "y": 1, "color": "#ff0000" }] }),
    );
    let depth = undo_depth(&mut s);
    let before = s.state(&id);
    for (tool, args) in [
        ("draw.text", json!({ "text": "Hi", "x": 100, "y": 100 })),
        ("draw.text", json!({ "text": "", "x": 0, "y": 0 })),
        (
            "draw.gradient",
            json!({ "x0": 3, "y0": 3, "x1": 3, "y1": 3, "from": "#000000", "to": "#ffffff" }),
        ),
    ] {
        let out = s.ok(tool, args);
        assert_eq!(out["changed"], false, "{tool}");
        assert_eq!(undo_depth(&mut s), depth, "{tool}");
        assert_eq!(s.state(&id), before, "{tool}");
    }
}

#[test]
fn pixel_selection_clips_gradient_text_and_fill_only() {
    let calls: Vec<(&str, Value)> = vec![
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 15, "y1": 0, "from": "#ff0000", "to": "#0000ff" }),
        ),
        (
            "draw.text",
            json!({ "text": "WW", "x": 0, "y": 0, "size": 12 }),
        ),
        ("draw.fill", json!({ "x": 0, "y": 0, "color": "#00ff00" })),
        (
            "draw.stroke",
            json!({ "points": [[0, 0], [15, 11]], "size": 2 }),
        ),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 15, "y1": 11, "fill": "#ff0000" }),
        ),
        (
            "draw.pixels",
            json!({ "pixels": [{ "x": 0, "y": 0, "color": "#ff0000" }, { "x": 15, "y": 11, "color": "#ff0000" }] }),
        ),
        (
            "draw.line",
            json!({ "x0": 0, "y0": 11, "x1": 15, "y1": 0, "color": "#ff0000" }),
        ),
        (
            "draw.rect",
            json!({ "x0": 0, "y0": 0, "x1": 15, "y1": 11, "color": "#ff0000", "filled": true }),
        ),
        (
            "draw.ellipse",
            json!({ "x0": 0, "y0": 0, "x1": 15, "y1": 11, "color": "#ff0000", "filled": true }),
        ),
        ("draw.clear", json!({})),
    ];
    for (tool, args) in calls {
        let clips = matches!(tool, "draw.gradient" | "draw.text" | "draw.fill");
        // Without a selection.
        let mut free = S::new();
        let fid = pixel_doc(&mut free);
        free.ok(tool, args.clone());
        // With a selection.
        let mut s = S::new();
        let id = pixel_doc(&mut s);
        let before = canvas(&s, &id);
        s.ok(
            "select.rect",
            json!({ "x": 4, "y": 0, "width": 5, "height": 12 }),
        );
        s.ok(tool, args.clone());
        let after = canvas(&s, &id);
        if clips {
            for y in 0..12 {
                for x in (0..4).chain(9..16) {
                    assert_eq!(
                        px(&after, 16, x, y),
                        px(&before, 16, x, y),
                        "{tool} changed ({x}, {y})"
                    );
                }
            }
            assert_ne!(after, before, "{tool} changed nothing");
        } else {
            assert_eq!(after, canvas(&free, &fid), "{tool} ignores the selection");
        }
    }
}

#[test]
fn image_selection_clips_every_drawing_tool() {
    for (tool, args) in [
        (
            "draw.stroke",
            json!({ "points": [[0, 4], [15, 4]], "size": 6 }),
        ),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 15, "y1": 7, "fill": "#ff0000" }),
        ),
        (
            "draw.text",
            json!({ "text": "WWWW", "x": 0, "y": 0, "size": 8 }),
        ),
        ("draw.fill", json!({ "x": 0, "y": 0, "color": "#00ff00" })),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 15, "y1": 0, "from": "#ff0000", "to": "#0000ff" }),
        ),
    ] {
        let mut s = S::new();
        let id = s.new_image(16, 8);
        s.ok(
            "select.rect",
            json!({ "x": 4, "y": 0, "width": 6, "height": 8, "feather": 1 }),
        );
        s.ok(tool, args.clone());
        let d = s.image(&id).document();
        let sel = d.selection.clone().expect("selection");
        let layer = &d.layers()[0].pixels;
        let mut inside = false;
        for y in 0..8 {
            for x in 0..16 {
                let a = layer.get_pixel(x, y).expect("pixel").a;
                if sel.coverage(x, y) == 0 {
                    assert_eq!(a, 0, "{tool} painted outside at ({x}, {y})");
                } else if a > 0 {
                    inside = true;
                }
            }
        }
        assert!(inside, "{tool} painted nothing inside");
    }
}

/// Layer 0 of a fresh 16×8 Image document after `tool` runs with the
/// rectangle x 4–9 selected and feathered by 2 px, or (`hard`) with every
/// pixel that feathered selection touches fully selected. Returns the
/// feathered coverage per pixel and the layer's RGBA.
fn image_drawn_in_selection(tool: &str, args: &Value, hard: bool) -> (Vec<u8>, Vec<u8>) {
    let mut s = S::new();
    let id = s.new_image(16, 8);
    s.ok(
        "select.rect",
        json!({ "x": 4, "y": 0, "width": 6, "height": 8, "feather": 2 }),
    );
    let soft = s
        .image(&id)
        .document()
        .selection
        .clone()
        .expect("selection");
    let cov = soft.data().to_vec();
    if hard {
        let b = soft.bounding_box().expect("bounds");
        s.ok(
            "select.rect",
            json!({ "x": b.x, "y": b.y, "width": b.w, "height": b.h }),
        );
        let mask = s.image(&id).document().selection.clone().expect("mask");
        let full: Vec<u8> = cov.iter().map(|&c| if c > 0 { 255 } else { 0 }).collect();
        assert_eq!(mask.data(), full.as_slice(), "hard mask");
    }
    s.ok(tool, args.clone());
    let layer = s.image(&id).document().layers()[0].pixels.data().to_vec();
    (cov, layer)
}

#[test]
fn image_feathered_selection_blends_every_drawing_tool_by_coverage() {
    for (tool, args) in [
        (
            "draw.stroke",
            json!({ "points": [[0, 4], [15, 4]], "size": 6 }),
        ),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 15, "y1": 7, "fill": "#ff0000" }),
        ),
        (
            "draw.text",
            json!({ "text": "WWWW", "x": 0, "y": 0, "size": 8 }),
        ),
        ("draw.fill", json!({ "x": 0, "y": 0, "color": "#00ff00" })),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 15, "y1": 0, "from": "#ff0000", "to": "#0000ff" }),
        ),
    ] {
        let (cov, soft) = image_drawn_in_selection(tool, &args, false);
        let (_, hard) = image_drawn_in_selection(tool, &args, true);
        assert!(cov.iter().any(|&c| c > 0 && c < 255), "feathered edge");
        let mut blended = false;
        for (i, &c) in cov.iter().enumerate() {
            let (px_soft, px_hard) = (&soft[i * 4..i * 4 + 4], &hard[i * 4..i * 4 + 4]);
            match c {
                0 => assert_eq!(px_soft[3], 0, "{tool} painted outside at {i}"),
                255 => assert_eq!(px_soft, px_hard, "{tool} fully selected pixel {i}"),
                _ => {
                    assert!(
                        px_soft[3] <= px_hard[3],
                        "{tool} pixel {i} above its coverage"
                    );
                    blended |= px_soft[3] > 0 && px_soft[3] < px_hard[3];
                }
            }
        }
        assert!(
            blended,
            "{tool}: no feathered pixel was blended by its coverage"
        );
    }
}

#[test]
fn arguments_of_the_other_modes_editor_fail_naming_argument_and_mode() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    for (tool, args, arg) in [
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "opacity": 50 }),
            "opacity",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "hardness": 50 }),
            "hardness",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "shape": "soft_round" }),
            "shape",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "shape": "hard_round" }),
            "shape",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "shape": "flat" }),
            "shape",
        ),
        (
            "draw.shape",
            json!({ "shape": "polygon", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000", "cx": 2 }),
            "cx",
        ),
        (
            "draw.shape",
            json!({ "shape": "polygon", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000", "cy": 2 }),
            "cy",
        ),
        (
            "draw.shape",
            json!({ "shape": "polygon", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000", "radius": 2 }),
            "radius",
        ),
        (
            "draw.shape",
            json!({ "shape": "line", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "stroke": "#000000" }),
            "shape",
        ),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000", "rotation": 5 }),
            "rotation",
        ),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000", "dash": "dashed" }),
            "dash",
        ),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000", "anti_alias": true }),
            "anti_alias",
        ),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff", "opacity": 50 }),
            "opacity",
        ),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff", "kind": "conical" }),
            "kind",
        ),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff", "kind": "reflected" }),
            "kind",
        ),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff", "kind": "diamond" }),
            "kind",
        ),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff", "channel": "transparency" }),
            "channel",
        ),
        // The Pixel editor's Text dialog has no underline or strikethrough.
        (
            "draw.text",
            json!({ "text": "A", "x": 0, "y": 0, "underline": true }),
            "underline",
        ),
        (
            "draw.text",
            json!({ "text": "A", "x": 0, "y": 0, "strikethrough": false }),
            "strikethrough",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(
            e.contains(&format!("`{arg}`")) && e.contains("is a Pixel document"),
            "{tool} {args}: {e}"
        );
        assert_eq!(s.state(&id), before);
    }
    let mut s = S::new();
    let id = s.new_image(8, 8);
    for (tool, args, arg) in [
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "pixel_perfect": true }),
            "pixel_perfect",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "ink": "shading" }),
            "ink",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "ramp_from": 0 }),
            "ramp_from",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "ramp_to": 0 }),
            "ramp_to",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "step": 1 }),
            "step",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "mirror": "x" }),
            "mirror",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "axis_x2": 4 }),
            "axis_x2",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "axis_y2": 4 }),
            "axis_y2",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "wrap": "x" }),
            "wrap",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "shape": "square" }),
            "shape",
        ),
        (
            "draw.stroke",
            json!({ "points": [[1, 1]], "shape": "round" }),
            "shape",
        ),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff", "dither": 4 }),
            "dither",
        ),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, args.clone());
        assert!(
            e.contains(&format!("`{arg}`")) && e.contains("is an Image document"),
            "{tool} {args}: {e}"
        );
        assert_eq!(s.image(&id).document(), &before);
    }
}

#[test]
fn values_outside_the_modes_range_fail_with_the_range() {
    let stroke = |extra: Value| {
        let mut a = json!({ "points": [[1, 1]] });
        a.as_object_mut()
            .expect("obj")
            .extend(extra.as_object().expect("obj").clone());
        a
    };
    let shape = |kind: &str, extra: Value| {
        let mut a =
            json!({ "shape": kind, "x0": 0, "y0": 0, "x1": 3, "y1": 3, "stroke": "#000000" });
        a.as_object_mut()
            .expect("obj")
            .extend(extra.as_object().expect("obj").clone());
        a
    };
    let text = |size: f64| json!({ "text": "A", "x": 0, "y": 0, "size": size });
    let gradient = |extra: Value| {
        let mut a =
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff" });
        a.as_object_mut()
            .expect("obj")
            .extend(extra.as_object().expect("obj").clone());
        a
    };
    let pixel = [
        (
            "draw.stroke",
            stroke(json!({ "size": 9 })),
            "`size` must be 1–8 on Pixel documents",
        ),
        (
            "draw.stroke",
            stroke(json!({ "size": 0 })),
            "`size` must be 1–8 on Pixel documents",
        ),
        (
            "draw.stroke",
            stroke(json!({ "size": 501 })),
            "`size` must be 1–8 on Pixel documents",
        ),
        (
            "draw.shape",
            shape("rect", json!({ "stroke_width": 65 })),
            "`stroke_width` must be 1–64 on Pixel documents",
        ),
        (
            "draw.shape",
            shape("rect", json!({ "stroke_width": 501 })),
            "`stroke_width` must be 1–64 on Pixel documents",
        ),
        (
            "draw.shape",
            shape("rect", json!({ "stroke_width": 0 })),
            "`stroke_width` must be 1–64 on Pixel documents",
        ),
        (
            "draw.shape",
            shape("rounded", json!({ "corner_radius": 300 })),
            "`corner_radius` must be 0–256 on Pixel documents",
        ),
        (
            "draw.shape",
            shape("rounded", json!({ "corner_radius": -1 })),
            "`corner_radius` must be 0–256 on Pixel documents",
        ),
        (
            "draw.shape",
            shape("polygon", json!({ "sides": 65 })),
            "`sides` must be 3–64 on Pixel documents",
        ),
        (
            "draw.shape",
            shape("polygon", json!({ "sides": 101 })),
            "`sides` must be 3–64 on Pixel documents",
        ),
        (
            "draw.shape",
            shape("polygon", json!({ "sides": 2 })),
            "`sides` must be 3–64 on Pixel documents",
        ),
        (
            "draw.text",
            text(2001.0),
            "`size` must be 1–2000 on Pixel documents",
        ),
        (
            "draw.text",
            text(0.0),
            "`size` must be 1–2000 on Pixel documents",
        ),
        (
            "draw.gradient",
            gradient(json!({ "dither": 3 })),
            "`dither` must be 0, 2, 4 or 8 on Pixel documents",
        ),
    ];
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    for (tool, args, msg) in pixel {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(e.contains(msg), "{tool} {args}: {e}");
        assert_eq!(s.state(&id), before);
    }
    let image = [
        (
            "draw.stroke",
            stroke(json!({ "size": 501 })),
            "`size` must be 1–500 on Image documents",
        ),
        (
            "draw.stroke",
            stroke(json!({ "size": 0 })),
            "`size` must be 1–500 on Image documents",
        ),
        (
            "draw.shape",
            shape("rect", json!({ "stroke_width": 501 })),
            "`stroke_width` must be 1–500 on Image documents",
        ),
        (
            "draw.shape",
            shape("rect", json!({ "stroke_width": 0.5 })),
            "`stroke_width` must be 1–500 on Image documents",
        ),
        (
            "draw.shape",
            shape(
                "polygon",
                json!({ "sides": 101, "cx": 2, "cy": 2, "radius": 2 }),
            ),
            "`sides` must be 3–100 on Image documents",
        ),
        (
            "draw.shape",
            shape(
                "polygon",
                json!({ "sides": 2, "cx": 2, "cy": 2, "radius": 2 }),
            ),
            "`sides` must be 3–100 on Image documents",
        ),
        (
            "draw.text",
            text(1000.0),
            "`size` must be 1–999 on Image documents",
        ),
        (
            "draw.text",
            text(1500.0),
            "`size` must be 1–999 on Image documents",
        ),
        (
            "draw.text",
            text(0.5),
            "`size` must be 1–999 on Image documents",
        ),
    ];
    let mut s = S::new();
    let id = s.new_image(8, 8);
    for (tool, args, msg) in image {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, args.clone());
        assert!(e.contains(msg), "{tool} {args}: {e}");
        assert_eq!(s.image(&id).document(), &before);
    }
    // Unknown values name the rejected value, in both modes.
    let unknown = [
        (
            "draw.stroke",
            stroke(json!({ "tool": "spray" })),
            "\"spray\"",
        ),
        (
            "draw.stroke",
            stroke(json!({ "ink": "glaze" })),
            "\"glaze\"",
        ),
        ("draw.stroke", stroke(json!({ "mirror": "z" })), "\"z\""),
        (
            "draw.stroke",
            stroke(json!({ "wrap": "diagonal" })),
            "\"diagonal\"",
        ),
        (
            "draw.gradient",
            gradient(json!({ "kind": "spiral" })),
            "\"spiral\"",
        ),
        (
            "draw.text",
            json!({ "text": "A", "x": 0, "y": 0, "align": "justify" }),
            "\"justify\"",
        ),
    ];
    for image in [false, true] {
        let mut s = S::new();
        if image {
            s.new_image(8, 8);
        } else {
            s.new_doc(8, 8);
        }
        for (tool, args, value) in &unknown {
            let e = s.err(tool, args.clone());
            assert!(e.contains(value), "image: {image}, {tool} {args}: {e}");
        }
    }
}

#[test]
fn drawing_on_locked_group_or_tilemap_layers_fails_with_the_reason() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    s.ok("layer.add", json!({ "kind": "group" }));
    s.ok("layer.set", json!({ "layer": 0, "locked": true }));
    s.ws.get_mut(&id)
        .expect("doc")
        .sprite
        .layers
        .push(pincel_core::Layer::tilemap(
            LayerId::new(7),
            "Map",
            pincel_core::TilesetId::new(0),
        ));
    let calls = [
        ("draw.stroke", json!({ "points": [[1, 1]] })),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000" }),
        ),
        ("draw.text", json!({ "text": "A", "x": 0, "y": 0 })),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff" }),
        ),
    ];
    for (layer, reason) in [(0, "is locked"), (1, "a group"), (7, "not an image layer")] {
        for (tool, args) in &calls {
            let mut a = args.clone();
            a["layer"] = json!(layer);
            let before = s.state(&id);
            let e = s.err(tool, a);
            assert!(
                e.contains(reason) && e.contains(&format!("layer {layer}")),
                "{tool}: {e}"
            );
            assert_eq!(s.state(&id), before);
        }
    }
    let mut s = S::new();
    let id = s.new_image(8, 8);
    s.ok("layer.add", json!({ "kind": "group" }));
    for (tool, args) in calls {
        let mut a = args.clone();
        a["layer"] = json!(1);
        let before = s.image(&id).document().clone();
        let e = s.err(tool, a);
        assert!(e.contains("is a group"), "{tool}: {e}");
        assert_eq!(s.image(&id).document(), &before);
    }
}

/// The four drawing calls of this spec, without a target layer.
fn drawing_calls() -> [(&'static str, Value); 4] {
    [
        ("draw.stroke", json!({ "points": [[1, 1], [3, 2]] })),
        (
            "draw.shape",
            json!({ "shape": "rect", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "fill": "#000000" }),
        ),
        ("draw.text", json!({ "text": "A", "x": 0, "y": 0 })),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff" }),
        ),
    ]
}

/// A locked group `Box` holding the image layer `Inner`; returns the ids
/// of the group and of `Inner`.
fn locked_group(s: &mut S) -> (u64, u64) {
    let group = s.ok("layer.add", json!({ "kind": "group", "name": "Box" }))["layer"]
        .as_u64()
        .expect("group id");
    let inner = s.ok("layer.add", json!({ "name": "Inner", "parent": group }))["layer"]
        .as_u64()
        .expect("layer id");
    s.ok("layer.set", json!({ "layer": group, "locked": true }));
    (group, inner)
}

#[test]
fn drawing_in_a_locked_group_or_on_a_locked_layer_names_the_layer() {
    let mut s = S::new();
    let id = s.new_doc(8, 8);
    let (_, inner) = locked_group(&mut s);
    for (tool, args) in drawing_calls() {
        let mut a = args.clone();
        a["layer"] = json!(inner);
        let before = s.state(&id);
        let e = s.err(tool, a);
        assert!(
            e.contains(&format!("layer {inner} (\"Inner\") is locked")),
            "Pixel {tool}: {e}"
        );
        assert_eq!(s.state(&id), before);
    }

    let mut s = S::new();
    let id = s.new_image(8, 8);
    s.ok(
        "layer.set",
        json!({ "layer": 0, "name": "Base", "locked": true }),
    );
    let (group, inner) = locked_group(&mut s);
    for (tool, args) in drawing_calls() {
        for (layer, reason) in [
            (0, "layer 0 (\"Base\") is locked".to_owned()),
            (inner, format!("layer {inner} (\"Inner\") is locked")),
            (group, format!("layer {group} (\"Box\") is a group")),
        ] {
            let mut a = args.clone();
            a["layer"] = json!(layer);
            let before = s.image(&id).document().clone();
            let e = s.err(tool, a);
            assert!(e.contains(&reason), "Image {tool} on {layer}: {e}");
            assert_eq!(s.image(&id).document(), &before);
        }
    }
}

#[test]
fn every_pixel_drawing_call_is_one_undo_step() {
    let mut s = S::new();
    let id = s.new_doc(16, 12);
    for (tool, args) in [
        (
            "draw.stroke",
            json!({ "points": [[1, 1], [6, 6]], "mirror": "xy", "wrap": "xy" }),
        ),
        ("draw.stroke", json!({ "points": [[2, 1], [7, 6]] })),
        (
            "draw.shape",
            json!({ "shape": "ellipse", "x0": 0, "y0": 0, "x1": 9, "y1": 9, "stroke": "#ff0000" }),
        ),
        ("draw.text", json!({ "text": "A", "x": 2, "y": 2 })),
        (
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 15, "y1": 0, "from": "#000000", "to": "#ffffff" }),
        ),
    ] {
        let depth = undo_depth(&mut s);
        let before = s.state(&id);
        s.ok(tool, args.clone());
        assert_eq!(undo_depth(&mut s), depth + 1, "{tool} {args}");
        s.ok("doc.undo", json!({}));
        assert_eq!(s.state(&id), before, "{tool} undo");
        s.ok("doc.redo", json!({}));
    }
    let depth = undo_depth(&mut s);
    let out = s.ok("draw.stroke", json!({ "points": [[40, 40], [50, 50]] }));
    assert_eq!(out["changed"], false, "entirely outside the canvas");
    assert_eq!(undo_depth(&mut s), depth);
}

#[test]
fn line_rect_and_ellipse_on_an_image_document_point_to_draw_shape() {
    let mut s = S::new();
    let id = s.new_image(8, 8);
    for (tool, shape) in [
        ("draw.line", "line"),
        ("draw.rect", "rect"),
        ("draw.ellipse", "ellipse"),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(
            tool,
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 3, "color": "#000000" }),
        );
        assert!(
            e.contains("Image document") && e.contains("draw.shape") && e.contains(shape),
            "{tool}: {e}"
        );
        assert_eq!(s.image(&id).document(), &before);
    }
}
