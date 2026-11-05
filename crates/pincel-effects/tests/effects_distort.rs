//! docs/specs/effects-distort.md: paint.net's nine distortions (Bulge,
//! Crystalize, Dents, Frosted Glass, Morphology, Pixelate, Polar Inversion,
//! Tile Reflection, Twist) through the wire format both cores use.

use std::collections::HashSet;

use pincel_effects::{EFFECT_NAMES, EffectError, EffectImage, EffectSpec};

const NAMES: [&str; 9] = [
    "bulge",
    "crystalize",
    "dents",
    "frosted_glass",
    "morphology",
    "pixelate",
    "polar_inversion",
    "tile_reflection",
    "twist",
];

/// Edge behaviour indices on the wire.
const CLAMP: f64 = 0.0;
const WRAP: f64 = 1.0;
const REFLECT: f64 = 2.0;
const TRANSPARENT: f64 = 3.0;

fn spec(name: &str, params: &[f64]) -> EffectSpec {
    EffectSpec::from_params(name, params).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn image(w: u32, h: u32, pixels: &[[u8; 4]]) -> EffectImage {
    let data = pixels.iter().flatten().copied().collect();
    EffectImage::from_rgba8(w, h, data).expect("w * h pixels")
}

fn pixels(img: &EffectImage) -> Vec<[u8; 4]> {
    img.data().as_chunks::<4>().0.to_vec()
}

fn run(name: &str, params: &[f64], src: &EffectImage) -> EffectImage {
    spec(name, params).run(src)
}

/// The table's defaults, in wire order (Pixelate with Image mode's
/// Average).
fn defaults(name: &str) -> &'static [f64] {
    match name {
        "bulge" => &[45.0, 0.5, 0.5, CLAMP],
        "crystalize" => &[8.0, 1.0],
        "dents" => &[25.0, 50.0, 10.0, 1.0, CLAMP],
        "frosted_glass" => &[10.0, 1.0],
        "morphology" => &[0.0, 0.0, 1.0],
        "pixelate" => &[8.0, 0.0],
        "polar_inversion" => &[1.0, 0.5, 0.5, WRAP],
        "tile_reflection" => &[40.0, 30.0, 8.0, CLAMP],
        "twist" => &[30.0, 100.0, 0.5, 0.5, CLAMP],
        other => panic!("not a distortion: {other}"),
    }
}

/// Parameter vectors per distortion: defaults, both ends of every range
/// and a few values in between.
fn settings(name: &str) -> Vec<Vec<f64>> {
    let mut out = vec![defaults(name).to_vec()];
    out.extend(match name {
        "bulge" => vec![
            vec![-200.0, 0.0, 0.0, WRAP],
            vec![100.0, 1.0, 1.0, REFLECT],
            vec![-60.0, 0.3, 0.7, CLAMP],
        ],
        "crystalize" => vec![vec![2.0, 0.0], vec![250.0, 9999.0], vec![3.0, 42.0]],
        "dents" => vec![
            vec![1.0, 0.0, 0.0, 0.0, CLAMP],
            vec![200.0, 200.0, 100.0, 9999.0, WRAP],
            vec![4.0, 120.0, 60.0, 5.0, REFLECT],
        ],
        "frosted_glass" => vec![vec![1.0, 0.0], vec![200.0, 9999.0], vec![3.0, 17.0]],
        "morphology" => vec![
            vec![1.0, 0.0, 1.0],
            vec![2.0, 1.0, 2.0],
            vec![3.0, 1.0, 100.0],
            vec![0.0, 1.0, 3.0],
        ],
        "pixelate" => vec![
            vec![2.0, 1.0],
            vec![200.0, 0.0],
            vec![3.0, 1.0],
            vec![5.0, 0.0],
        ],
        "polar_inversion" => vec![
            vec![-4.0, 0.0, 0.0, CLAMP],
            vec![4.0, 1.0, 1.0, REFLECT],
            vec![0.37, 0.25, 0.6, WRAP],
        ],
        "tile_reflection" => vec![
            vec![2.0, 0.0, -100.0, CLAMP],
            vec![200.0, 360.0, 100.0, WRAP],
            vec![7.0, 45.0, 0.0, REFLECT],
        ],
        "twist" => vec![
            vec![-100.0, 1.0, 0.0, 0.0, CLAMP],
            vec![100.0, 200.0, 1.0, 1.0, WRAP],
            vec![55.0, 150.0, 0.4, 0.6, REFLECT],
        ],
        other => panic!("not a distortion: {other}"),
    });
    out
}

