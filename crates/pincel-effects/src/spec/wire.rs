//! Reading the wire's flat `f64` parameter vector: arity checks and the
//! number conventions every effect shares (non-finite reads as 0).

use super::EffectError;
use crate::distort::EdgeBehavior;

pub(super) fn expect_len(name: &str, params: &[f64], expected: usize) -> Result<(), EffectError> {
    if params.len() == expected {
        Ok(())
    } else {
        Err(EffectError::ParamCount {
            effect: name.to_owned(),
            expected,
            got: params.len(),
        })
    }
}

pub(super) fn f(v: f64) -> f32 {
    if v.is_finite() { v as f32 } else { 0.0 }
}

pub(super) fn u(v: f64) -> u32 {
    if v.is_finite() {
        v.round().clamp(0.0, u32::MAX as f64) as u32
    } else {
        0
    }
}

/// A finite number clamped to `lo..=hi`; non-finite reads as 0 first.
pub(super) fn clamped(v: f64, lo: f32, hi: f32) -> f32 {
    f(v).clamp(lo, hi)
}

pub(super) fn b(v: f64) -> bool {
    v.is_finite() && v != 0.0
}

/// A choice index: non-finite reads as 0, then rounded and clamped to
/// `0..=last`.
pub(super) fn option(v: f64, last: u32) -> u32 {
    u(v.max(0.0)).min(last)
}

/// A resampling distortion's edge behaviour slot.
pub(super) fn edge(v: f64) -> EdgeBehavior {
    EdgeBehavior::from_param(f(v))
}
