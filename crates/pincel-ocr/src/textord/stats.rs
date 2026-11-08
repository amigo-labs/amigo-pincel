//! Tesseract's histogram `STATS` (`src/ccstruct/statistc.cpp`).

/// A histogram over the inclusive range `[min, max]`; values outside it
/// count in the end buckets.
#[derive(Debug, Clone)]
pub(crate) struct Stats {
    min: i32,
    buckets: Vec<i32>,
    total: i32,
}

impl Stats {
    /// `STATS(min, max)`; an empty range becomes `[0, 1]`.
    pub(crate) fn new(min: i32, max: i32) -> Self {
        let (min, max) = if max < min { (0, 1) } else { (min, max) };
        Self {
            min,
            buckets: vec![0; (max - min + 1) as usize],
            total: 0,
        }
    }

    fn max(&self) -> i32 {
        self.min + self.buckets.len() as i32 - 1
    }

    fn clip(&self, v: i32) -> usize {
        (v.clamp(self.min, self.max()) - self.min) as usize
    }

    /// `STATS::clear`.
    pub(crate) fn clear(&mut self) {
        self.buckets.fill(0);
        self.total = 0;
    }

    /// `STATS::add`.
    pub(crate) fn add(&mut self, v: i32, count: i32) {
        let i = self.clip(v);
        self.buckets[i] += count;
        self.total += count;
    }

    /// `STATS::pile_count`.
    pub(crate) fn pile_count(&self, v: i32) -> i32 {
        self.buckets[self.clip(v)]
    }

    /// `STATS::get_total`.
    pub(crate) fn total(&self) -> i32 {
        self.total
    }

    /// `STATS::mode`: the highest bucket wins ties, except bucket 0.
    pub(crate) fn mode(&self) -> i32 {
        let mut max = self.buckets[0];
        let mut index = 0;
        for i in (1..self.buckets.len()).rev() {
            if self.buckets[i] > max {
                max = self.buckets[i];
                index = i;
            }
        }
        self.min + index as i32
    }

    /// `STATS::min_bucket`.
    pub(crate) fn min_bucket(&self) -> i32 {
        if self.total == 0 {
            return self.min;
        }
        let i = self
            .buckets
            .iter()
            .position(|&c| c != 0)
            .unwrap_or(self.buckets.len());
        self.min + i as i32
    }

    /// `STATS::max_bucket`.
    pub(crate) fn max_bucket(&self) -> i32 {
        if self.total == 0 {
            return self.min;
        }
        let mut i = self.buckets.len() - 1;
        while i > 0 && self.buckets[i] == 0 {
            i -= 1;
        }
        self.min + i as i32
    }

    /// `STATS::ile`: the value below which `frac` of the samples lie.
    pub(crate) fn ile(&self, frac: f64) -> f64 {
        if self.total == 0 {
            return f64::from(self.min);
        }
        let target = (frac * f64::from(self.total)).clamp(1.0, f64::from(self.total));
        let mut sum = 0;
        let mut index = 0usize;
        while index < self.buckets.len() && f64::from(sum) < target {
            sum += self.buckets[index];
            index += 1;
        }
        if index > 0 {
            f64::from(self.min) + index as f64
                - (f64::from(sum) - target) / f64::from(self.buckets[index - 1])
        } else {
            f64::from(self.min)
        }
    }

    /// `STATS::mean`.
    pub(crate) fn mean(&self) -> f64 {
        if self.total <= 0 {
            return f64::from(self.min);
        }
        let sum: i64 = self
            .buckets
            .iter()
            .enumerate()
            .map(|(i, &c)| i as i64 * i64::from(c))
            .sum();
        sum as f64 / f64::from(self.total) + f64::from(self.min)
    }

    /// `STATS::median`.
    pub(crate) fn median(&self) -> f64 {
        let median = self.ile(0.5);
        let median_pile = median.floor() as i32;
        if self.total > 1 && self.pile_count(median_pile) == 0 {
            let mut min_pile = median_pile;
            while self.pile_count(min_pile) == 0 && min_pile > self.min {
                min_pile -= 1;
            }
            let mut max_pile = median_pile;
            while self.pile_count(max_pile) == 0 && max_pile < self.max() {
                max_pile += 1;
            }
            return f64::from(min_pile + max_pile) / 2.0;
        }
        median
    }

    /// `STATS::top_n_modes`: up to `max_modes` peaks as `(mean, count)`,
    /// by decreasing count.
    pub(crate) fn top_n_modes(&self, max_modes: usize) -> Vec<(f32, i32)> {
        let mut modes: Vec<(f32, i32)> = Vec::new();
        if max_modes == 0 {
            return modes;
        }
        let n = self.buckets.len();
        let mut used = vec![0i32; n];
        let mut least_count = 1;
        loop {
            let mut max_count = 0;
            let mut max_index = 0usize;
            for (i, (&b, &u)) in self.buckets.iter().zip(&used).enumerate() {
                if b - u > max_count {
                    max_count = b - u;
                    max_index = i;
                }
            }
            if max_count <= 0 {
                break;
            }
            used[max_index] = max_count;
            let mut total_value = (max_index as i32 * max_count) as f64;
            let mut total_count = max_count;
            let mut gather = |index: usize, prev: &mut i32, tc: &mut i32, tv: &mut f64| {
                let pile = self.buckets[index] - used[index];
                if pile <= *prev && pile > 0 {
                    *tc += pile;
                    *tv += (index as i32 * pile) as f64;
                    used[index] = self.buckets[index];
                    *prev = pile;
                    true
                } else {
                    false
                }
            };
            let mut prev = max_count;
            let mut offset = 1;
            while max_index + offset < n {
                if !gather(
                    max_index + offset,
                    &mut prev,
                    &mut total_count,
                    &mut total_value,
                ) {
                    break;
                }
                offset += 1;
            }
            prev = self.buckets[max_index];
            let mut offset = 1;
            while offset <= max_index {
                if !gather(
                    max_index - offset,
                    &mut prev,
                    &mut total_count,
                    &mut total_value,
                ) {
                    break;
                }
                offset += 1;
            }
            if total_count > least_count || modes.len() < max_modes {
                if modes.len() == max_modes {
                    modes.truncate(max_modes - 1);
                }
                let mut target = 0;
                while target < modes.len() && modes[target].1 >= total_count {
                    target += 1;
                }
                let mean = (total_value / f64::from(total_count) + f64::from(self.min)) as f32;
                modes.insert(target, (mean, total_count));
                least_count = modes.last().map_or(1, |m| m.1);
            }
        }
        modes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_median_of_even_count_interpolates_gap() {
        let mut s = Stats::new(0, 10);
        s.add(2, 1);
        s.add(6, 1);
        assert_eq!(s.median(), 4.0);
    }
}
