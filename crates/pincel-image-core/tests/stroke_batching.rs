//! A brush stroke paints the same pixels whether the UI hands its pointer
//! samples to the core one event at a time or batched per animation frame
//! (`docs/specs/performance-input-path.md` AC-01).
//!
//! The UI sends a press as a one-point stroke, then every update as the
//! previous point followed by the new samples, all under one `stroke_id`.

use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{Color, CommandBus, Document, ImageBuffer};

const W: u32 = 48;
const H: u32 = 48;

/// A one-layer document whose layer holds a color pattern, so smudge,
/// recolor and partial strokes have something to work on.
fn document() -> CommandBus {
    let mut layer = ImageBuffer::new_transparent(W, H);
    for y in 0..H {
        for x in 0..W {
            let c = if (x / 6 + y / 6) % 2 == 0 {
                Color::rgba(200, 40, 40, 255)
            } else {
                Color::rgba(30, 90, 220, 180)
            };
            layer.set_pixel(x, y, c);
        }
    }
    CommandBus::new(Document::from_pixels(layer).expect("doc"))
}

/// A deterministic wandering drag of `n` sub-pixel samples with pressure.
fn wander(n: usize, seed: u32) -> Vec<([f32; 2], f32)> {
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) as f32 / (1u32 << 24) as f32
    };
    let (mut x, mut y) = (24.0f32, 24.0f32);
    (0..n)
        .map(|_| {
            x = (x + next() * 10.0 - 5.0).clamp(-3.0, W as f32 + 3.0);
            y = (y + next() * 10.0 - 5.0).clamp(-3.0, H as f32 + 3.0);
            ([x, y], 0.2 + next() * 0.8)
        })
        .collect()
}

/// The JSON of one stroke command of `kind` over `pts`.
fn command(kind: &str, pts: &[([f32; 2], f32)]) -> CommandSpec {
    let points: Vec<[f32; 2]> = pts.iter().map(|p| p.0).collect();
    let pressure: Vec<f32> = pts.iter().map(|p| p.1).collect();
    let common = serde_json::json!({
        "layer": 0,
        "size": 7,
        "opacity": 0.6,
        "shape": "soft_round",
        "hardness": 0.4,
        "points": points,
        "pressure": pressure,
        "pressure_size": true,
        "pressure_opacity": true,
        "stroke_id": 9,
    });
    let mut v = common;
    let o = v.as_object_mut().expect("object");
    o.insert("type".into(), format!("{kind}_stroke").into());
    match kind {
        "pencil" => {
            o.insert("color".into(), serde_json::json!([10, 200, 60, 255]));
        }
        "eraser" => {}
        "recolor" => {
            o.insert("from".into(), serde_json::json!([200, 40, 40, 255]));
            o.insert("to".into(), serde_json::json!([250, 250, 0, 255]));
            o.insert("tolerance".into(), serde_json::json!(40));
        }
        "smudge" => {
            o.insert("strength".into(), serde_json::json!(0.7));
        }
        _ => panic!("unknown stroke kind {kind}"),
    }
    serde_json::from_value(v).expect("valid command JSON")
}

/// Runs a stroke the way the UI does: the press as one point, then each
/// group of samples prefixed with the last point sent. Returns the pixels.
fn run(kind: &str, samples: &[([f32; 2], f32)], groups: &[usize]) -> Vec<Option<Color>> {
    let mut bus = document();
    let mut scratch = StrokeScratch::default();
    let (first, rest) = samples.split_first().expect("samples");
    run_command(&mut bus, command(kind, &[*first]), &mut scratch, &[]).expect("press");
    let mut last = *first;
    let mut start = 0;
    for &len in groups.iter().cycle() {
        if start >= rest.len() {
            break;
        }
        let end = (start + len).min(rest.len());
        let mut pts = vec![last];
        pts.extend_from_slice(&rest[start..end]);
        run_command(&mut bus, command(kind, &pts), &mut scratch, &[]).expect("move");
        last = rest[end - 1];
        start = end;
    }
    assert_eq!(bus.history.undo_depth(), 1, "{kind}: one undo step");
    pixels(&bus)
}

/// Every pixel of the top layer, row by row.
fn pixels(bus: &CommandBus) -> Vec<Option<Color>> {
    let layer = &bus.document.layers()[0].pixels;
    (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .map(|(x, y)| layer.get_pixel(x, y))
        .collect()
}

#[test]
fn stroke_batched_per_frame_paints_like_one_sample_per_event() {
    for (seed, kind) in ["pencil", "eraser", "recolor", "smudge"].iter().enumerate() {
        let samples = wander(80, seed as u32 + 3);
        let per_event = run(kind, &samples, &[1]);
        let per_frame = run(kind, &samples, &[3, 1, 7, 2, 5]);
        let all_at_once = run(kind, &samples, &[samples.len()]);
        assert_ne!(per_event, pixels(&document()), "{kind}: the stroke paints");
        assert_eq!(per_frame, per_event, "{kind}: batched per frame");
        assert_eq!(all_at_once, per_event, "{kind}: one batch");
    }
}
