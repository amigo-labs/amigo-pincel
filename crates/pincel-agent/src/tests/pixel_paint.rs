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
    for t in list_tools() {
        if ["draw.stroke", "draw.shape", "draw.text", "draw.gradient"].contains(&t.name) {
            assert!(
                t.description.contains("Image documents")
                    && t.description.contains("Pixel documents"),
                "{}",
                t.name
            );
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
}

#[test]
fn pixel_shading_stroke_equals_the_editors_shading_ink() {
    for (args, from, to, step) in [
        (json!({}), 0usize, 31usize, 1i8),
        (
            json!({ "ramp_from": 3, "ramp_to": 7, "step": -1 }),
            3,
            7,
            -1,
        ),
    ] {
        let mut s = S::new();
        let id = pixel_doc(&mut s);
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
    let e = s.err(
        "draw.stroke",
        json!({ "points": [[1, 1]], "ink": "shading" }),
    );
    assert!(e.contains("palette is empty"), "{e}");
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
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(e.contains(needle), "{tool} {args}: {e}");
        assert_eq!(s.state(&id), before);
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
    for (args, needle) in [
        (
            json!({ "shape": "line", "x0": 0, "y0": 0, "x1": 3, "y1": 3, "stroke": "#000000" }),
            "draw.line",
        ),
        (
            json!({ "shape": "polygon", "cx": 4, "cy": 4, "radius": 3, "stroke": "#000000" }),
            "(x0, y0)–(x1, y1)",
        ),
    ] {
        let before = s.state(&id);
        let e = s.err("draw.shape", args.clone());
        assert!(e.contains(needle), "{args}: {e}");
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
    ] {
        let mut s = S::new();
        let id = s.new_doc(40, 20);
        let style = TextStyle {
            size,
            color: BLACK,
            bold,
            italic: false,
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
    s.new_doc(8, 8);
    let depth = undo_depth(&mut s);
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
    ] {
        let before = s.state(&id);
        let e = s.err(tool, args.clone());
        assert!(
            e.contains(&format!("`{arg}`")) && e.contains("Pixel document"),
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
            "draw.gradient",
            json!({ "x0": 0, "y0": 0, "x1": 3, "y1": 0, "from": "#000000", "to": "#ffffff", "dither": 4 }),
            "dither",
        ),
    ] {
        let before = s.image(&id).document().clone();
        let e = s.err(tool, args.clone());
        assert!(
            e.contains(&format!("`{arg}`")) && e.contains("Image document"),
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