/// A 13x9 layer of varied colours, alpha between 40 and 230.
fn varied() -> EffectImage {
    let (w, h) = (13u32, 9u32);
    let px: Vec<[u8; 4]> = (0..w * h)
        .map(|i| {
            [
                (i * 53 % 256) as u8,
                ((i * 97 + 31) % 256) as u8,
                ((i * 151 + 7) % 256) as u8,
                (40 + (i * 71 % 191)) as u8,
            ]
        })
        .collect();
    image(w, h, &px)
}

/// A `w` x `h` opaque layer whose every pixel has its own colour.
fn unique(w: u32, h: u32) -> EffectImage {
    let px: Vec<[u8; 4]> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            [
                (x * 7 % 256) as u8,
                (y * 7 % 256) as u8,
                ((x + y) % 256) as u8,
                255,
            ]
        })
        .collect();
    image(w, h, &px)
}

/// A `w` x `h` opaque layer of pseudo-random noise.
fn noisy(w: u32, h: u32) -> EffectImage {
    let mut state = 0x1234_5678_u32;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state >> 24) as u8
    };
    let px: Vec<[u8; 4]> = (0..w * h).map(|_| [next(), next(), next(), 255]).collect();
    image(w, h, &px)
}

#[test]
fn distortions_are_known_wire_names_with_labels() {
    for (name, label) in NAMES.iter().zip([
        "Bulge",
        "Crystalize",
        "Dents",
        "Frosted Glass",
        "Morphology",
        "Pixelate",
        "Polar Inversion",
        "Tile Reflection",
        "Twist",
    ]) {
        assert!(EFFECT_NAMES.contains(name), "{name} missing");
        assert_eq!(spec(name, defaults(name)).label(), label);
    }
}

// AC-03
#[test]
fn every_distortion_keeps_the_size() {
    for src in [varied(), unique(1, 1), unique(2, 7)] {
        for name in NAMES {
            for params in settings(name) {
                let out = run(name, &params, &src);
                assert_eq!(
                    (out.width(), out.height()),
                    (src.width(), src.height()),
                    "{name} {params:?}"
                );
            }
        }
    }
}

// AC-03
#[test]
fn bulge_dents_and_twist_at_their_neutral_setting_change_nothing() {
    let src = varied();
    assert_eq!(run("bulge", &[0.0, 0.3, 0.6, TRANSPARENT], &src), src);
    assert_eq!(
        run("dents", &[25.0, 0.0, 80.0, 3.0, TRANSPARENT], &src),
        src
    );
    assert_eq!(
        run("twist", &[0.0, 200.0, 0.5, 0.5, TRANSPARENT], &src),
        src
    );
}

/// Morphology's operations, shapes and radii.
fn morphologies() -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    for op in 0..4 {
        for shape in 0..2 {
            for r in [1.0, 2.0, 5.0] {
                out.push([f64::from(op), f64::from(shape), r]);
            }
        }
    }
    out
}

/// `varied()` with a transparent hole and a transparent column.
fn varied_with_holes() -> EffectImage {
    let mut px = pixels(&varied());
    for (i, p) in px.iter_mut().enumerate() {
        let (x, y) = (i % 13, i / 13);
        if x == 9 || ((3..6).contains(&x) && (2..5).contains(&y)) {
            *p = [77, 66, 55, 0];
        }
    }
    image(13, 9, &px)
}

