//! A fully transparent pixel's colour is invisible, so no spatial effect may
//! let it tint the opaque pixels around it (docs/specs/effects-adjustments.md
//! AC-18).

use pincel_effects::{EffectImage, EffectSpec};

const W: u32 = 9;
const H: u32 = 9;
/// The opaque pixels: a neutral mid gray.
const GRAY: [u8; 4] = [128, 128, 128, 255];

/// Every spatial effect (blur, sharpen, distort, reduce noise), with
/// parameters whose reach crosses the transparent column.
const SPATIAL: &[(&str, &[f64])] = &[
    ("gaussian_blur", &[2.0]),
    ("box_blur", &[3.0, 3.0]),
    ("motion_blur", &[6.0, 0.0]),
    ("radial_blur", &[30.0, 0.5, 0.5, 0.0]),
    ("radial_blur", &[30.0, 0.5, 0.5, 1.0]),
    ("sharpen", &[]),
    ("unsharp_mask", &[200.0, 2.0, 0.0]),
    ("emboss", &[45.0, 45.0, 1.0]),
    ("edge_detect", &[0.0, 100.0]),
    ("edge_detect", &[1.0, 100.0]),
    ("edge_detect", &[2.0, 100.0]),
    ("relief", &[45.0, 1.0]),
    ("reduce_noise", &[1.0]),
    ("reduce_noise", &[2.0]),
];

/// A gray image whose middle column and top-left corner are fully
/// transparent, their (invisible) colour being `hidden`.
fn gray_with_hole(hidden: [u8; 3]) -> EffectImage {
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            if is_hole(x, y) {
                data.extend_from_slice(&[hidden[0], hidden[1], hidden[2], 0]);
            } else {
                data.extend_from_slice(&GRAY);
            }
        }
    }
    EffectImage::from_rgba8(W, H, data).expect("valid buffer")
}

fn is_hole(x: u32, y: u32) -> bool {
    x == W / 2 || (x < 2 && y < 2)
}

/// RGBA of every opaque source pixel, in row order.
fn opaque_pixels(img: &EffectImage) -> Vec<(u32, u32, [u8; 4])> {
    let mut out = Vec::new();
    for y in 0..H {
        for x in 0..W {
            if !is_hole(x, y) {
                let o = ((y * W + x) * 4) as usize;
                let p = &img.data()[o..o + 4];
                out.push((x, y, [p[0], p[1], p[2], p[3]]));
            }
        }
    }
    out
}

#[test]
fn spatial_effects_ignore_the_colour_of_transparent_pixels() {
    for &(name, params) in SPATIAL {
        let spec = EffectSpec::from_params(name, params).expect("valid effect");
        let reference = opaque_pixels(&spec.run(&gray_with_hole([0, 0, 0])));
        for hidden in [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255]] {
            let got = opaque_pixels(&spec.run(&gray_with_hole(hidden)));
            assert_eq!(
                got, reference,
                "{name} {params:?}: transparent colour {hidden:?} changed opaque pixels"
            );
        }
    }
}

#[test]
fn spatial_effects_keep_gray_neighbours_of_transparent_red_pixels_gray() {
    for &(name, params) in SPATIAL {
        let spec = EffectSpec::from_params(name, params).expect("valid effect");
        for (x, y, [r, g, b, _]) in opaque_pixels(&spec.run(&gray_with_hole([255, 0, 0]))) {
            assert!(
                r == g && g == b,
                "{name} {params:?}: pixel ({x}, {y}) tinted to ({r}, {g}, {b})"
            );
        }
    }
}
