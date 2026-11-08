//! Normal and Overwrite blending of the painting tools
//! (docs/specs/image-edit-extras-tool-options.md AC-09 – AC-13).

use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{CommandBus, Document, DocumentError, SetLayerLocked};

const FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../pincel-core/tests/fixtures/LiberationSans-Regular.ttf"
));

const BLUE: [u8; 4] = [0, 0, 255, 255];

fn bus(w: u32, h: u32) -> CommandBus {
    CommandBus::new(Document::new(w, h).expect("doc"))
}

fn try_run(
    bus: &mut CommandBus,
    json: &str,
    scratch: &mut StrokeScratch,
) -> Result<(), DocumentError> {
    let spec: CommandSpec = serde_json::from_str(json).expect("json");
    run_command(bus, spec, scratch, &[FONT])
}

fn run(bus: &mut CommandBus, json: &str) {
    try_run(bus, json, &mut StrokeScratch::default()).expect(json);
}

/// A `w` × `h` document whose layer is `color` everywhere.
fn solid(w: u32, h: u32, color: [u8; 4]) -> CommandBus {
    let mut b = bus(w, h);
    if color[3] > 0 {
        run(
            &mut b,
            &format!(r#"{{"type":"fill_selection","layer":0,"color":{color:?}}}"#),
        );
    }
    b
}

fn pixels(b: &CommandBus) -> Vec<u8> {
    b.document.layers()[0].pixels.data().to_vec()
}

fn px(b: &CommandBus, x: u32, y: u32) -> [u8; 4] {
    let c = b.document.layers()[0]
        .pixels
        .get_pixel(x, y)
        .expect("pixel");
    [c.r, c.g, c.b, c.a]
}

/// `json` with `"blend":"<blend>"` added after its opening brace.
fn with_blend(json: &str, blend: &str) -> String {
    format!(r#"{{"blend":"{blend}",{}"#, &json[1..])
}

const PENCIL: &str = r#"{"type":"pencil_stroke","layer":0,"size":9,"color":[255,0,0,255],"opacity":0.4,"shape":"hard_round","points":[[10.5,10.5],[16.2,12.7]],"anti_alias":true,"stroke_id":1}"#;
const BUCKET: &str = r#"{"type":"fill_bucket","layer":0,"color":[255,0,0,255],"opacity":0.4,"tolerance":0,"contiguous":true,"x":3,"y":3}"#;
const GRADIENT: &str = r#"{"type":"draw_gradient","layer":0,"x0":0,"y0":0,"x1":20,"y1":0,"from":[255,0,0,255],"to":[0,255,0,0],"opacity":1}"#;
const SHAPE: &str = r#"{"type":"draw_shape","layer":0,"shape":"rectangle","points":[[2,2],[18,18]],"mode":"fill_and_outline","stroke_width":3,"stroke_color":[255,0,0,60],"fill_color":[0,255,0,120],"anti_alias":true}"#;
const TEXT: &str = r#"{"type":"draw_text","layer":0,"font_id":0,"text":"Wl","x":1,"y":1,"size":18,"color":[255,0,0,100],"anti_alias":true}"#;

#[test]
fn normal_blending_paints_as_before_for_every_tool() {
    for json in [PENCIL, BUCKET, GRADIENT, SHAPE, TEXT] {
        let mut old = solid(24, 24, [0, 0, 255, 128]);
        run(&mut old, json);
        let mut normal = solid(24, 24, [0, 0, 255, 128]);
        run(&mut normal, &with_blend(json, "normal"));
        assert_eq!(pixels(&old), pixels(&normal), "{json}");
    }
}

#[test]
fn overwrite_replaces_fully_covered_pixels_alpha_included() {
    // Brush at 40 %: alpha 102 where Normal leaves 255.
    let mut normal = solid(24, 24, BLUE);
    run(&mut normal, PENCIL);
    assert_eq!(px(&normal, 10, 10)[3], 255);
    let mut over = solid(24, 24, BLUE);
    run(&mut over, &with_blend(PENCIL, "overwrite"));
    let [r, g, b, a] = px(&over, 10, 10);
    assert_eq!([r, g, b], [255, 0, 0]);
    assert!(a.abs_diff(102) <= 1, "alpha {a}");

    // Paint Bucket: the whole region takes the paint alpha.
    let mut bucket = solid(24, 24, BLUE);
    run(&mut bucket, &with_blend(BUCKET, "overwrite"));
    for (x, y) in [(0, 0), (12, 7), (23, 23)] {
        let [r, g, b, a] = px(&bucket, x, y);
        assert_eq!([r, g, b], [255, 0, 0]);
        assert!(a.abs_diff(102) <= 1);
    }

    // Gradient: the ramp's own color and alpha at each pixel.
    let mut grad = solid(24, 24, BLUE);
    run(&mut grad, &with_blend(GRADIENT, "overwrite"));
    let [_, _, _, near_end] = px(&grad, 19, 5);
    assert!(near_end < 16, "alpha {near_end}");
    assert_eq!(px(&grad, 23, 5), [0, 0, 0, 0]);
    let mut grad_normal = solid(24, 24, BLUE);
    run(&mut grad_normal, GRADIENT);
    assert_eq!(px(&grad_normal, 19, 5)[3], 255);

    // Shapes: the fill takes its color, the outline overwrites the fill.
    let mut shape = solid(24, 24, BLUE);
    run(&mut shape, &with_blend(SHAPE, "overwrite"));
    assert_eq!(px(&shape, 10, 10), [0, 255, 0, 120]);
    assert_eq!(px(&shape, 2, 10), [255, 0, 0, 60]);

    // Text: glyph interiors take the paint exactly.
    let mut text = solid(24, 24, BLUE);
    run(&mut text, &with_blend(TEXT, "overwrite"));
    let data = pixels(&text);
    assert!(data.chunks_exact(4).any(|p| p == [255, 0, 0, 100]));
    assert!(data.chunks_exact(4).all(|p| p == BLUE || p[3] >= 100));
}

#[test]
fn overwrite_moves_partially_covered_pixels_by_their_coverage() {
    let dab = r#"{"type":"pencil_stroke","layer":0,"size":11,"color":[255,0,0,255],"opacity":0.4,"shape":"hard_round","points":[[10.3,10.6]],"anti_alias":true,"stroke_id":1}"#;
    // On a transparent layer Normal at full opacity gives alpha = 255 · c.
    let full = dab.replace(r#""opacity":0.4"#, r#""opacity":1"#);
    let mut probe = bus(21, 21);
    run(&mut probe, &full);
    let mut over = solid(21, 21, BLUE);
    run(&mut over, &with_blend(dab, "overwrite"));
    let mut partial = 0;
    for y in 0..21 {
        for x in 0..21 {
            let c = f32::from(px(&probe, x, y)[3]) / 255.0;
            let [r, g, b, a] = px(&over, x, y);
            if c == 0.0 {
                assert_eq!([r, g, b, a], BLUE, "({x}, {y}) unchanged");
                continue;
            }
            let want = 255.0 + (102.0 - 255.0) * c;
            assert!(
                (f32::from(a) - want).abs() <= 1.0,
                "({x}, {y}): {a} vs {want}"
            );
            assert_eq!(g, 0);
            if c < 1.0 {
                partial += 1;
                assert!(r > 0 && b > 0, "({x}, {y}) mixes both colors");
            }
        }
    }
    assert!(partial > 0);
}

#[test]
fn overwrite_stroke_takes_the_highest_coverage_and_never_builds_up() {
    let stroke = r#"{"type":"pencil_stroke","layer":0,"size":12,"color":[255,0,0,255],"opacity":0.5,"shape":"soft_round","hardness":0.2,"points":[[6,15],[24,15]],"stroke_id":7}"#;
    let mut normal = bus(30, 30);
    run(&mut normal, stroke);
    assert!(
        pixels(&normal).chunks_exact(4).any(|p| p[3] > 140),
        "Normal builds up"
    );
    let mut over = bus(30, 30);
    run(&mut over, &with_blend(stroke, "overwrite"));
    let alphas: Vec<u8> = pixels(&over).chunks_exact(4).map(|p| p[3]).collect();
    assert!(
        alphas.iter().all(|&a| a <= 128),
        "max {:?}",
        alphas.iter().max()
    );
    assert!(alphas.contains(&128));

    // The same drag split into two commands of one stroke ends the same,
    // as one undo step.
    let mut split = bus(30, 30);
    let mut scratch = StrokeScratch::default();
    let first = with_blend(&stroke.replace("[24,15]", "[15,15]"), "overwrite");
    let second = with_blend(&stroke.replace("[6,15]", "[15,15]"), "overwrite");
    try_run(&mut split, &first, &mut scratch).expect("first");
    try_run(&mut split, &second, &mut scratch).expect("second");
    assert_eq!(pixels(&split), pixels(&over));
    assert_eq!(split.history.undo_depth(), 1);
    split.undo().expect("undo");
    assert!(pixels(&split).iter().all(|&v| v == 0));
}

#[test]
fn overwrite_keeps_undo_locks_and_the_selection() {
    for json in [PENCIL, BUCKET, GRADIENT, SHAPE, TEXT] {
        let over = with_blend(json, "overwrite");
        // One undo step, undone exactly.
        let mut b = solid(24, 24, BLUE);
        let before = pixels(&b);
        let depth = b.history.undo_depth();
        run(&mut b, &over);
        assert_eq!(b.history.undo_depth(), depth + 1, "{json}");
        b.undo().expect("undo");
        assert_eq!(pixels(&b), before, "{json}");

        // A locked layer is refused as under Normal.
        let refusal = |blend: &str| {
            let mut b = solid(24, 24, BLUE);
            b.apply(Box::new(SetLayerLocked::new(0, true)))
                .expect("lock");
            let before = pixels(&b);
            let r = try_run(
                &mut b,
                &with_blend(json, blend),
                &mut StrokeScratch::default(),
            );
            (r.map_err(|e| e.to_string()), pixels(&b) == before)
        };
        assert_eq!(refusal("overwrite"), refusal("normal"), "{json}");

        // Only selected pixels change.
        let mut s = solid(24, 24, BLUE);
        run(
            &mut s,
            r#"{"type":"select_rectangle","x":0,"y":0,"w":11,"h":24}"#,
        );
        run(&mut s, &over);
        for y in 0..24 {
            for x in 11..24 {
                assert_eq!(px(&s, x, y), BLUE, "{json} at ({x}, {y})");
            }
        }
    }
}

#[test]
fn live_shapes_gradients_and_texts_take_and_change_the_blend() {
    let begin = r#"{"type":"begin_live_shape","layer":0,"shape":"rectangle","points":[[2,2],[18,18]],"mode":"fill","fill_color":[0,255,0,120],"blend":"overwrite"}"#;
    let mut b = solid(24, 24, BLUE);
    run(&mut b, begin);
    assert_eq!(px(&b, 10, 10), [0, 255, 0, 120]);
    run(&mut b, r#"{"type":"edit_live","blend":"normal"}"#);
    assert_eq!(px(&b, 10, 10)[3], 255);
    run(&mut b, r#"{"type":"commit_live"}"#);

    let mut g = solid(24, 24, BLUE);
    run(
        &mut g,
        r#"{"type":"begin_live_gradient","layer":0,"x0":0,"y0":0,"x1":20,"y1":0,"from":[255,0,0,255],"to":[0,255,0,0],"blend":"overwrite"}"#,
    );
    assert_eq!(px(&g, 23, 5), [0, 0, 0, 0]);

    let mut t = solid(24, 24, BLUE);
    run(
        &mut t,
        r#"{"type":"begin_live_text","layer":0,"font_id":0,"text":"Wl","x":1,"y":1,"size":18,"color":[255,0,0,100],"anti_alias":true}"#,
    );
    run(&mut t, r#"{"type":"edit_live","blend":"overwrite"}"#);
    assert!(pixels(&t).chunks_exact(4).any(|p| p == [255, 0, 0, 100]));
}
