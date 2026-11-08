//! Where the text of a line image is: what Tesseract's single-line page
//! layout (`--psm 7`) hands the LSTM.
//!
//! Exact ports: the Otsu binarization (`src/ccstruct/otsuthr.cpp`,
//! `ImageThresholder::ThresholdRectToPix`), the noise classification that
//! sets the block's line size (`Textord::filter_noise_blobs`), the x-height,
//! ascender and descender estimate (`compute_row_xheight`,
//! `compute_block_xheight`, `correct_row_xheight` in
//! `src/textord/makerow.cpp`) and the crop (`Tesseract::LSTMRecognizeWord`,
//! `GetRectImage`). Approximations: blobs are 8-connected components of the
//! binary image, blobs overlapping in x are merged (`pre_associate_blobs`
//! without chopping), and the baseline is a straight line through the
//! bottoms of the blobs that sit on it instead of Tesseract's spline fit
//! (docs/specs/ocr-recognizer.md, Decisions).
//!
//! Coordinates follow Tesseract: `x` grows to the right, `y` grows upwards
//! from the bottom of the image, boxes are `[left, right) × [bottom, top)`.

/// A box in Tesseract coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TBox {
    pub(crate) left: i32,
    pub(crate) bottom: i32,
    pub(crate) right: i32,
    pub(crate) top: i32,
}

impl TBox {
    pub(crate) fn width(&self) -> i32 {
        self.right - self.left
    }

    pub(crate) fn height(&self) -> i32 {
        self.top - self.bottom
    }

    pub(crate) fn union(&self, o: &TBox) -> TBox {
        TBox {
            left: self.left.min(o.left),
            bottom: self.bottom.min(o.bottom),
            right: self.right.max(o.right),
            top: self.top.max(o.top),
        }
    }

    pub(crate) fn x_middle(&self) -> i32 {
        (self.left + self.right) / 2
    }

    /// `TBOX::major_x_overlap`.
    fn major_x_overlap(&self, b: &TBox) -> bool {
        let mut overlap = b.width();
        if self.left > b.left {
            overlap -= self.left - b.left;
        }
        if self.right < b.right {
            overlap -= b.right - self.right;
        }
        overlap >= b.width() / 2 || overlap >= self.width() / 2
    }
}

/// A connected component of the binary image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Blob {
    pub(crate) bbox: TBox,
    /// Number of black pixels (Tesseract's enclosed area).
    pub(crate) area: i32,
}

// --- Otsu binarization ------------------------------------------------------

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

// --- Connected components ---------------------------------------------------

fn find(parent: &mut [u32], mut x: u32) -> u32 {
    while parent[x as usize] != x {
        parent[x as usize] = parent[parent[x as usize] as usize];
        x = parent[x as usize];
    }
    x
}

/// 8-connected components of the black pixels, in Tesseract coordinates.
pub(crate) fn components(black: &[bool], width: usize, height: usize) -> Vec<Blob> {
    let mut labels = vec![u32::MAX; width * height];
    let mut parent: Vec<u32> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            if !black[i] {
                continue;
            }
            let mut label = u32::MAX;
            let mut neighbours = [u32::MAX; 4];
            if x > 0 {
                neighbours[0] = labels[i - 1];
            }
            if y > 0 {
                let up = i - width;
                neighbours[1] = labels[up];
                if x > 0 {
                    neighbours[2] = labels[up - 1];
                }
                if x + 1 < width {
                    neighbours[3] = labels[up + 1];
                }
            }
            for &n in &neighbours {
                if n == u32::MAX {
                    continue;
                }
                if label == u32::MAX {
                    label = find(&mut parent, n);
                } else {
                    let a = find(&mut parent, n);
                    if a != label {
                        let (lo, hi) = (a.min(label), a.max(label));
                        parent[hi as usize] = lo;
                        label = lo;
                    }
                }
            }
            if label == u32::MAX {
                label = parent.len() as u32;
                parent.push(label);
            }
            labels[i] = label;
        }
    }
    let mut index = vec![u32::MAX; parent.len()];
    let mut blobs: Vec<Blob> = Vec::new();
    let h = height as i32;
    for y in 0..height {
        for x in 0..width {
            let l = labels[y * width + x];
            if l == u32::MAX {
                continue;
            }
            let root = find(&mut parent, l) as usize;
            let (xi, yb) = (x as i32, h - 1 - y as i32);
            if index[root] == u32::MAX {
                index[root] = blobs.len() as u32;
                blobs.push(Blob {
                    bbox: TBox {
                        left: xi,
                        bottom: yb,
                        right: xi + 1,
                        top: yb + 1,
                    },
                    area: 0,
                });
            }
            let b = &mut blobs[index[root] as usize];
            b.bbox.left = b.bbox.left.min(xi);
            b.bbox.right = b.bbox.right.max(xi + 1);
            b.bbox.bottom = b.bbox.bottom.min(yb);
            b.bbox.top = b.bbox.top.max(yb + 1);
            b.area += 1;
        }
    }
    blobs.sort_by_key(|b| (b.bbox.left, b.bbox.bottom));
    blobs
}

