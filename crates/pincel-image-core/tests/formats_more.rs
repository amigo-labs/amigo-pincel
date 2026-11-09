//! TGA, DDS and ICO (docs/specs/image-formats-more.md): files built here
//! byte by byte, so the fixtures stay readable and need no binary blobs.

use pincel_image_core::codec::{
    CodecError, ICO_MAX_SIDE, decode, decode_hinted, to_ico_bytes, to_png_bytes, to_tga_bytes,
};
use pincel_image_core::{Color, ImageBuffer};
use proptest::prelude::*;

/// A TGA header for an image of `w` × `h` with origin at the top left.
fn tga_header(
    kind: u8,
    cmap: Option<(u16, u8)>,
    w: u16,
    h: u16,
    bpp: u8,
    alpha_bits: u8,
) -> Vec<u8> {
    let mut v = vec![0u8, u8::from(cmap.is_some()), kind];
    let (len, entry) = cmap.unwrap_or((0, 0));
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&len.to_le_bytes());
    v.push(entry);
    v.extend_from_slice(&[0, 0, 0, 0]);
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.push(bpp);
    v.push(0x20 | alpha_bits);
    v
}

const PIXELS: [[u8; 4]; 4] = [
    [255, 0, 0, 255],
    [0, 255, 0, 128],
    [0, 0, 255, 0],
    [10, 20, 30, 255],
];

fn expect_pixels(buf: &ImageBuffer, want: &[[u8; 4]]) {
    assert_eq!((buf.width(), buf.height()), (2, 2));
    for (i, w) in want.iter().enumerate() {
        let c = buf.get_pixel(i as u32 % 2, i as u32 / 2).unwrap();
        assert_eq!([c.r, c.g, c.b, c.a], *w, "pixel {i}");
    }
}

#[test]
fn tga_uncompressed_rle_grayscale_and_color_mapped_open_with_a_hint_only() {
    // Uncompressed 32-bit true-colour (BGRA).
    let mut raw = tga_header(2, None, 2, 2, 32, 8);
    for [r, g, b, a] in PIXELS {
        raw.extend_from_slice(&[b, g, r, a]);
    }
    expect_pixels(&decode_hinted(&raw, Some("shot.TGA")).unwrap(), &PIXELS);
    // Without the hint TGA is not recognised (AC-04).
    assert!(decode(&raw).is_err());

    // RLE: one run of 3 red pixels, then one raw pixel.
    let mut rle = tga_header(10, None, 2, 2, 32, 8);
    rle.extend_from_slice(&[0x82, 0, 0, 255, 255, 0x00, 30, 20, 10, 255]);
    let red = [255, 0, 0, 255];
    expect_pixels(
        &decode_hinted(&rle, Some("image/x-targa")).unwrap(),
        &[red, red, red, [10, 20, 30, 255]],
    );

    // 8-bit grayscale.
    let mut gray = tga_header(3, None, 2, 2, 8, 0);
    gray.extend_from_slice(&[0, 85, 170, 255]);
    let g = |v: u8| [v, v, v, 255];
    expect_pixels(
        &decode_hinted(&gray, Some("g.tga")).unwrap(),
        &[g(0), g(85), g(170), g(255)],
    );

    // Colour-mapped: a 24-bit palette of two entries.
    let mut mapped = tga_header(1, Some((2, 24)), 2, 2, 8, 0);
    mapped.extend_from_slice(&[0, 0, 255, 255, 0, 0]); // red, blue (BGR)
    mapped.extend_from_slice(&[0, 1, 1, 0]);
    let blue = [0, 0, 255, 255];
    expect_pixels(
        &decode_hinted(&mapped, Some("m.tga")).unwrap(),
        &[red, blue, blue, red],
    );
}

/// A DDS file of `w` × `h` with FourCC `four_cc` (and a DX10 header with
/// `dxgi` when the FourCC is `DX10`), followed by `data`.
fn dds(w: u32, h: u32, four_cc: &[u8; 4], dxgi: Option<u32>, data: &[u8]) -> Vec<u8> {
    let mut v = b"DDS ".to_vec();
    let mut header = [0u8; 124];
    header[0..4].copy_from_slice(&124u32.to_le_bytes());
    header[4..8].copy_from_slice(&0x0008_1007u32.to_le_bytes());
    header[8..12].copy_from_slice(&h.to_le_bytes());
    header[12..16].copy_from_slice(&w.to_le_bytes());
    header[72..76].copy_from_slice(&32u32.to_le_bytes());
    header[76..80].copy_from_slice(&4u32.to_le_bytes());
    header[80..84].copy_from_slice(four_cc);
    header[104..108].copy_from_slice(&0x1000u32.to_le_bytes());
    v.extend_from_slice(&header);
    if let Some(format) = dxgi {
        for word in [format, 3, 0, 1, 0] {
            v.extend_from_slice(&word.to_le_bytes());
        }
    }
    v.extend_from_slice(data);
    v
}

/// One BC1 colour block of solid red.
const BC1_RED: [u8; 8] = [0x00, 0xF8, 0x00, 0x00, 0, 0, 0, 0];

