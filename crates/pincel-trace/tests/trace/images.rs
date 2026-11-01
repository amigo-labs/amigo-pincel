//! Test images (`docs/specs/vectorize-core.md`, Technical notes). The
//! generators match the reference implementation's tests bit for bit,
//! including JavaScript's arithmetic, so the golden corpus can be generated
//! from the same definitions.

/// An RGBA8 image.
pub struct Img {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Img {
    pub fn new(w: u32, h: u32, f: impl Fn(i64, i64) -> [u8; 4]) -> Self {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..i64::from(h) {
            for x in 0..i64::from(w) {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        Self { w, h, rgba }
    }

    pub fn filled(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn bw(on: bool) -> [u8; 4] {
    if on { BLACK } else { WHITE }
}

fn in_disc(x: i64, y: i64, cx: i64, cy: i64, r: i64) -> bool {
    (x - cx).pow(2) + (y - cy).pow(2) <= r * r
}

/// The reference tests' generator: `seed = (seed * 1103515245 + 12345) &
/// 0x7fffffff` evaluated as JavaScript does — an `f64` product and sum,
/// then ToInt32 — and `rnd = seed / 0x7fffffff`.
pub struct JsRng(i64);

impl JsRng {
    pub fn new(seed: i64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> f64 {
        let x = self.0 as f64 * 1_103_515_245.0 + 12_345.0;
        let m = x.trunc().rem_euclid(4_294_967_296.0) as u64;
        self.0 = (m & 0x7fff_ffff) as i64;
        self.0 as f64 / f64::from(0x7fff_ffff)
    }
}

/// A value stored into JavaScript's `Uint8ClampedArray`.
pub fn clamped(v: f64) -> u8 {
    v.clamp(0.0, 255.0).round_ties_even() as u8
}

pub fn disc() -> Img {
    Img::new(100, 100, |x, y| bw(in_disc(x, y, 50, 50, 30)))
}

pub fn ring() -> Img {
    Img::new(120, 120, |x, y| {
        let d = (x - 60).pow(2) + (y - 60).pow(2);
        bw((400..=2025).contains(&d))
    })
}

pub fn rectangle() -> Img {
    Img::new(80, 60, |x, y| {
        bw((10..70).contains(&x) && (10..50).contains(&y))
    })
}

pub fn square_with_stray_pixel() -> Img {
    Img::new(60, 60, |x, y| {
        bw(((10..40).contains(&x) && (10..40).contains(&y)) || (x == 50 && y == 50))
    })
}

pub fn four_colour() -> Img {
    Img::new(200, 150, |x, y| {
        if in_disc(x, y, 60, 75, 40) {
            [46, 134, 171, 255]
        } else if x > 120 && x < 175 && y > 20 && y < 70 {
            [192, 57, 43, 255]
        } else if x > 120 && x < 175 && y > 90 && y < 130 {
            [39, 174, 96, 255]
        } else {
            [244, 239, 230, 255]
        }
    })
}

pub fn noisy_two_tone() -> Img {
    let mut rng = JsRng::new(42);
    let (w, h) = (200_u32, 150_u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..i64::from(h) {
        for x in 0..i64::from(w) {
            let base = if in_disc(x, y, 60, 75, 40) {
                40.0
            } else {
                220.0
            };
            let v = clamped(base + (rng.next() * 30.0 - 15.0));
            rgba.extend_from_slice(&[v, v, v, 255]);
        }
    }
    Img { w, h, rgba }
}

pub fn speckled_two_tone() -> Img {
    let mut img = noisy_two_tone();
    let mut rng = JsRng::new(7);
    for px in img.rgba.chunks_exact_mut(4) {
        if rng.next() < 0.04 {
            let v = if rng.next() < 0.5 { 0 } else { 255 };
            px[0] = v;
            px[1] = v;
            px[2] = v;
        }
    }
    img
}

/// One pixel, fully transparent.
pub fn transparent_1x1() -> Img {
    Img::new(1, 1, |_, _| [0, 0, 0, 0])
}