// AC-04
#[test]
fn morphology_only_picks_existing_colours() {
    let src = varied_with_holes();
    let colours: HashSet<[u8; 4]> = pixels(&src).into_iter().collect();
    for params in morphologies() {
        for p in pixels(&run("morphology", &params, &src)) {
            assert!(
                p[3] == 0 || colours.contains(&p),
                "{params:?}: {p:?} is a new colour"
            );
        }
    }
}

// AC-05
#[test]
fn morphology_dilate_grows_and_erode_shrinks_the_opaque_pixels() {
    let src = varied_with_holes();
    let before = pixels(&src);
    for params in morphologies() {
        let after = pixels(&run("morphology", &params, &src));
        for (i, (a, b)) in before.iter().zip(&after).enumerate() {
            match params[0] as u8 {
                0 => assert!(a[3] == 0 || b[3] > 0, "{params:?}: dilate lost pixel {i}"),
                1 => assert!(a[3] > 0 || b[3] == 0, "{params:?}: erode grew pixel {i}"),
                _ => {}
            }
        }
    }
}

/// The alpha mask of a 7x7 transparent layer with one opaque pixel in the
/// middle after Morphology with `params`, as rows of `#` and `.`.
fn single_pixel(params: &[f64]) -> Vec<String> {
    let mut px = vec![[0, 0, 0, 0]; 49];
    px[24] = [200, 10, 30, 255];
    let out = pixels(&run("morphology", params, &image(7, 7, &px)));
    out.chunks(7)
        .map(|row| {
            row.iter()
                .map(|p| if p[3] > 0 { '#' } else { '.' })
                .collect()
        })
        .collect()
}

// AC-05
#[test]
fn morphology_on_a_single_pixel() {
    let empty = ".......";
    let dot = "...#...";
    let bar = "..###..";
    assert_eq!(
        single_pixel(&[0.0, 0.0, 1.0]),
        [empty, empty, bar, bar, bar, empty, empty],
        "dilate square"
    );
    assert_eq!(
        single_pixel(&[0.0, 1.0, 1.0]),
        [empty, empty, dot, bar, dot, empty, empty],
        "dilate circle"
    );
    for shape in [0.0, 1.0] {
        assert_eq!(
            single_pixel(&[1.0, shape, 1.0]),
            [empty; 7],
            "erode {shape}"
        );
    }
}

#[test]
fn morphology_dilate_takes_the_brightest_neighbour_first_in_row_major_order() {
    // Two equally bright pixels: the upper-left one wins.
    let mut px = vec![[10, 10, 10, 255]; 9];
    px[2] = [0, 255, 0, 255];
    px[6] = [0, 255, 0, 254];
    let out = pixels(&run("morphology", &[0.0, 0.0, 1.0], &image(3, 3, &px)));
    assert_eq!(out[4], [0, 255, 0, 255]);
}

#[test]
fn morphology_erode_of_a_fully_opaque_layer_clears_its_edge() {
    let src = image(9, 7, &[[90, 80, 70, 255]; 63]);
    let out = pixels(&run("morphology", &[1.0, 0.0, 2.0], &src));
    for (i, p) in out.iter().enumerate() {
        let (x, y) = (i % 9, i / 9);
        let inner = (2..7).contains(&x) && (2..5).contains(&y);
        assert_eq!(p[3] > 0, inner, "({x}, {y})");
    }
}

#[test]
fn morphology_of_a_fully_transparent_layer_is_unchanged() {
    let src = image(6, 4, &[[90, 10, 200, 0]; 24]);
    for params in morphologies() {
        let out = run("morphology", &params, &src);
        assert!(pixels(&out).iter().all(|p| p[3] == 0), "{params:?}");
    }
}

/// The `cell` x `cell` squares of a `w` x `h` buffer whose top-left pixel
/// sits at canvas position `origin`, clipped to the buffer, as pixel
/// index lists.
fn grid(w: u32, h: u32, origin: (i32, i32), cell: i32) -> Vec<Vec<usize>> {
    let mut squares: std::collections::BTreeMap<(i32, i32), Vec<usize>> = Default::default();
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let key = (
                (origin.0 + x).div_euclid(cell),
                (origin.1 + y).div_euclid(cell),
            );
            squares
                .entry(key)
                .or_default()
                .push((y * w as i32 + x) as usize);
        }
    }
    squares.into_values().collect()
}

