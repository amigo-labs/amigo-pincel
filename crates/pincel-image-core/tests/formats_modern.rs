//! JPEG XL in Image mode (docs/specs/image-formats-more-modern.md): decoding
//! against reference renderings of libjxl (`tests/fixtures/modern`,
//! `make_fixtures.py`), lossless encoding round trips, and the size and
//! damage errors.

use pincel_image_core::codec::{CodecError, JXL_MIN_SIDE, decode, is_jxl, to_jxl_bytes};
use pincel_image_core::{DocumentError, ImageBuffer};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/modern/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("fixture")
}

/// The reference rendering of `name` (an 8-bit RGBA PNG).
fn reference(name: &str) -> ImageBuffer {
    decode(&fixture(name)).expect("reference PNG")
}

/// Mean absolute error per channel (R, G, B, A).
fn mae(a: &ImageBuffer, b: &ImageBuffer) -> [f64; 4] {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let mut sum = [0f64; 4];
    for (pa, pb) in a
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.data().as_chunks::<4>().0)
    {
        for c in 0..4 {
            sum[c] += (f64::from(pa[c]) - f64::from(pb[c])).abs();
        }
    }
    let n = f64::from(a.width() * a.height());
    sum.map(|s| s / n)
}

fn assert_close(name: &str, png: &str) {
    let got = decode(&fixture(name)).expect(name);
    let want = reference(png);
    for (c, e) in mae(&got, &want).iter().enumerate() {
        assert!(*e < 2.0, "{name}: channel {c} differs by {e} on average");
    }
}

// AC-03: lossless 8-bit opens with exactly its pixels, bare codestream and
// container alike.
#[test]
fn jxl_lossless_8_bit_opens_exactly() {
    let want = reference("lossless8.png");
    for name in ["lossless8.jxl", "container.jxl"] {
        let got = decode(&fixture(name)).expect(name);
        assert_eq!(got.data(), want.data(), "{name}");
    }
}

// AC-03: lossy files (with and without alpha) and a 16-bit lossless file
// open within 2 of 255 of the reference rendering.
#[test]
fn jxl_lossy_and_deep_open_within_the_bound() {
    assert_close("lossy_alpha.jxl", "lossy_alpha.png");
    assert_close("lossy_rgb.jxl", "lossy_rgb.png");
    assert_close("lossless16.jxl", "lossless16.png");
}

// AC-03: an animated file opens from its first frame.
#[test]
fn jxl_animation_opens_its_first_frame() {
    let got = decode(&fixture("animated.jxl")).expect("animated");
    assert_eq!(got.data(), reference("animated_first.png").data());
}

// AC-08: a truncated file fails to open.
#[test]
fn jxl_truncated_fails() {
    assert!(decode(&fixture("truncated.jxl")).is_err());
}

// AC-12: a file declaring 40000 px fails with the size error, though its
// pixel data is cut off.
#[test]
fn jxl_oversized_header_fails_with_the_size_error() {
    let err = decode(&fixture("huge.jxl")).expect_err("too large");
    assert!(
        matches!(
            err,
            CodecError::Buffer(DocumentError::InvalidCanvasSize { .. })
        ),
        "{err:?}"
    );
}

// AC-05: what Image mode writes decodes back to exactly those pixels,
// transparent ones included.
#[test]
fn jxl_written_files_decode_back_exactly() {
    for (w, h) in [(2, 2), (3, 7), (64, 33)] {
        let data: Vec<u8> = (0..w * h * 4)
            .map(|i| match i % 4 {
                3 if (i / 4) % 5 == 0 => 0,
                3 => (i * 37 % 256) as u8,
                _ => (i * 91 % 256) as u8,
            })
            .collect();
        let buf = ImageBuffer::from_raw(w, h, data).expect("buffer");
        let bytes = to_jxl_bytes(&buf).expect("encodes");
        let back = decode(&bytes).expect("decodes");
        assert_eq!(back.data(), buf.data(), "{w}x{h}");
    }
}

// AC-16: the encoder needs 2 px on each side; a 1-px side fails with its
// own error, not a generic encoder failure.
#[test]
fn to_jxl_bytes_with_a_1_px_side_returns_the_size_error() {
    for (w, h) in [(1, 1), (1, 8), (8, 1)] {
        let err = to_jxl_bytes(&ImageBuffer::new_transparent(w, h)).expect_err("1-px side");
        assert!(matches!(err, CodecError::JxlTooSmall), "{w}x{h}: {err:?}");
        assert_eq!(
            err.to_string(),
            "JPEG XL needs at least 2 × 2 px",
            "{w}x{h}"
        );
    }
    // The smallest size the encoder writes still round-trips (AC-05).
    let edge = JXL_MIN_SIDE;
    let buf = ImageBuffer::new_transparent(edge, edge + 5);
    assert_eq!(
        decode(&to_jxl_bytes(&buf).expect("2 px"))
            .expect("decodes")
            .data(),
        buf.data()
    );
}

// AC-04: JPEG XL is recognised by content; an AVIF file is not JPEG XL and
// fails to open, a PNG is a PNG whatever its name.
#[test]
fn jxl_is_recognised_by_content_and_avif_fails() {
    assert!(is_jxl(&fixture("lossless8.jxl")));
    assert!(is_jxl(&fixture("container.jxl")));
    let avif = fixture("still.avif");
    assert!(!is_jxl(&avif));
    assert!(decode(&avif).is_err());
    let png = fixture("lossless8.png");
    assert!(!is_jxl(&png));
    assert_eq!(decode(&png).expect("png").width(), 24);
}
