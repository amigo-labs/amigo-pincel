//! Property-based codec round-trips (CLAUDE.md §7.4): for the lossless
//! formats, `decode(encode(x))` must reproduce the pixels exactly for
//! arbitrary buffer contents and dimensions.

use fineliner_core::codec::{decode, to_bmp_bytes, to_png_bytes};
use fineliner_core::ImageBuffer;
use proptest::prelude::*;

/// An arbitrary small buffer: 1–16 × 1–16 with arbitrary RGBA bytes.
fn arb_buffer() -> impl Strategy<Value = ImageBuffer> {
    (1u32..=16, 1u32..=16)
        .prop_flat_map(|(w, h)| {
            let len = (w * h * 4) as usize;
            (
                Just(w),
                Just(h),
                proptest::collection::vec(any::<u8>(), len),
            )
        })
        .prop_map(|(w, h, data)| ImageBuffer::from_raw(w, h, data).expect("sized to fit"))
}

proptest! {
    #[test]
    fn png_round_trip_is_lossless(buf in arb_buffer()) {
        let bytes = to_png_bytes(&buf, 6).expect("encode");
        let back = decode(&bytes).expect("decode");
        prop_assert_eq!(buf.data(), back.data());
    }

    #[test]
    fn bmp_round_trip_is_lossless(buf in arb_buffer()) {
        let bytes = to_bmp_bytes(&buf).expect("encode");
        let back = decode(&bytes).expect("decode");
        prop_assert_eq!(buf.data(), back.data());
    }
}