// AC-06
#[test]
fn pixelate_fills_every_square_with_one_colour() {
    let src = varied();
    for cell in [2.0, 3.0, 4.0, 8.0] {
        for colour in [0.0, 1.0] {
            let out = pixels(&run("pixelate", &[cell, colour], &src));
            for square in grid(13, 9, (0, 0), cell as i32) {
                let first = out[square[0]];
                for &i in &square {
                    assert_eq!(out[i], first, "cell {cell} colour {colour} pixel {i}");
                }
            }
        }
    }
}

/// The square's centre pixel: offset ⌊(w − 1) / 2⌋, ⌊(h − 1) / 2⌋ of the
/// square clipped to the buffer.
fn centre_of(square: &[usize], w: usize) -> usize {
    let xs: Vec<usize> = square.iter().map(|i| i % w).collect();
    let ys: Vec<usize> = square.iter().map(|i| i / w).collect();
    let (x0, x1) = (*xs.iter().min().expect("px"), *xs.iter().max().expect("px"));
    let (y0, y1) = (*ys.iter().min().expect("px"), *ys.iter().max().expect("px"));
    (y0 + (y1 - y0) / 2) * w + x0 + (x1 - x0) / 2
}

// AC-06
#[test]
fn pixelate_centre_takes_the_square_centre_pixel_and_adds_no_colour() {
    let src = varied();
    let before = pixels(&src);
    let colours: HashSet<[u8; 4]> = before.iter().copied().collect();
    for cell in [2.0, 3.0, 4.0, 8.0] {
        let out = pixels(&run("pixelate", &[cell, 1.0], &src));
        for square in grid(13, 9, (0, 0), cell as i32) {
            let centre = before[centre_of(&square, 13)];
            assert_eq!(out[square[0]], centre, "cell {cell}");
        }
        assert!(out.iter().all(|p| colours.contains(p)), "cell {cell}");
    }
}

// AC-06
#[test]
fn pixelate_grid_starts_at_the_canvas_origin() {
    // A trimmed Pixel-mode cel at canvas position (3, -2).
    let src = varied();
    let before = pixels(&src);
    let out = pixels(&spec("pixelate", &[4.0, 1.0]).run_at(&src, (3, -2), |_, _| true));
    for square in grid(13, 9, (3, -2), 4) {
        assert_eq!(out[square[0]], before[centre_of(&square, 13)]);
        assert!(square.iter().all(|&i| out[i] == out[square[0]]));
    }
    // The first square is one column wide (canvas x 3) and two rows high
    // (canvas y -2, -1).
    assert_eq!(out[0], before[0]);
    assert_ne!(out[0], out[1]);
}

#[test]
fn pixelate_average_is_the_premultiplied_mean() {
    let src = image(2, 1, &[[255, 0, 0, 255], [0, 0, 255, 0]]);
    let out = pixels(&run("pixelate", &[2.0, 0.0], &src));
    assert_eq!(out, vec![[255, 0, 0, 128]; 2]);
}

#[test]
fn pixelate_with_a_cell_larger_than_the_layer_gives_one_colour() {
    let out = pixels(&run("pixelate", &[200.0, 0.0], &varied()));
    assert!(out.iter().all(|p| *p == out[0]));
}

/// A `size` x `size` opaque black / white checkerboard of `square` px.
fn checkerboard(size: u32, square: u32) -> EffectImage {
    let px: Vec<[u8; 4]> = (0..size * size)
        .map(|i| {
            let (x, y) = (i % size / square, i / size / square);
            let v = if (x + y) % 2 == 0 { 0 } else { 255 };
            [v, v, v, 255]
        })
        .collect();
    image(size, size, &px)
}

