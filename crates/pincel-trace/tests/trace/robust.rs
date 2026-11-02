//! No accepted image and option set makes tracing or the SVG panic
//! (`docs/specs/vectorize-core.md` AC-21).

use pincel_trace::{Mode, Rgb, Threshold, TraceOptions, TraceSession, to_svg};

use crate::images::{self, Img};

fn small_images() -> Vec<Img> {
    let colours = [
        [255, 0, 0, 255],
        [0, 0, 255, 128],
        [0, 0, 0, 0],
        [20, 200, 20, 255],
    ];
    let pick = |x: i64, y: i64| colours[((x * 3 + y * 5) % 4) as usize];
    vec![
        Img::new(1, 1, |_, _| [9, 9, 9, 255]),
        Img::new(1, 7, pick),
        Img::new(7, 1, pick),
        Img::new(2, 2, pick),
        Img::new(3, 3, |x, y| {
            if (x + y) % 2 == 0 {
                [0, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        }),
        images::transparent_1x1(),
    ]
}

#[test]
fn every_mode_on_every_test_image_traces_without_panic() {
    let mut all = small_images();
    all.extend([
        images::disc(),
        images::ring(),
        images::rectangle(),
        images::square_with_stray_pixel(),
        images::four_colour(),
        images::noisy_two_tone(),
        images::speckled_two_tone(),
    ]);
    let modes = [
        Mode::Color {
            colors: 6,
            pinned: None,
        },
        Mode::Color {
            colors: 2,
            pinned: None,
        },
        Mode::Color {
            colors: 16,
            pinned: Some(vec![Rgb::new(0, 0, 0), Rgb::new(255, 0, 0)]),
        },
        Mode::Mono {
            threshold: Threshold::Auto,
            invert: false,
            fill: Rgb::new(0, 0, 0),
        },
        Mode::Mono {
            threshold: Threshold::Manual(0),
            invert: true,
            fill: Rgb::new(0, 0, 0),
        },
        Mode::Mono {
            threshold: Threshold::Manual(255),
            invert: false,
            fill: Rgb::new(0, 0, 0),
        },
        Mode::PixelExact,
    ];
    for img in &all {
        let mut session = TraceSession::new(&img.rgba, img.w, img.h).expect("valid image");
        for mode in &modes {
            for despeckle in [false, true] {
                let opts = TraceOptions {
                    mode: mode.clone(),
                    despeckle,
                    ..TraceOptions::default()
                };
                let r = session.trace(&opts).expect("accepted options trace");
                let svg = to_svg(&r);
                assert!(svg.starts_with("<svg ") && svg.ends_with("</svg>"));
                assert!(
                    !svg.contains("NaN") && !svg.contains("inf"),
                    "no NaN or infinite number (AC-04)"
                );
            }
        }
    }
}

/// The three benchmark images are test images too. They are large, so
/// each runs once per mode (the option variations above are covered on
/// the small images), with and without despeckle.
#[test]
fn every_mode_on_the_benchmark_images_traces_without_panic() {
    let modes = [
        Mode::Color {
            colors: 6,
            pinned: None,
        },
        Mode::Mono {
            threshold: Threshold::Auto,
            invert: false,
            fill: Rgb::new(0, 0, 0),
        },
        Mode::PixelExact,
    ];
    for img in [images::line_art(), images::text_scan(), images::noise()] {
        let mut session = TraceSession::new(&img.rgba, img.w, img.h).expect("valid image");
        for mode in &modes {
            for despeckle in [false, true] {
                let opts = TraceOptions {
                    mode: mode.clone(),
                    despeckle,
                    ..TraceOptions::default()
                };
                let r = session.trace(&opts).expect("accepted options trace");
                let svg = to_svg(&r);
                assert!(svg.starts_with("<svg ") && svg.ends_with("</svg>"));
                assert!(
                    !svg.contains("NaN") && !svg.contains("inf"),
                    "no NaN or infinite number (AC-04)"
                );
            }
        }
    }
}
