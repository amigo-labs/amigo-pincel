//! The flat option vector both wasm layers pass
//! (`docs/specs/vectorize-dialog.md`, Technical notes; Decision Log
//! 2026-11-01 "SVG export dialog").

use pincel_trace::{Mode, PotraceParams, Rgb, Threshold, TraceError, TraceOptions};

const DEFAULTS: [f64; 9] = [6.0, -1.0, 0.0, 0.0, 0.0, 2.0, 1.0, 1.0, 0.2];

#[test]
fn from_wire_defaults_match_trace_options_defaults() {
    assert_eq!(
        TraceOptions::from_wire(0, &DEFAULTS),
        Ok(TraceOptions::default())
    );
}

#[test]
fn from_wire_reads_every_field() {
    let mono = TraceOptions::from_wire(
        1,
        &[3.0, 77.0, 1.0, f64::from(0x0a141e), 1.0, 0.0, 0.5, 0.0, 0.3],
    )
    .expect("valid");
    assert_eq!(
        mono,
        TraceOptions {
            mode: Mode::Mono {
                threshold: Threshold::Manual(77),
                invert: true,
                fill: Rgb::new(10, 20, 30)
            },
            despeckle: true,
            potrace: PotraceParams {
                min_area: 0,
                alpha_max: 0.5,
                opt_curve: false,
                opt_tolerance: 0.3,
                ..PotraceParams::default()
            },
        }
    );
    let auto = TraceOptions::from_wire(1, &DEFAULTS).expect("valid");
    assert!(matches!(
        auto.mode,
        Mode::Mono {
            threshold: Threshold::Auto,
            ..
        }
    ));
    assert_eq!(
        TraceOptions::from_wire(2, &DEFAULTS).expect("valid").mode,
        Mode::PixelExact
    );
}

#[test]
fn from_wire_rejects_malformed_input() {
    let option = |r: Result<TraceOptions, TraceError>| match r {
        Err(TraceError::InvalidOption { option, .. }) => option,
        other => panic!("expected an invalid option, got {other:?}"),
    };
    assert_eq!(option(TraceOptions::from_wire(3, &DEFAULTS)), "mode");
    assert_eq!(option(TraceOptions::from_wire(0, &DEFAULTS[..8])), "params");
    let with = |i: usize, v: f64| {
        let mut p = DEFAULTS;
        p[i] = v;
        TraceOptions::from_wire(1, &p)
    };
    assert_eq!(option(with(0, 2.5)), "colors");
    assert_eq!(option(with(1, 256.0)), "threshold");
    assert_eq!(option(with(1, 1.5)), "threshold");
    assert_eq!(option(with(3, 16_777_216.0)), "fill");
    assert_eq!(option(with(5, -1.0)), "min_area");
    assert_eq!(option(with(5, f64::NAN)), "min_area");
}