/// Lengths of the runs of dark / light pixels along row `y`.
fn runs(img: &EffectImage, y: usize) -> Vec<usize> {
    let w = img.width() as usize;
    let row: Vec<bool> = pixels(img)[y * w..(y + 1) * w]
        .iter()
        .map(|p| p[0] >= 128)
        .collect();
    let mut out = vec![1];
    for x in 1..w {
        if row[x] == row[x - 1] {
            *out.last_mut().expect("run") += 1;
        } else {
            out.push(1);
        }
    }
    out
}

// AC-07
#[test]
fn bulge_magnifies_or_pinches_the_centre() {
    let src = checkerboard(64, 4);
    let middle = |runs: &[usize]| runs[runs.len() / 2];
    let grow = runs(&run("bulge", &[45.0, 0.5, 0.5, CLAMP], &src), 33);
    assert!(
        middle(&grow) > grow[0],
        "Amount 45: centre {} vs edge {}",
        middle(&grow),
        grow[0]
    );
    let shrink = runs(&run("bulge", &[-100.0, 0.5, 0.5, CLAMP], &src), 33);
    assert!(
        middle(&shrink) < shrink[0],
        "Amount -100: centre {} vs edge {}",
        middle(&shrink),
        shrink[0]
    );
}

// AC-08
#[test]
fn twist_rotates_inside_its_circle_only() {
    let src = unique(31, 31);
    for (amount, size) in [(30.0, 100.0), (-80.0, 50.0), (100.0, 20.0)] {
        let out = pixels(&run("twist", &[amount, size, 0.5, 0.5, CLAMP], &src));
        let before = pixels(&src);
        let radius = size / 100.0 * 15.5;
        let mut moved = 0;
        for (i, (a, b)) in before.iter().zip(&out).enumerate() {
            let (x, y) = ((i % 31) as f64 - 15.0, (i / 31) as f64 - 15.0);
            if (x * x + y * y).sqrt() >= radius {
                assert_eq!(a, b, "{amount} {size}: ({x}, {y}) outside moved");
            } else if a != b {
                moved += 1;
            }
        }
        assert_eq!(out[15 * 31 + 15], before[15 * 31 + 15], "centre");
        assert!(moved > 0, "{amount} {size}: nothing rotated");
    }
}

/// A seeded distortion's parameter vector for a seed.
type SeededParams = fn(f64) -> Vec<f64>;

// AC-09
#[test]
fn seeded_distortions_are_reproducible_per_seed() {
    let src = noisy(40, 32);
    let cases: [(&str, SeededParams); 3] = [
        ("crystalize", |s| vec![8.0, s]),
        ("dents", |s| vec![8.0, 100.0, 40.0, s, CLAMP]),
        ("frosted_glass", |s| vec![4.0, s]),
    ];
    for (name, params) in cases {
        for seed in [0.0, 1.0, 4242.0] {
            assert_eq!(
                run(name, &params(seed), &src),
                run(name, &params(seed), &src),
                "{name} seed {seed}"
            );
        }
        assert_ne!(
            run(name, &params(1.0), &src),
            run(name, &params(2.0), &src),
            "{name}: seeds 1 and 2"
        );
    }
}

// AC-09
#[test]
fn crystalize_cells_have_no_single_pixels() {
    let src = noisy(64, 48);
    for seed in 0..12 {
        let out = pixels(&run("crystalize", &[8.0, f64::from(seed)], &src));
        let (w, h) = (64usize, 48usize);
        for y in 0..h {
            for x in 0..w {
                let p = out[y * w + x];
                let same = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .any(|(dx, dy)| {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        nx >= 0
                            && ny >= 0
                            && (nx as usize) < w
                            && (ny as usize) < h
                            && out[ny as usize * w + nx as usize] == p
                    });
                assert!(same, "seed {seed}: ({x}, {y}) is alone");
            }
        }
    }
}

#[test]
fn crystalize_averages_each_cell() {
    let out = pixels(&run("crystalize", &[8.0, 1.0], &noisy(64, 48)));
    let colours: HashSet<[u8; 4]> = out.iter().copied().collect();
    // About one cell per 8 x 8 px.
    assert!(
        (20..=100).contains(&colours.len()),
        "{} cells",
        colours.len()
    );
}

