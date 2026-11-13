//! Maps over independent items — the layer PNGs of an OpenRaster file — on
//! all cores with the `parallel` feature (built for the desktop app's native
//! job backend, removed 2026-11-13: docs/specs/performance-desktop-jobs.md),
//! one after another without it (the wasm builds). Each item runs the same
//! code either way and the results keep their order, so the output is
//! byte-identical.

/// `f` applied to every item of `items`, in order.
pub(crate) fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        items.par_iter().map(f).collect()
    }
    #[cfg(not(feature = "parallel"))]
    items.iter().map(f).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_keeps_the_items_order() {
        let items: Vec<u32> = (0..100).collect();
        assert_eq!(
            map(&items, |i| i * 2),
            (0..100).map(|i| i * 2).collect::<Vec<_>>()
        );
    }
}
