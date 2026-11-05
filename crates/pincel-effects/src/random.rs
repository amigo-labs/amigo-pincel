//! Seeded, stateless randomness shared by the effects that need it (Sketch
//! Blur, Crystalize, Dents, Frosted Glass): one SplitMix64 hash per seed and
//! position, so a result never depends on the order pixels are visited.

/// SplitMix64 of the seed and a position: a stateless per-position random
/// number. The same arguments always give the same bits.
pub(crate) fn hash(seed: u32, x: usize, y: usize) -> u64 {
    let mut z = (u64::from(seed) << 40) ^ ((y as u64) << 20) ^ x as u64;
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 24 bits of `bits` as a number in `[0, 1]`.
pub(crate) fn unit(bits: u64) -> f32 {
    (bits & 0xFF_FFFF) as f32 / 0xFF_FFFF as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_differs_per_seed_and_position() {
        assert_ne!(hash(1, 0, 0), hash(2, 0, 0));
        assert_ne!(hash(1, 1, 0), hash(1, 0, 1));
        assert_eq!(hash(7, 3, 4), hash(7, 3, 4));
    }

    #[test]
    fn unit_spans_zero_to_one() {
        assert_eq!(unit(0), 0.0);
        assert_eq!(unit(0xFF_FFFF), 1.0);
    }
}