// AC-10
#[test]
fn frosted_glass_copies_input_pixels_from_within_amount() {
    let src = unique(36, 30);
    let before = pixels(&src);
    let (w, h) = (36i32, 30i32);
    for (amount, seed) in [(1.0, 1.0), (3.0, 7.0), (10.0, 1.0), (10.0, 99.0)] {
        let out = pixels(&run("frosted_glass", &[amount, seed], &src));
        let a = amount as i32;
        for y in 0..h {
            for x in 0..w {
                let p = out[(y * w + x) as usize];
                let near = (-a..=a).any(|dy| {
                    (-a..=a).any(|dx| {
                        let (sx, sy) = (x + dx, y + dy);
                        dx * dx + dy * dy <= a * a
                            && (0..w).contains(&sx)
                            && (0..h).contains(&sy)
                            && before[(sy * w + sx) as usize] == p
                    })
                });
                assert!(near, "{amount}: ({x}, {y}) took {p:?} from too far");
            }
        }
        assert_ne!(out, before, "{amount}: nothing moved");
    }
}

// AC-11
#[test]
fn edge_behaviour_decides_what_a_position_outside_reads() {
    // Polar Inversion around the top-left corner of an 8 x 8 layer
    // (R = 4 px): pixel (1, 0) is z = 1/4 and maps to x = 4 · (9/16) / (1/4)
    // = 9, one pixel past the right edge.
    let src = unique(8, 8);
    let before = pixels(&src);
    let at = |edge: f64| pixels(&run("polar_inversion", &[0.5625, 0.0, 0.0, edge], &src))[1];
    assert_eq!(at(CLAMP), before[7], "clamp: nearest edge pixel");
    assert_eq!(at(WRAP), before[1], "wrap: the opposite edge");
    assert_eq!(at(REFLECT), before[6], "reflect: the mirrored position");
    assert_eq!(at(TRANSPARENT)[3], 0, "transparent");
}

// AC-11
#[test]
fn edge_behaviour_transparent_clears_only_positions_outside() {
    let src = unique(31, 31);
    let clear = |name: &str, params: &[f64]| {
        pixels(&run(name, params, &src))
            .iter()
            .filter(|p| p[3] == 0)
            .count()
    };
    // Twist with Size 200 % turns the corners outside the layer.
    assert!(clear("twist", &[60.0, 200.0, 0.5, 0.5, TRANSPARENT]) > 0);
    assert_eq!(clear("twist", &[60.0, 200.0, 0.5, 0.5, CLAMP]), 0);
    // Bulge reads inside its circle only.
    assert_eq!(clear("bulge", &[-200.0, 0.5, 0.5, TRANSPARENT]), 0);
    assert!(clear("polar_inversion", &[1.0, 0.5, 0.5, TRANSPARENT]) > 0);
    assert!(clear("tile_reflection", &[10.0, 30.0, 50.0, TRANSPARENT]) > 0);
}

// AC-12
#[test]
fn a_solid_opaque_layer_stays_unchanged() {
    let src = image(12, 10, &[[123, 45, 200, 255]; 120]);
    for name in NAMES {
        for params in settings(name) {
            if name == "morphology" && params[0] != 0.0 {
                continue;
            }
            assert_eq!(run(name, &params, &src), src, "{name} {params:?}");
        }
    }
}

// AC-12: Morphology's outside always counts as transparent — its fixed
// edge behaviour (Terms, edge cases) — so Erode, Open and Close clear
// pixels near the layer's edge only, as Transparent does for the others.
#[test]
fn morphology_on_a_solid_layer_changes_only_pixels_near_the_edge() {
    let (w, h) = (12usize, 10usize);
    let colour = [123, 45, 200, 255];
    let src = image(w as u32, h as u32, &vec![colour; w * h]);
    for params in morphologies() {
        let out = pixels(&run("morphology", &params, &src));
        let r = params[2] as usize;
        for (i, p) in out.iter().enumerate() {
            let (x, y) = (i % w, i / w);
            let depth = x.min(y).min(w - 1 - x).min(h - 1 - y);
            if params[0] == 0.0 || depth >= r {
                assert_eq!(*p, colour, "{params:?} ({x}, {y})");
            } else {
                assert!(*p == colour || p[3] == 0, "{params:?} ({x}, {y}): {p:?}");
            }
        }
    }
}

