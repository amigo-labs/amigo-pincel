//! Row loops for the kernels whose rows are independent: on all cores with
//! the `parallel` feature (the desktop app's native job backend,
//! docs/specs/performance-desktop-jobs.md), one after another without it
//! (the wasm builds). Each row runs the same code either way, so the
//! results are byte-identical.

/// Calls `f(y, row)` for each `row_len`-long row of `out`, top to bottom
/// (in parallel with the `parallel` feature). A `row_len` of 0 calls
/// nothing.
pub(crate) fn for_each_row<T: Send>(
    out: &mut [T],
    row_len: usize,
    f: impl Fn(usize, &mut [T]) + Sync + Send,
) {
    if row_len == 0 {
        return;
    }
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        out.par_chunks_mut(row_len)
            .enumerate()
            .for_each(|(y, row)| f(y, row));
    }
    #[cfg(not(feature = "parallel"))]
    out.chunks_mut(row_len)
        .enumerate()
        .for_each(|(y, row)| f(y, row));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_each_row_visits_every_row_with_its_index() {
        let mut data = vec![0usize; 12];
        for_each_row(&mut data, 3, |y, row| {
            for (x, v) in row.iter_mut().enumerate() {
                *v = y * 10 + x;
            }
        });
        assert_eq!(data, vec![0, 1, 2, 10, 11, 12, 20, 21, 22, 30, 31, 32]);
    }

    #[test]
    fn for_each_row_with_zero_row_len_calls_nothing() {
        let mut data: Vec<u8> = Vec::new();
        for_each_row(&mut data, 0, |_, _| panic!("no rows"));
    }
}
