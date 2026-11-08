//! Activation functions and vector helpers, as in Tesseract's
//! `src/lstm/functions.h` with `TFloat = float` (`FAST_FLOAT`).

use crate::functions_table::{LOGISTIC_F64, TANH_F64};

const TABLE_SIZE: usize = 4096;
const SCALE_FACTOR: f32 = 256.0;

const fn to_f32(table: &[f64; TABLE_SIZE]) -> [f32; TABLE_SIZE] {
    let mut out = [0.0f32; TABLE_SIZE];
    let mut i = 0;
    while i < TABLE_SIZE {
        out[i] = table[i] as f32;
        i += 1;
    }
    out
}

static TANH_TABLE: [f32; TABLE_SIZE] = to_f32(&TANH_F64);
static LOGISTIC_TABLE: [f32; TABLE_SIZE] = to_f32(&LOGISTIC_F64);

fn lookup(table: &[f32; TABLE_SIZE], x: f32) -> f32 {
    let x = x * SCALE_FACTOR;
    // `static_cast<unsigned>(x)`; `x` is non-negative here. NaN maps to 0.
    let index = x as u32 as usize;
    if index >= TABLE_SIZE - 1 {
        return 1.0;
    }
    let i0 = table[index];
    let i1 = table[index + 1];
    i0 + (i1 - i0) * (x - index as f32)
}

/// `Tanh` from the lookup table.
pub(crate) fn tanh(x: f32) -> f32 {
    if x < 0.0 {
        return -tanh(-x);
    }
    lookup(&TANH_TABLE, x)
}

/// `Logistic` from the lookup table.
pub(crate) fn logistic(x: f32) -> f32 {
    if x < 0.0 {
        return 1.0 - logistic(-x);
    }
    lookup(&LOGISTIC_TABLE, x)
}

/// `SoftmaxInPlace<float>`.
pub(crate) fn softmax_in_place(v: &mut [f32]) {
    const MAX_ACTIVATION: f32 = 86.0;
    let Some(&first) = v.first() else {
        return;
    };
    let max = v.iter().fold(first, |m, &x| if x > m { x } else { m });
    let mut total = 0.0f32;
    for x in v.iter_mut() {
        let p = (*x - max).clamp(-MAX_ACTIVATION, 0.0).exp();
        total += p;
        *x = p;
    }
    if total > 0.0 {
        for x in v.iter_mut() {
            *x /= total;
        }
    }
}

/// `IntCastRounded(float)`: rounds half away from zero.
pub(crate) fn round_f32(x: f32) -> i32 {
    if x >= 0.0 {
        (x + 0.5) as i32
    } else {
        -((-x + 0.5) as i32)
    }
}

/// `IntCastRounded(double)`.
pub(crate) fn round_f64(x: f64) -> i32 {
    if x >= 0.0 {
        (x + 0.5) as i32
    } else {
        -((-x + 0.5) as i32)
    }
}

/// Tesseract's `TRand`: a 64-bit linear congruential generator. The
/// recognizer seeds it identically for every line, so its "random" padding
/// is deterministic.
#[derive(Debug, Clone)]
pub(crate) struct TRand {
    seed: u64,
}

impl TRand {
    pub(crate) fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub(crate) fn int_rand(&mut self) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.seed >> 33) as i32
    }

    pub(crate) fn signed_rand(&mut self, range: f64) -> f64 {
        range * 2.0 * f64::from(self.int_rand()) / f64::from(i32::MAX) - range
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tanh_matches_table_at_grid_points() {
        assert_eq!(tanh(0.0), 0.0);
        assert_eq!(tanh(1.0 / 256.0), TANH_F64[1] as f32);
        assert_eq!(tanh(-1.0 / 256.0), -(TANH_F64[1] as f32));
        assert_eq!(tanh(100.0), 1.0);
    }

    #[test]
    fn logistic_of_zero_is_half() {
        assert_eq!(logistic(0.0), 0.5);
        assert_eq!(logistic(-100.0), 0.0);
    }

    #[test]
    fn round_f32_rounds_half_away_from_zero() {
        assert_eq!(round_f32(2.5), 3);
        assert_eq!(round_f32(-2.5), -3);
        assert_eq!(round_f32(-0.4), 0);
    }

    #[test]
    fn softmax_in_place_sums_to_one() {
        let mut v = [1.0, 2.0, 3.0];
        softmax_in_place(&mut v);
        assert!((v.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert!(v[2] > v[1] && v[1] > v[0]);
    }
}