// AC-13
#[test]
fn wrong_parameter_count_is_rejected() {
    for name in NAMES {
        let n = defaults(name).len();
        for got in [0, n - 1, n + 1] {
            assert!(
                matches!(
                    EffectSpec::from_params(name, &vec![1.0; got]),
                    Err(EffectError::ParamCount { expected, got: g, .. }) if expected == n && g == got
                ),
                "{name} with {got}"
            );
        }
    }
}

// AC-13
#[test]
fn non_finite_numbers_read_as_zero_then_clamp_to_the_range() {
    let same = |name: &str, a: &[f64], b: &[f64]| {
        assert_eq!(spec(name, a), spec(name, b), "{name} {a:?} vs {b:?}");
    };
    let nan = f64::NAN;
    let inf = f64::INFINITY;
    same("bulge", &[nan, nan, inf, nan], &[0.0, 0.0, 0.0, CLAMP]);
    same(
        "bulge",
        &[500.0, 2.0, -1.0, 9.0],
        &[100.0, 1.0, 0.0, TRANSPARENT],
    );
    same(
        "bulge",
        &[-500.0, 0.5, 0.5, -2.0],
        &[-200.0, 0.5, 0.5, CLAMP],
    );
    same("crystalize", &[nan, inf], &[2.0, 0.0]);
    same("crystalize", &[999.0, 20000.0], &[250.0, 9999.0]);
    same(
        "dents",
        &[nan, nan, nan, nan, nan],
        &[1.0, 0.0, 0.0, 0.0, CLAMP],
    );
    same(
        "dents",
        &[900.0, 900.0, 900.0, 1e9, 1.4],
        &[200.0, 200.0, 100.0, 9999.0, WRAP],
    );
    same("frosted_glass", &[nan, nan], &[1.0, 0.0]);
    same("frosted_glass", &[999.0, -5.0], &[200.0, 0.0]);
    same("morphology", &[nan, nan, nan], &[0.0, 0.0, 1.0]);
    same("morphology", &[7.0, 7.0, 999.0], &[3.0, 1.0, 100.0]);
    same("pixelate", &[nan, nan], &[2.0, 0.0]);
    same("pixelate", &[999.0, 5.0], &[200.0, 1.0]);
    same(
        "polar_inversion",
        &[nan, nan, nan, nan],
        &[0.0, 0.0, 0.0, CLAMP],
    );
    same(
        "polar_inversion",
        &[9.0, 3.0, -3.0, 3.0],
        &[4.0, 1.0, 0.0, TRANSPARENT],
    );
    same(
        "polar_inversion",
        &[-9.0, 0.5, 0.5, 1.0],
        &[-4.0, 0.5, 0.5, WRAP],
    );
    same(
        "tile_reflection",
        &[nan, nan, nan, nan],
        &[2.0, 0.0, 0.0, CLAMP],
    );
    same(
        "tile_reflection",
        &[999.0, 370.0, 999.0, 2.0],
        &[200.0, 10.0, 100.0, REFLECT],
    );
    same(
        "tile_reflection",
        &[40.0, -10.0, -999.0, 0.0],
        &[40.0, 350.0, -100.0, CLAMP],
    );
    same(
        "twist",
        &[nan, nan, nan, nan, nan],
        &[0.0, 1.0, 0.0, 0.0, CLAMP],
    );
    same(
        "twist",
        &[999.0, 999.0, 5.0, -5.0, 9.0],
        &[100.0, 200.0, 1.0, 0.0, TRANSPARENT],
    );
}
