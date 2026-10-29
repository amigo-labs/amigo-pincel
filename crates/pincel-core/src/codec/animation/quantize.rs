//! Median-cut colour reduction for GIF frames whose colours don't fit a
//! 256-entry table (spec §7.5). Only frames that overflow go through here;
//! frames that fit keep their exact colours.

use std::collections::HashMap;

/// Reduce the weighted colour set `colors` (RGB → pixel count) to at most
/// `max` representative colours by median cut, splitting the box with the
/// widest channel range at its weighted median until `max` boxes exist or
/// no box can be split further. Each palette entry is its box's weighted
/// mean.
pub(super) fn median_cut(colors: &HashMap<[u8; 3], u32>, max: usize) -> Vec<[u8; 3]> {
    let mut boxes: Vec<Vec<([u8; 3], u32)>> = vec![colors.iter().map(|(&c, &n)| (c, n)).collect()];
    if boxes[0].is_empty() || max == 0 {
        return Vec::new();
    }
    while boxes.len() < max {
        // The splittable box with the widest channel range.
        let Some((index, channel)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .map(|(i, b)| {
                let (channel, range) = widest_channel(b);
                (i, channel, range)
            })
            .max_by_key(|&(_, _, range)| range)
            .map(|(i, channel, _)| (i, channel))
        else {
            break;
        };
        let mut split = boxes.swap_remove(index);
        split.sort_unstable_by_key(|(c, _)| c[channel]);
        let total: u64 = split.iter().map(|&(_, n)| u64::from(n)).sum();
        let mut seen = 0u64;
        let mut cut = split.len() / 2;
        for (i, &(_, n)) in split.iter().enumerate() {
            seen += u64::from(n);
            if seen * 2 >= total {
                cut = i + 1;
                break;
            }
        }
        // Both halves must be non-empty.
        let cut = cut.clamp(1, split.len() - 1);
        let upper = split.split_off(cut);
        boxes.push(split);
        boxes.push(upper);
    }
    boxes.iter().map(|b| weighted_mean(b)).collect()
}

/// The channel (0 = R, 1 = G, 2 = B) with the largest value range in
/// `colors`, and that range.
fn widest_channel(colors: &[([u8; 3], u32)]) -> (usize, u8) {
    (0..3)
        .map(|ch| {
            let (lo, hi) = colors.iter().fold((u8::MAX, u8::MIN), |(lo, hi), (c, _)| {
                (lo.min(c[ch]), hi.max(c[ch]))
            });
            (ch, hi.saturating_sub(lo))
        })
        .max_by_key(|&(_, range)| range)
        .unwrap_or((0, 0))
}

/// Pixel-count weighted mean colour of a non-empty box.
fn weighted_mean(colors: &[([u8; 3], u32)]) -> [u8; 3] {
    let mut sum = [0u64; 3];
    let mut total = 0u64;
    for &(c, n) in colors {
        let n = u64::from(n.max(1));
        for ch in 0..3 {
            sum[ch] += u64::from(c[ch]) * n;
        }
        total += n;
    }
    let total = total.max(1);
    // A mean of u8 values fits u8.
    sum.map(|s| ((s + total / 2) / total).min(255) as u8)
}

/// Index of the palette entry closest to `color` (squared RGB distance).
pub(super) fn nearest(palette: &[[u8; 3]], color: [u8; 3]) -> u8 {
    let mut best = 0usize;
    let mut best_distance = u32::MAX;
    for (i, entry) in palette.iter().enumerate() {
        let distance: u32 = entry
            .iter()
            .zip(color)
            .map(|(&a, b)| u32::from(a.abs_diff(b)).pow(2))
            .sum();
        if distance < best_distance {
            best = i;
            best_distance = distance;
        }
    }
    // Callers pass at most 256 entries.
    u8::try_from(best).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(n: u32) -> HashMap<[u8; 3], u32> {
        (0..n)
            .map(|i| {
                let [r, g, b, _] = (i * 97).to_le_bytes();
                ([r, g, b], 1 + i % 3)
            })
            .collect()
    }

    #[test]
    fn median_cut_overflowing_set_returns_at_most_max_colors() {
        let colors = gradient(1000);
        let palette = median_cut(&colors, 255);
        assert!(!palette.is_empty());
        assert!(palette.len() <= 255, "{} entries", palette.len());
    }

    #[test]
    fn median_cut_small_set_keeps_every_color() {
        let colors: HashMap<[u8; 3], u32> =
            [([1, 2, 3], 5), ([200, 0, 0], 1), ([0, 0, 255], 2)].into();
        let mut palette = median_cut(&colors, 255);
        palette.sort_unstable();
        assert_eq!(palette, [[0, 0, 255], [1, 2, 3], [200, 0, 0]]);
    }

    #[test]
    fn nearest_picks_the_closest_entry() {
        let palette = [[0, 0, 0], [255, 255, 255], [250, 0, 0]];
        assert_eq!(nearest(&palette, [240, 10, 5]), 2);
        assert_eq!(nearest(&palette, [20, 20, 20]), 0);
    }
}