#[test]
fn dds_bc1_bc2_bc3_and_dx10_open_whatever_the_extension() {
    let red = Color::rgba(255, 0, 0, 255);
    let mut bc2 = vec![0xFF; 8];
    bc2.extend_from_slice(&BC1_RED);
    let mut bc3 = vec![255, 255, 0, 0, 0, 0, 0, 0];
    bc3.extend_from_slice(&BC1_RED);
    for file in [
        dds(4, 4, b"DXT1", None, &BC1_RED),
        dds(4, 4, b"DXT3", None, &bc2),
        dds(4, 4, b"DXT5", None, &bc3),
        dds(4, 4, b"DX10", Some(71), &BC1_RED),
    ] {
        let buf = decode_hinted(&file, Some("texture.png")).expect("dds");
        assert_eq!((buf.width(), buf.height()), (4, 4));
        assert_eq!(buf.get_pixel(3, 3), Some(red));
    }
}

#[test]
fn dds_with_an_unsupported_variant_or_size_fails_with_its_own_reason() {
    let bc7 = dds(4, 4, b"DX10", Some(98), &[0; 16]);
    assert!(matches!(decode(&bc7), Err(CodecError::DdsUnsupported)));
    let blocks = BC1_RED.repeat(64);
    let odd = dds(30, 30, b"DXT1", None, &blocks);
    assert!(matches!(
        decode(&odd),
        Err(CodecError::DdsSize {
            width: 30,
            height: 30
        })
    ));
    // Valid header, data cut off: an ordinary decode error (AC-12).
    let cut = dds(8, 8, b"DXT1", None, &BC1_RED);
    assert!(matches!(decode(&cut), Err(CodecError::Decode(_))));
}

#[test]
fn ico_opens_the_deepest_then_largest_entry() {
    let png = |w: u32, c: Color| {
        let mut b = ImageBuffer::new_transparent(w, w);
        for y in 0..w {
            for x in 0..w {
                b.set_pixel(x, y, c);
            }
        }
        to_png_bytes(&b, 6).unwrap()
    };
    let entries = [
        (64u32, 8u16, png(64, Color::rgba(1, 1, 1, 255))),
        (32, 32, png(32, Color::rgba(2, 2, 2, 255))),
        (16, 32, png(16, Color::rgba(3, 3, 3, 255))),
    ];
    let mut v = vec![0, 0, 1, 0];
    v.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * entries.len() as u32;
    for (w, bpp, data) in &entries {
        v.extend_from_slice(&[*w as u8, *w as u8, 0, 0]);
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&bpp.to_le_bytes());
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(&offset.to_le_bytes());
        offset += data.len() as u32;
    }
    for (_, _, data) in &entries {
        v.extend_from_slice(data);
    }
    let buf = decode_hinted(&v, Some("app.bin")).expect("ico");
    assert_eq!(
        (buf.width(), buf.get_pixel(0, 0)),
        (32, Some(Color::rgba(2, 2, 2, 255)))
    );
}

#[test]
fn ico_refuses_more_than_256_px_and_garbage_fails_to_decode() {
    assert!(matches!(
        to_ico_bytes(&ImageBuffer::new_transparent(ICO_MAX_SIDE + 1, 4)),
        Err(CodecError::IcoTooLarge)
    ));
    assert!(to_ico_bytes(&ImageBuffer::new_transparent(256, 256)).is_ok());
    assert!(decode_hinted(&[0, 0, 2, 0, 9, 9], Some("x.tga")).is_err());
}

fn image_strategy() -> impl Strategy<Value = ImageBuffer> {
    (1u32..=64, 1u32..=64).prop_flat_map(|(w, h)| {
        proptest::collection::vec(any::<u8>(), (w * h * 4) as usize)
            .prop_map(move |data| ImageBuffer::from_raw(w, h, data).unwrap())
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// AC-05: TGA and ICO written by Pincel decode back to the exact RGBA,
    /// colour values of fully transparent pixels included.
    #[test]
    fn tga_and_ico_round_trip_exactly(img in image_strategy()) {
        let tga = to_tga_bytes(&img).unwrap();
        prop_assert_eq!(decode_hinted(&tga, Some("a.tga")).unwrap(), img.clone());
        let ico = to_ico_bytes(&img).unwrap();
        prop_assert_eq!(decode(&ico).unwrap(), img);
    }
}

#[test]
fn tga_and_ico_round_trip_a_single_transparent_pixel() {
    let img = ImageBuffer::from_raw(1, 1, vec![12, 34, 56, 0]).unwrap();
    assert_eq!(
        decode_hinted(&to_tga_bytes(&img).unwrap(), Some("a.tga")).unwrap(),
        img
    );
    assert_eq!(decode(&to_ico_bytes(&img).unwrap()).unwrap(), img);
}

/// AC-05 at the largest ICO: a 256 × 256 image with varied colours and
/// alphas (fully transparent pixels with colour among them) round-trips
/// through TGA and through ICO to the exact RGBA.
#[test]
fn tga_and_ico_round_trip_a_256_px_image_exactly() {
    let mut data = Vec::with_capacity(256 * 256 * 4);
    for y in 0..256u32 {
        for x in 0..256u32 {
            let a = if (x + y) % 7 == 0 {
                0
            } else {
                ((x * 3 + y) % 256) as u8
            };
            data.extend_from_slice(&[x as u8, y as u8, (x ^ y) as u8, a]);
        }
    }
    let img = ImageBuffer::from_raw(256, 256, data).unwrap();
    assert_eq!(
        decode_hinted(&to_tga_bytes(&img).unwrap(), Some("big.tga")).unwrap(),
        img
    );
    assert_eq!(decode(&to_ico_bytes(&img).unwrap()).unwrap(), img);
}
