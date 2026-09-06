//! Snapshot tests for compositing and codec pixel output (CLAUDE.md §7.3).
//!
//! A known layer stack must keep producing byte-identical RGBA output; any
//! change to blend math, opacity handling, or codec round-trips shows up as a
//! reviewable snapshot diff under `tests/snapshots/`.

use fineliner_core::{codec, compose, BlendMode, Color, ImageBuffer, Layer};

/// A 4×4 buffer with a deterministic per-pixel gradient.
fn gradient(alpha: u8) -> ImageBuffer {
    let mut buf = ImageBuffer::new_transparent(4, 4);
    for y in 0..4 {
        for x in 0..4 {
            buf.set_pixel(
                x,
                y,
                Color::rgba((x * 60) as u8, (y * 60) as u8, 128, alpha),
            );
        }
    }
    buf
}

/// Renders the composite as rows of `RRGGBBAA` hex pixels for stable diffs.
fn hex_rows(buf: &ImageBuffer) -> String {
    let mut out = String::new();
    for y in 0..buf.height() {
        for x in 0..buf.width() {
            let c = buf.get_pixel(x, y).expect("in bounds");
            out.push_str(&format!("{:02x}{:02x}{:02x}{:02x} ", c.r, c.g, c.b, c.a));
        }
        out.push('\n');
    }
    out
}

#[test]
fn compose_normal_over_gradient_snapshot() {
    let bottom = Layer::from_pixels("bottom", gradient(255));
    let top = Layer::from_pixels("top", gradient(128)).with_opacity(0.5);
    insta::assert_snapshot!(hex_rows(&compose(&[bottom, top])));
}

#[test]
fn compose_multiply_and_screen_stack_snapshot() {
    let bottom = Layer::from_pixels("bottom", gradient(255));
    let mid = Layer::from_pixels("mid", gradient(200)).with_blend_mode(BlendMode::Multiply);
    let top = Layer::from_pixels("top", gradient(90)).with_blend_mode(BlendMode::Screen);
    insta::assert_snapshot!(hex_rows(&compose(&[bottom, mid, top])));
}

#[test]
fn png_round_trip_pixels_snapshot() {
    let src = gradient(180);
    let bytes = codec::to_png_bytes(&src, 6).expect("encode");
    let back = codec::decode(&bytes).expect("decode");
    insta::assert_snapshot!(hex_rows(&back));
}
