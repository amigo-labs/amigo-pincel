//! u8 / sRGB pixel blending primitives shared by the composition paths.
//! See `docs/specs/pincel.md` §4.

/// Source-over (`Normal`) blend, non-premultiplied 8-bit channels.
pub(super) fn blend_normal_into(dst: &mut [u8], sr: u8, sg: u8, sb: u8, sa: u8) {
    if sa == 0 {
        return;
    }
    if sa == 255 {
        dst[0] = sr;
        dst[1] = sg;
        dst[2] = sb;
        dst[3] = 255;
        return;
    }
    let dr = u32::from(dst[0]);
    let dg = u32::from(dst[1]);
    let db = u32::from(dst[2]);
    let da = u32::from(dst[3]);
    let sa32 = u32::from(sa);
    let inv = 255u32 - sa32;
    // αd · (1 − αs), rounded.
    let blend_a = (da * inv + 127) / 255;
    let oa = sa32 + blend_a;
    if oa == 0 {
        dst[3] = 0;
        return;
    }
    dst[0] = ((u32::from(sr) * sa32 + dr * blend_a) / oa) as u8;
    dst[1] = ((u32::from(sg) * sa32 + dg * blend_a) / oa) as u8;
    dst[2] = ((u32::from(sb) * sa32 + db * blend_a) / oa) as u8;
    dst[3] = oa as u8;
}

#[inline]
pub(super) fn mul_u8(a: u8, b: u8) -> u8 {
    (((u16::from(a)) * (u16::from(b)) + 127) / 255) as u8
}