// --- STATS --------------------------------------------------------------------

/// Tesseract's `STATS`: a histogram over `[min, max]`.
struct Stats {
    min: i32,
    buckets: Vec<i32>,
}

impl Stats {
    fn new(min: i32, max: i32) -> Self {
        let (min, max) = if max < min { (0, 1) } else { (min, max) };
        Self {
            min,
            buckets: vec![0; (max - min + 1) as usize],
        }
    }

    fn clip(&self, v: i32) -> usize {
        (v.clamp(self.min, self.min + self.buckets.len() as i32 - 1) - self.min) as usize
    }

    fn add(&mut self, v: i32, count: i32) {
        let i = self.clip(v);
        self.buckets[i] += count;
    }

    fn pile_count(&self, v: i32) -> i32 {
        let i = self.clip(v);
        self.buckets[i]
    }

    fn total(&self) -> i32 {
        self.buckets.iter().sum()
    }

    /// `STATS::mode`: ties go to the highest value, except that the lowest
    /// bucket wins ties with it.
    fn mode(&self) -> i32 {
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

    fn min_bucket(&self) -> i32 {
        self.buckets
            .iter()
            .position(|&c| c != 0)
            .map_or(self.min, |i| self.min + i as i32)
    }

    fn max_bucket(&self) -> i32 {
        self.buckets
            .iter()
            .rposition(|&c| c != 0)
            .map_or(self.min, |i| self.min + i as i32)
    }

    /// `STATS::ile`.
    fn ile(&self, frac: f64) -> f64 {
        let total = self.total();
        if total == 0 {
            return f64::from(self.min);
        }
        let target = (frac * f64::from(total)).clamp(1.0, f64::from(total));
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

    /// `STATS::median`.
    fn median(&self) -> f64 {
        let median = self.ile(0.5);
        let median_pile = median.floor() as i32;
        if self.total() > 1 && self.pile_count(median_pile) == 0 {
            let mut min_pile = median_pile;
            while self.pile_count(min_pile) == 0 {
                min_pile -= 1;
            }
            let mut max_pile = median_pile;
            while self.pile_count(max_pile) == 0 {
                max_pile += 1;
            }
            return f64::from(min_pile + max_pile) / 2.0;
        }
        median
    }
}

// --- Line size (filter_noise_blobs) ------------------------------------------

/// `Textord::filter_noise_blobs` reduced to the line size it returns,
/// followed by `filter_blobs`' scaling.
fn line_size(blobs: &[Blob]) -> f32 {
    const MAX_NOISE_SIZE: i32 = 7;
    const NOISE_AREA_RATIO: f64 = 0.7;
    const INITIAL_X_ILE: f64 = 0.75;
    const INITIAL_ASC_ILE: f64 = 0.90;
    const WIDTH_LIMIT: f64 = 8.0;
    const MIN_LINESIZE: f32 = 1.25;
    let mut src = Vec::new();
    let mut small = Vec::new();
    for b in blobs {
        let h = b.bbox.height();
        if h < MAX_NOISE_SIZE {
            continue;
        }
        if f64::from(b.area) >= f64::from(h * b.bbox.width()) * NOISE_AREA_RATIO {
            small.push(*b);
        } else {
            src.push(*b);
        }
    }
    let mut stats = Stats::new(0, 599);
    for b in &src {
        stats.add(b.bbox.height(), 1);
    }
    let mut initial_x = stats.ile(INITIAL_X_ILE) as f32;
    let max_y = (f64::from(initial_x) * (0.25 + 0.5 + 2.0 * 0.25) / 0.5).ceil() as f32;
    let min_y = (initial_x / 2.0).floor();
    let max_x = (f64::from(initial_x) * WIDTH_LIMIT).ceil() as f32;
    for b in &small {
        let h = b.bbox.height() as f32;
        if h <= max_y && h >= min_y {
            src.push(*b);
        }
    }
    let mut stats = Stats::new(0, 599);
    for b in &src {
        let (h, w) = (b.bbox.height() as f32, b.bbox.width() as f32);
        if h >= min_y && h <= max_y && w <= max_x {
            stats.add(b.bbox.height(), 1);
        }
    }
    let max_height = (stats.ile(INITIAL_ASC_ILE) * (0.5 / 0.75)) as f32;
    if max_height > initial_x {
        initial_x = max_height;
    }
    let mut size = initial_x;
    if size == 0.0 {
        size = 1.0;
    }
    size * MIN_LINESIZE
}

// --- Row metrics -------------------------------------------------------------

/// `textord_min_xheight`.
const MIN_XHEIGHT: i32 = 10;
const ASCX_RATIO_MIN: f32 = 1.25;
const ASCX_RATIO_MAX: f32 = 1.8;
const DESCX_RATIO_MIN: f32 = 0.25;
const DESCX_RATIO_MAX: f32 = 0.6;
const XHEIGHT_MODE_FRACTION: f32 = 0.4;
const ASCHEIGHT_MODE_FRACTION: f32 = 0.08;
const DESCHEIGHT_MODE_FRACTION: f32 = 0.08;
const MIN_BLOB_HEIGHT_FRACTION: f32 = 0.75;
const XHEIGHT_ERROR_MARGIN: f32 = 0.1;
const MAX_HEIGHT_MODES: usize = 12;

/// The row's baseline and size estimate (`ROW::base_line`, `x_height`,
/// `ascenders`, `descenders`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RowMetrics {
    pub(crate) baseline: f32,
    pub(crate) xheight: f32,
    pub(crate) ascrise: f32,
    pub(crate) descdrop: f32,
}

/// `get_min_max_xheight`.
fn min_max_xheight(line_size: i32) -> (i32, i32) {
    let min = ((line_size as f64 * 0.25).floor() as i32).max(MIN_XHEIGHT);
    let max = (f64::from(line_size) * 3.0).ceil() as i32;
    (min, max)
}

/// `fill_heights` with a straight baseline.
fn fill_heights(units: &[TBox], baseline: f32, min: i32, max: i32) -> (Stats, Stats) {
    let mut heights = Stats::new(min, max);
    let mut floating = Stats::new(min, max);
    for b in units {
        let top = b.top as f32 - baseline;
        let height = b.height() as f32;
        if top >= min as f32 && top <= max as f32 {
            let v = (top + 0.5).floor() as i32;
            heights.add(v, 1);
            if height / top < MIN_BLOB_HEIGHT_FRACTION {
                floating.add(v, 1);
            }
        }
    }
    (heights, floating)
}

/// `compute_height_modes`.
fn height_modes(heights: &Stats, min: i32, max: i32) -> Vec<i32> {
    let mut modes = [0i32; MAX_HEIGHT_MODES];
    let mut dest = 0usize;
    let mut least_count = i32::MAX;
    let mut least_index = 0usize;
    for v in min..=max {
        let pile = heights.pile_count(v);
        if pile <= 0 {
            continue;
        }
        if dest < MAX_HEIGHT_MODES {
            if pile < least_count {
                least_count = pile;
                least_index = dest;
            }
            modes[dest] = v;
            dest += 1;
        } else if pile >= least_count {
            while least_index < MAX_HEIGHT_MODES - 1 {
                modes[least_index] = modes[least_index + 1];
                least_index += 1;
            }
            modes[MAX_HEIGHT_MODES - 1] = v;
            if pile == least_count {
                least_index = MAX_HEIGHT_MODES - 1;
            } else {
                least_count = heights.pile_count(modes[0]);
                least_index = 0;
                for (i, &m) in modes.iter().enumerate().skip(1) {
                    let c = heights.pile_count(m);
                    if c < least_count {
                        least_count = c;
                        least_index = i;
                    }
                }
            }
        }
    }
    modes[..dest].to_vec()
}

/// `compute_xheight_from_modes`: returns `(evidence, xheight, ascrise)`.
fn xheight_from_modes(
    heights: &mut Stats,
    floating: &Stats,
    min: i32,
    max: i32,
) -> (i32, f32, f32) {
    let mut blob_index = heights.mode();
    let blob_count = heights.pile_count(blob_index);
    if blob_count == 0 {
        return (0, 0.0, 0.0);
    }
    let modes = height_modes(heights, min, max);
    let mut in_best_pile = false;
    let mut prev_size = -i32::MAX;
    let mut best_count = 0;
    let (mut xheight, mut ascrise) = (0.0f32, 0.0f32);
    for x in 0..modes.len().saturating_sub(1) {
        if modes[x] != prev_size + 1 {
            in_best_pile = false;
        }
        let modes_x_count = heights.pile_count(modes[x]) - floating.pile_count(modes[x]);
        if modes_x_count as f32 >= blob_count as f32 * XHEIGHT_MODE_FRACTION
            && (in_best_pile || modes_x_count > best_count)
        {
            for asc in x + 1..modes.len() {
                let ratio = modes[asc] as f32 / modes[x] as f32;
                if ASCX_RATIO_MIN < ratio
                    && ratio < ASCX_RATIO_MAX
                    && heights.pile_count(modes[asc]) as f32
                        >= blob_count as f32 * ASCHEIGHT_MODE_FRACTION
                {
                    if modes_x_count > best_count {
                        in_best_pile = true;
                        best_count = modes_x_count;
                    }
                    prev_size = modes[x];
                    xheight = modes[x] as f32;
                    ascrise = (modes[asc] - modes[x]) as f32;
                }
            }
        }
    }
    if xheight == 0.0 {
        if floating.total() > 0 {
            for x in min..max {
                heights.add(x, -floating.pile_count(x));
            }
            blob_index = heights.mode();
            for x in min..max {
                heights.add(x, floating.pile_count(x));
            }
        }
        xheight = blob_index as f32;
        ascrise = 0.0;
        best_count = heights.pile_count(blob_index);
    }
    (best_count, xheight, ascrise)
}

/// `compute_row_descdrop` with a straight baseline.
fn row_descdrop(units: &[TBox], baseline: f32, xheight: f32, evidence: i32, asc: &Stats) -> f32 {
    let mut i_min = asc.min_bucket();
    if (i_min as f32 / xheight) < ASCX_RATIO_MIN {
        i_min = (xheight * ASCX_RATIO_MIN + 0.5).floor() as i32;
    }
    let mut i_max = asc.max_bucket();
    if (i_max as f32 / xheight) > ASCX_RATIO_MAX {
        i_max = (xheight * ASCX_RATIO_MAX).floor() as i32;
    }
    let num_potential_asc: i32 = (i_min..=i_max).map(|i| asc.pile_count(i)).sum();
    let min = (xheight * DESCX_RATIO_MIN + 0.5).floor() as i32;
    let max = (xheight * DESCX_RATIO_MAX).floor() as i32;
    let mut heights = Stats::new(min, max);
    for b in units {
        let height = baseline - b.bottom as f32;
        if height >= min as f32 && height <= max as f32 {
            heights.add((height + 0.5).floor() as i32, 1);
        }
    }
    let index = heights.mode();
    let mut count = heights.pile_count(index);
    if ((count + num_potential_asc) as f32)
        < evidence as f32 * (DESCHEIGHT_MODE_FRACTION + ASCHEIGHT_MODE_FRACTION)
    {
        count = 0;
    }
    if count > 0 { -(index as f32) } else { 0.0 }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Invalid,
    Ascenders,
    Descenders,
    Unknown,
}

fn category(m: &RowMetrics) -> Category {
    if m.xheight <= 0.0 {
        Category::Invalid
    } else if m.ascrise > 0.0 {
        Category::Ascenders
    } else if m.descdrop != 0.0 {
        Category::Descenders
    } else {
        Category::Unknown
    }
}

fn within_margin(test: f32, num: f32) -> bool {
    test >= num * (1.0 - XHEIGHT_ERROR_MARGIN) && test <= num * (1.0 + XHEIGHT_ERROR_MARGIN)
}

/// `compute_row_xheight`, `compute_block_xheight` and `correct_row_xheight`
/// for a block of one row.
fn row_metrics(units: &[TBox], baseline: f32, line_size: f32) -> RowMetrics {
    const ASC_FRAC: f32 = 0.25 / 0.5;
    const DESC_FRAC: f32 = 0.25 / 0.5;
    const XHEIGHT_CAP_RATIO: f32 = 0.5 / 0.75;
    let line_size_i = line_size as i32;
    let (min, max) = min_max_xheight(line_size_i);
    let (mut heights, floating) = fill_heights(units, baseline, min, max);
    let (evidence, xheight, ascrise) = xheight_from_modes(&mut heights, &floating, min, max);
    let mut row = RowMetrics {
        baseline,
        xheight,
        ascrise,
        descdrop: 0.0,
    };
    if row.xheight > 0.0 {
        row.descdrop = row_descdrop(units, baseline, row.xheight, evidence, &heights);
    }
    // compute_block_xheight: the block statistics come from this one row.
    let asc_min = (min as f32 * ASC_FRAC) as i32;
    let asc_max = (max as f32 * ASC_FRAC) as i32;
    let desc_min = (min as f32 * DESC_FRAC) as i32;
    let desc_max = (max as f32 * DESC_FRAC) as i32;
    let (bxh, basc, bdesc) = match category(&row) {
        Category::Ascenders => {
            let mut xh = Stats::new(min, max);
            let mut asc = Stats::new(asc_min, asc_max);
            let mut desc = Stats::new(desc_min, desc_max);
            xh.add(row.xheight as i32, evidence);
            asc.add(row.ascrise as i32, evidence);
            desc.add((-row.descdrop) as i32, evidence);
            (
                xh.median() as f32,
                asc.median() as f32,
                -(desc.median() as f32),
            )
        }
        Category::Descenders => {
            let mut xh = Stats::new(min, max);
            let mut desc = Stats::new(desc_min, desc_max);
            xh.add(row.xheight as i32, evidence);
            desc.add((-row.descdrop) as i32, evidence);
            (xh.median() as f32, 0.0, -(desc.median() as f32))
        }
        Category::Unknown => {
            let (mut caps, floating) = fill_heights(units, baseline, min, max);
            if caps.total() > 0 {
                let (_, xh, asc) = xheight_from_modes(&mut caps, &floating, min, max);
                if asc == 0.0 {
                    (
                        (caps.median() * f64::from(XHEIGHT_CAP_RATIO)) as f32,
                        asc,
                        0.0,
                    )
                } else {
                    (xh, asc, 0.0)
                }
            } else {
                (line_size * 0.5, 0.0, 0.0)
            }
        }
        Category::Invalid => (line_size * 0.5, 0.0, 0.0),
    };
    let (mut bxh, mut basc, mut bdesc) = (bxh, basc, bdesc);
    let mut corrected = false;
    if bxh < MIN_XHEIGHT as f32 {
        bxh = MIN_XHEIGHT as f32;
        corrected = true;
    }
    if corrected || basc <= 0.0 {
        basc = bxh * ASC_FRAC;
    }
    if corrected || bdesc >= 0.0 {
        bdesc = -(bxh * DESC_FRAC);
    }
    // correct_row_xheight
    let normal = within_margin(row.xheight, bxh);
    let cap = within_margin(row.xheight, bxh + basc);
    match category(&row) {
        Category::Ascenders => {
            if row.descdrop >= 0.0 {
                row.descdrop = row.xheight * (bdesc / bxh);
            }
        }
        Category::Invalid => {
            row.xheight = bxh;
            row.ascrise = basc;
            row.descdrop = bdesc;
        }
        Category::Descenders if normal || cap => {
            row.xheight = bxh;
            row.ascrise = basc;
            row.descdrop = bdesc;
        }
        Category::Unknown if normal => {
            row.xheight = bxh;
            row.ascrise = basc;
            row.descdrop = bdesc;
        }
        Category::Descenders => {
            row.ascrise = row.xheight * (basc / bxh);
        }
        Category::Unknown => {
            if cap {
                row.xheight = bxh;
                row.ascrise = basc;
                row.descdrop = bdesc;
            } else {
                row.ascrise = row.xheight * (basc / (bxh + basc));
                row.xheight -= row.ascrise;
                row.descdrop = row.xheight * (bdesc / bxh);
            }
        }
    }
    row
}

/// Merges blobs that overlap in x into units (`pre_associate_blobs`,
/// without chopping). `blobs` are sorted by left edge.
fn associate(blobs: &[Blob]) -> Vec<TBox> {
    let mut units: Vec<TBox> = Vec::new();
    let mut i = 0;
    while i < blobs.len() {
        let mut b = blobs[i].bbox;
        let mut j = i + 1;
        while j < blobs.len() && b.major_x_overlap(&blobs[j].bbox) {
            b = b.union(&blobs[j].bbox);
            j += 1;
        }
        units.push(b);
        i = j;
    }
    units
}

/// A straight, horizontal baseline: the least-median-of-squares location
/// of the blob bottoms (the midpoint of the narrowest window holding just
/// over half of them). Tesseract fits its baseline robustly to the
/// individual blobs, before dots and accents are merged with their letters.
fn baseline(blobs: &[Blob]) -> f32 {
    let mut bottoms: Vec<i32> = blobs.iter().map(|b| b.bbox.bottom).collect();
    if bottoms.is_empty() {
        return 0.0;
    }
    bottoms.sort_unstable();
    let half = bottoms.len() / 2 + 1;
    let mut best = (i32::MAX, 0usize);
    for i in 0..=bottoms.len() - half {
        let range = bottoms[i + half - 1] - bottoms[i];
        if range < best.0 {
            best = (range, i);
        }
    }
    (bottoms[best.1] + bottoms[best.1 + half - 1]) as f32 / 2.0
}

/// `kMaxWordSizeRatio`, `kMaxLineSizeRatio`, `kMaxWordGapRatio` in
/// `src/ccstruct/pageres.cpp`.
const MAX_WORD_SIZE_RATIO: f32 = 1.25;
const MAX_LINE_SIZE_RATIO: f32 = 1.25;
const MAX_WORD_GAP_RATIO: f32 = 2.0;

/// The boxes Tesseract crops for the LSTM, one per group of words it
/// recognizes together (`ROW_RES::ROW_RES` with `merge_similar_words`),
/// each the union of its blobs extended to the row's ascender and
/// descender lines, padded by 4 and clipped to the image
/// (`LSTMRecognizeWord`, `GetRectImage`). Groups split where the gap to
/// the next blob exceeds twice the line height. Returns the boxes with the
/// blobs of each group.
pub(crate) fn line_boxes(blobs: &[Blob], width: i32, height: i32) -> Vec<(TBox, Vec<Blob>)> {
    if blobs.is_empty() {
        let whole = TBox {
            left: 0,
            bottom: 0,
            right: width,
            top: height,
        };
        return vec![(whole, Vec::new())];
    }
    let units = associate(blobs);
    let base = baseline(blobs);
    let metrics = row_metrics(&units, base, line_size(blobs));
    let line_height = metrics.xheight + metrics.ascrise - metrics.descdrop;
    // Group the units like ROW_RES merges similar words.
    let mut groups: Vec<TBox> = Vec::new();
    let mut current: Option<TBox> = None;
    for u in &units {
        current = match current {
            None => Some(*u),
            Some(c) => {
                let joined = c.union(u);
                let fits = (c.height() as f32) <= line_height * MAX_WORD_SIZE_RATIO
                    && (u.height() as f32) <= line_height * MAX_WORD_SIZE_RATIO
                    && (joined.height() as f32) <= line_height * MAX_LINE_SIZE_RATIO
                    && (u.left as f32) <= c.right as f32 + line_height * MAX_WORD_GAP_RATIO;
                if fits {
                    Some(joined)
                } else {
                    groups.push(c);
                    Some(*u)
                }
            }
        };
    }
    groups.extend(current);
    const PADDING: i32 = 4;
    groups
        .iter()
        .map(|g| {
            let mut b = *g;
            // TBOX::set_bottom / set_top take int16: the float truncates.
            let desc_line = base + metrics.descdrop;
            if desc_line < b.bottom as f32 {
                b.bottom = desc_line as i32;
            }
            let asc_line = base + metrics.xheight + metrics.ascrise;
            if asc_line > b.top as f32 {
                b.top = asc_line as i32;
            }
            let padded = TBox {
                left: (b.left - PADDING).max(0),
                bottom: (b.bottom - PADDING).max(0),
                right: (b.right + PADDING).min(width),
                top: (b.top + PADDING).min(height),
            };
            let members = blobs
                .iter()
                .filter(|bl| bl.bbox.left >= g.left && bl.bbox.right <= g.right)
                .copied()
                .collect();
            (padded, members)
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

    #[test]
    fn components_joins_diagonal_pixels() {
        let black = [true, false, false, true];
        let blobs = components(&black, 2, 2);
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].area, 2);
        assert_eq!(
            blobs[0].bbox,
            TBox {
                left: 0,
                bottom: 0,
                right: 2,
                top: 2
            }
        );
    }

    #[test]
    fn stats_median_of_even_count_interpolates_gap() {
        let mut s = Stats::new(0, 10);
        s.add(2, 1);
        s.add(6, 1);
        assert_eq!(s.median(), 4.0);
    }
}
