//! Line image preparation: flattening, the crop Tesseract hands the LSTM,
//! scaling to the network's input height and the conversion to network
//! input (`Input::PreparePixInput`, `NetworkIO::FromPix`).
//!
//! Pixels use Leptonica's 32-bpp layout (`0xRRGGBBAA`): Tesseract reads the
//! reference line images as RGB PNGs, which Leptonica stores with an opaque
//! alpha byte, and follows its colour paths from there.

pub(crate) mod scale;

use crate::functions::TRand;
use crate::network::{Map, NetworkIo};
use crate::textord::TBox;
use scale::{Grey8, Rgb32};

/// Composites RGBA pixels over white or black, whichever contrasts more
/// with the line's ink (docs/specs/ocr-recognizer.md, "Flattening"):
/// black when the alpha-weighted mean luminance is above 50 %, otherwise
/// white. Returns `None` when every pixel is fully transparent.
pub(crate) fn flatten(width: usize, height: usize, rgba: &[u8]) -> Option<Rgb32> {
    let mut weighted = 0u64;
    let mut alpha_total = 0u64;
    for p in rgba.as_chunks::<4>().0 {
        // Rec. 601 luma in 1/1000 steps.
        let luma = 299 * u64::from(p[0]) + 587 * u64::from(p[1]) + 114 * u64::from(p[2]);
        weighted += luma * u64::from(p[3]);
        alpha_total += u64::from(p[3]);
    }
    if alpha_total == 0 {
        return None;
    }
    let background: u32 = if weighted * 2 > alpha_total * 255 * 1000 {
        0
    } else {
        255
    };
    let data = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            let a = u32::from(p[3]);
            let mix = |c: u8| (u32::from(c) * a + background * (255 - a) + 127) / 255;
            (mix(p[0]) << 24) | (mix(p[1]) << 16) | (mix(p[2]) << 8) | 0xff
        })
        .collect();
    Some(Rgb32 {
        width,
        height,
        data,
    })
}

/// The pixels as Leptonica-order bytes (R, G, B, A per pixel), the layout
/// Tesseract's thresholder reads.
pub(crate) fn channel_bytes(image: &Rgb32) -> Vec<u8> {
    image.data.iter().flat_map(|p| p.to_be_bytes()).collect()
}

/// `pixClipRectangle` of a box in Tesseract coordinates.
pub(crate) fn crop(image: &Rgb32, b: &TBox) -> Rgb32 {
    let h = image.height as i32;
    let (x0, y0) = (b.left.max(0) as usize, (h - b.top).max(0) as usize);
    let w = b.width().max(0) as usize;
    let rows = b.height().max(0) as usize;
    let mut data = Vec::with_capacity(w * rows);
    for y in y0..y0 + rows {
        let start = y * image.width + x0;
        data.extend_from_slice(&image.data[start..start + w]);
    }
    Rgb32 {
        width: w,
        height: rows,
        data,
    }
}

/// `ImageData::PreScale`: scales the crop to `target_height` rows.
/// Returns the image and the scale factor.
pub(crate) fn prescale(image: &Rgb32, target_height: usize) -> (Rgb32, f32) {
    let factor = target_height as f32 / image.height as f32;
    (scale::scale_rgb(image, factor), factor)
}

/// `ComputeBlackWhite`: the 25th percentile of local minima and the 75th
/// percentile of local maxima along the middle row.
fn black_white(grey: &Grey8) -> (f32, f32) {
    let (w, h) = (grey.width, grey.height);
    let mut mins = [0i32; 256];
    let mut maxes = [0i32; 256];
    if w >= 3 {
        let line = &grey.data[(h / 2) * w..(h / 2) * w + w];
        let mut prev = i32::from(line[0]);
        let mut curr = i32::from(line[1]);
        for &next in &line[2..] {
            let next = i32::from(next);
            if (curr < prev && curr <= next) || (curr <= prev && curr < next) {
                mins[curr as usize] += 1;
            }
            if (curr > prev && curr >= next) || (curr >= prev && curr > next) {
                maxes[curr as usize] += 1;
            }
            prev = curr;
            curr = next;
        }
    }
    if mins.iter().all(|&c| c == 0) {
        mins[0] = 1;
    }
    if maxes.iter().all(|&c| c == 0) {
        maxes[255] = 1;
    }
    (ile(&mins, 0.25) as f32, ile(&maxes, 0.75) as f32)
}

/// `STATS::ile` over a 0..=255 histogram.
fn ile(buckets: &[i32; 256], frac: f64) -> f64 {
    let total: i32 = buckets.iter().sum();
    let target = (frac * f64::from(total)).clamp(1.0, f64::from(total));
    let mut sum = 0;
    let mut index = 0usize;
    while index < 256 && f64::from(sum) < target {
        sum += buckets[index];
        index += 1;
    }
    if index > 0 {
        index as f64 - (f64::from(sum) - target) / f64::from(buckets[index - 1])
    } else {
        0.0
    }
}

/// `NetworkIO::FromPix` for a one-channel network input of `height` rows:
/// normalizes each pixel by the line's black and white levels and stores it
/// as int8.
pub(crate) fn network_input(grey: &Grey8, height: usize, rand: &mut TRand) -> NetworkIo {
    let width = grey.width;
    let (black, white) = black_white(grey);
    let mut contrast = (white - black) / 2.0;
    if contrast <= 0.0 {
        contrast = 1.0;
    }
    let mut io = NetworkIo::new_int(Map { height, width }, 1);
    let mut t = 0;
    for y in 0..height {
        let mut x = 0;
        if y < grey.height {
            while x < width {
                let pixel = grey.data[y * width + x];
                let value = (f32::from(pixel) - black) / contrast - 1.0;
                io.write_raw(
                    t,
                    crate::functions::round_f32(128.0 * value).clamp(-127, 127) as i8,
                );
                x += 1;
                t += 1;
            }
        }
        while x < width {
            io.randomize(t, 0, 1, rand);
            x += 1;
            t += 1;
        }
    }
    io
}

/// `pixInvert` of the 32-bpp crop: every bit flips, alpha included, before
/// the conversion to grey.
pub(crate) fn invert(image: &Rgb32) -> Rgb32 {
    Rgb32 {
        width: image.width,
        height: image.height,
        data: image.data.iter().map(|&p| !p).collect(),
    }
}
