//! Opening raster files in Image mode (docs/specs/image-files.md AC-01,
//! AC-02): every supported format becomes a one-layer "Background"
//! document of the image's size holding its pixels, keeping only the first
//! frame, 8 bits per channel and no metadata.
//!
//! "Open" is what `pincel-image-wasm::open_image` does: `decode` then
//! `Document::from_pixels`. The fixtures are written with the `image`
//! crate's own encoders, independent of Pincel's.

use std::io::Cursor;

use image::codecs::gif::GifEncoder;
use image::{Delay, DynamicImage, Frame, ImageFormat, Rgb, Rgba, RgbaImage};
use pincel_image_core::codec::{decode, to_jpeg_bytes, to_png_bytes};
use pincel_image_core::{Document, compose};

/// What Image mode's Open does with the bytes of a raster file.
fn open(bytes: &[u8]) -> Document {
    Document::from_pixels(decode(bytes).expect("decodes")).expect("document")
}

/// A 5 × 3 opaque image with 15 distinct colors (fits a GIF palette).
fn sample() -> RgbaImage {
    RgbaImage::from_fn(5, 3, |x, y| {
        Rgba([
            (x * 50) as u8,
            (y * 100) as u8,
            (x * 20 + y * 30) as u8,
            255,
        ])
    })
}

fn encode(img: DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, format).expect("fixture encodes");
    out.into_inner()
}

/// Asserts `doc` is a single "Background" layer of `w` × `h`.
fn assert_one_background_layer(doc: &Document, w: u32, h: u32) {
    assert_eq!((doc.canvas.width(), doc.canvas.height()), (w, h));
    assert_eq!(doc.layer_count(), 1);
    let layer = &doc.layers()[0];
    assert_eq!(layer.name, "Background");
    assert_eq!((layer.pixels.width(), layer.pixels.height()), (w, h));
}

#[test]
fn open_lossless_formats_yield_one_background_layer_with_the_pixels() {
    let src = sample();
    for format in [
        ImageFormat::Png,
        ImageFormat::WebP,
        ImageFormat::Bmp,
        ImageFormat::Gif,
        ImageFormat::Tiff,
    ] {
        let doc = open(&encode(DynamicImage::ImageRgba8(src.clone()), format));
        assert_one_background_layer(&doc, 5, 3);
        assert_eq!(
            doc.layers()[0].pixels.data(),
            src.as_raw().as_slice(),
            "{format:?} pixels"
        );
    }
}

#[test]
fn open_jpeg_yields_one_background_layer_close_to_the_source() {
    // JPEG is lossy: two flat gray halves on 8 × 8 block edges come back
    // within a few levels.
    let src = image::RgbImage::from_fn(16, 8, |x, _| {
        if x < 8 {
            Rgb([40, 40, 40])
        } else {
            Rgb([220, 220, 220])
        }
    });
    let doc = open(&encode(
        DynamicImage::ImageRgb8(src.clone()),
        ImageFormat::Jpeg,
    ));
    assert_one_background_layer(&doc, 16, 8);
    let pixels = &doc.layers()[0].pixels;
    for (x, y, Rgb(want)) in src.enumerate_pixels() {
        let got = pixels.get_pixel(x, y).expect("in bounds");
        for (g, w) in [got.r, got.g, got.b].into_iter().zip(*want) {
            assert!(g.abs_diff(w) <= 6, "({x}, {y}): {got:?} vs {want:?}");
        }
        assert_eq!(got.a, 255);
    }
}

#[test]
fn open_animated_gif_keeps_the_first_frame_only() {
    let frame = |rgba: [u8; 4]| {
        Frame::from_parts(
            RgbaImage::from_pixel(3, 2, Rgba(rgba)),
            0,
            0,
            Delay::from_numer_denom_ms(100, 1),
        )
    };
    let mut bytes = Vec::new();
    GifEncoder::new(&mut bytes)
        .encode_frames([frame([200, 10, 10, 255]), frame([10, 10, 200, 255])])
        .expect("gif encodes");

    let doc = open(&bytes);
    assert_one_background_layer(&doc, 3, 2);
    let data = doc.layers()[0].pixels.data();
    assert!(
        data.chunks_exact(4).all(|px| px == [200, 10, 10, 255]),
        "first frame expected, got {data:?}"
    );
}

#[test]
fn open_sixteen_bit_png_converts_to_eight_bits_per_channel() {
    let src = image::ImageBuffer::<Rgba<u16>, Vec<u16>>::from_fn(2, 1, |x, _| {
        if x == 0 {
            Rgba([0xffff, 0x8000, 0x0000, 0xffff])
        } else {
            Rgba([0x1234, 0x00ff, 0xfedc, 0x7fff])
        }
    });
    let bytes = encode(DynamicImage::ImageRgba16(src), ImageFormat::Png);
    // Bit depth 16 in the IHDR (offset 24) proves the fixture is 16-bit.
    assert_eq!(bytes[24], 16);

    let doc = open(&bytes);
    assert_one_background_layer(&doc, 2, 1);
    // Each channel scales by 255 / 65535 and rounds (not just the high or
    // low byte).
    assert_eq!(
        doc.layers()[0].pixels.data(),
        &[255, 128, 0, 255, 18, 1, 254, 127]
    );
}

/// A baseline JPEG of `w` × `h` mid gray with an EXIF APP1 segment that
/// says "rotate 90° clockwise" (orientation 6) spliced in after SOI.
fn jpeg_with_exif_orientation(w: u32, h: u32) -> Vec<u8> {
    let jpeg = encode(
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, Rgb([128, 128, 128]))),
        ImageFormat::Jpeg,
    );
    let mut exif = b"Exif\0\0II*\0\x08\0\0\0".to_vec();
    // One IFD entry: Orientation (0x0112), SHORT, count 1, value 6.
    exif.extend_from_slice(&[1, 0, 0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0]);
    exif.extend_from_slice(&[0, 0, 0, 0]);
    let len = u16::try_from(exif.len() + 2).expect("short segment");
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xff, 0xe1]);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&exif);
    out.extend_from_slice(&jpeg[2..]);
    out
}

#[test]
fn open_jpeg_with_exif_drops_the_metadata() {
    let bytes = jpeg_with_exif_orientation(4, 2);
    assert!(contains(&bytes, b"Exif\0\0"), "fixture carries EXIF");

    let doc = open(&bytes);
    // The EXIF orientation is neither applied (no 2 × 4 rotation) …
    assert_one_background_layer(&doc, 4, 2);
    // … nor carried into what the document writes back out.
    let composite = compose(doc.layers());
    for out in [
        to_png_bytes(&composite, 6).expect("png"),
        to_jpeg_bytes(&composite, 90).expect("jpeg"),
    ] {
        assert!(!contains(&out, b"Exif"), "exported bytes carry EXIF");
        assert!(!contains(&out, b"eXIf"), "exported bytes carry EXIF");
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}
