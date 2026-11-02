//! Benchmark against the reference implementation
//! (`docs/specs/vectorize-core.md` AC-22): session plus one trace on the
//! three benchmark images. The reference medians come from
//! `tests/golden/generate.ts` on the same machine; compare by hand and
//! record both in the spec's Evidence. Not part of the PR gate.

use criterion::{Criterion, criterion_group, criterion_main};
use pincel_trace::{Mode, PotraceParams, Rgb, Threshold, TraceOptions, TraceSession};

/// The reference tests' generator, with JavaScript's arithmetic.
struct JsRng(i64);

impl JsRng {
    fn next(&mut self) -> f64 {
        let x = self.0 as f64 * 1_103_515_245.0 + 12_345.0;
        let m = x.trunc().rem_euclid(4_294_967_296.0) as u64;
        self.0 = (m & 0x7fff_ffff) as i64;
        self.0 as f64 / f64::from(0x7fff_ffff)
    }
}

fn clamped(v: f64) -> u8 {
    v.clamp(0.0, 255.0).round_ties_even() as u8
}

fn grey(v: u8) -> [u8; 4] {
    [v, v, v, 255]
}

fn line_art() -> (Vec<u8>, u32) {
    let mut rgba = Vec::with_capacity(1600 * 1600 * 4);
    for y in 0..1600_i32 {
        for x in 0..1600_i32 {
            let (fx, fy) = (f64::from(x), f64::from(y));
            let dc = ((fx - 800.0) * (fx - 800.0) + (fy - 800.0) * (fy - 800.0)).sqrt();
            let on_circle = (1..=40).any(|k| (dc - f64::from(k * 20)).abs() <= 1.5);
            let on_line = (0..40).any(|i| {
                let (x1, y1, x2, y2) = (0.0, f64::from(40 * i), 1599.0, f64::from(1599 - 40 * i));
                let num = ((x2 - x1) * (y1 - fy) - (x1 - fx) * (y2 - y1)).abs();
                num / ((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)).sqrt() <= 1.5
            });
            rgba.extend_from_slice(&grey(if on_circle || on_line { 0 } else { 255 }));
        }
    }
    (rgba, 1600)
}

fn text_scan() -> (Vec<u8>, u32) {
    let mut bg = JsRng(1);
    let mut rgba = Vec::with_capacity(1200 * 1200 * 4);
    for _ in 0..1200 * 1200 {
        rgba.extend_from_slice(&grey(clamped(235.0 + (bg.next() * 20.0 - 10.0))));
    }
    let mut rnd = JsRng(5);
    for row in 0..30 {
        for col in 0..50 {
            let gw = 8 + (rnd.next() * 9.0).floor() as usize;
            let gh = 12 + (rnd.next() * 13.0).floor() as usize;
            for y in 0..gh {
                for x in 0..gw {
                    let i = ((row * 40 + 6 + y) * 1200 + col * 24 + 4 + x) * 4;
                    rgba[i..i + 4].copy_from_slice(&grey(30));
                }
            }
        }
    }
    (rgba, 1200)
}

fn noise() -> (Vec<u8>, u32) {
    let mut rnd = JsRng(3);
    let mut rgba = Vec::with_capacity(800 * 800 * 4);
    for _ in 0..800 * 800 {
        rgba.extend_from_slice(&grey(if rnd.next() < 0.5 { 0 } else { 255 }));
    }
    (rgba, 800)
}

fn mono(threshold: Threshold, min_area: u32) -> TraceOptions {
    TraceOptions {
        mode: Mode::Mono {
            threshold,
            invert: false,
            fill: Rgb::new(0, 0, 0),
        },
        despeckle: false,
        potrace: PotraceParams {
            min_area,
            ..PotraceParams::default()
        },
    }
}

fn bench(c: &mut Criterion) {
    let cases = [
        ("line_art", line_art(), mono(Threshold::Auto, 2)),
        ("text_scan", text_scan(), mono(Threshold::Auto, 2)),
        ("noise", noise(), mono(Threshold::Manual(128), 0)),
    ];
    let mut group = c.benchmark_group("trace");
    group.sample_size(10);
    for (name, (rgba, side), opts) in &cases {
        group.bench_function(*name, |b| {
            b.iter(|| {
                TraceSession::new(rgba, *side, *side)
                    .and_then(|mut s| s.trace(opts))
                    .expect("trace")
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
