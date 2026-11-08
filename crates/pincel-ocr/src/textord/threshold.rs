//! Tesseract's Otsu thresholding (`src/ccstruct/otsuthr.cpp`,
//! `ImageThresholder::ThresholdRectToPix`).

/// `OtsuStats`: returns the threshold, the total count and the count at or
/// below the best threshold.
fn otsu_stats(histogram: &[i32; 256]) -> (i32, i32, i32) {
    let mut h = 0i32;
    let mut mu_total = 0.0f64;
    for (i, &c) in histogram.iter().enumerate() {
        h += c;
        mu_total += i as f64 * f64::from(c);
    }
    let mut best_t = -1i32;
    let mut best_omega_0 = 0i32;
    let mut best_sig = 0.0f64;
    let mut omega_0 = 0i32;
    let mut mu_t = 0.0f64;
    for (t, &count) in histogram.iter().enumerate().take(255) {
        omega_0 += count;
        mu_t += t as f64 * f64::from(count);
        if omega_0 == 0 {
            continue;
        }
        let omega_1 = h - omega_0;
        if omega_1 == 0 {
            break;
        }
        let mu_0 = mu_t / f64::from(omega_0);
        let mu_1 = (mu_total - mu_t) / f64::from(omega_1);
        let mut sig = mu_1 - mu_0;
        sig *= sig * f64::from(omega_0) * f64::from(omega_1);
        if best_t < 0 || sig > best_sig {
            best_sig = sig;
            best_t = t as i32;
            best_omega_0 = omega_0;
        }
    }
    (best_t, h, best_omega_0)
}

/// Tesseract's Otsu thresholding of an RGB image: `true` marks a text
/// (black) pixel. `pixels` holds Leptonica-order bytes, four per pixel.
pub(crate) fn binarize(pixels: &[u8], width: usize, height: usize) -> Vec<bool> {
    const CHANNELS: usize = 4;
    let mut thresholds = [-1i32; CHANNELS];
    let mut hi_values = [-1i32; CHANNELS];
    let mut best_hi_value = 1;
    let mut best_hi_index = 0;
    let mut any_good = false;
    let mut best_hi_dist = 0.0f64;
    for ch in 0..CHANNELS {
        let mut histogram = [0i32; 256];
        for p in pixels.chunks_exact(CHANNELS).take(width * height) {
            histogram[usize::from(p[ch])] += 1;
        }
        let (best_t, h, best_omega_0) = otsu_stats(&histogram);
        if best_omega_0 == 0 || best_omega_0 == h {
            continue;
        }
        let hi_value = i32::from(f64::from(best_omega_0) < f64::from(h) * 0.5);
        thresholds[ch] = best_t;
        if f64::from(best_omega_0) > f64::from(h) * 0.75 {
            any_good = true;
            hi_values[ch] = 0;
        } else if f64::from(best_omega_0) < f64::from(h) * 0.25 {
            any_good = true;
            hi_values[ch] = 1;
        } else {
            let hi_dist = if hi_value != 0 {
                f64::from(h - best_omega_0)
            } else {
                f64::from(best_omega_0)
            };
            if hi_dist > best_hi_dist {
                best_hi_dist = hi_dist;
                best_hi_value = hi_value;
                best_hi_index = ch;
            }
        }
    }
    if !any_good {
        hi_values[best_hi_index] = best_hi_value;
    }
    pixels
        .chunks_exact(CHANNELS)
        .take(width * height)
        .map(|p| {
            (0..CHANNELS).any(|ch| {
                hi_values[ch] >= 0 && ((i32::from(p[ch]) > thresholds[ch]) == (hi_values[ch] == 0))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binarize_dark_text_on_white_marks_text() {
        let mut px = vec![255u8; 4 * 10];
        px[4 * 3..4 * 3 + 3].fill(0);
        let black = binarize(&px, 10, 1);
        assert_eq!(black.iter().filter(|&&b| b).count(), 1);
        assert!(black[3]);
    }

    #[test]
    fn binarize_uniform_image_has_no_text() {
        let px = vec![128u8; 4 * 16];
        assert!(binarize(&px, 4, 4).iter().all(|&b| !b));
    }
}
