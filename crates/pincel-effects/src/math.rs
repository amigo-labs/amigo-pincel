//! Float functions with the same results on every target: `libm` is the
//! code wasm32 runs for `std`'s float functions, so a native build that
//! calls it computes the browser's bytes (docs/specs/pincel.md §15,
//! 2026-11-11; docs/specs/performance-desktop-jobs.md AC-01).

/// `(sin x, cos x)`, as `f32::sin_cos` computes it on wasm32: each
/// function on its own.
#[inline]
pub(crate) fn sin_cos(x: f32) -> (f32, f32) {
    (libm::sinf(x), libm::cosf(x))
}
