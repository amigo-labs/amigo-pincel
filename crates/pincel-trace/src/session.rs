//! The trace session: one image, traced any number of times with changing
//! options (`docs/specs/vectorize-core.md`). Mirrors the reference
//! implementation's worker, including its caches.

use std::collections::HashMap;

use crate::color::{Rgb, Rgba};
use crate::error::TraceError;
use crate::pixel_exact::trace_exact;
use crate::potrace::{Bitmap, PotraceParams, Tag, TracedPath, path_data, trace as potrace};
use crate::quality::{despeckle, effective_brightness, otsu};
use crate::quantize::{Quantization, quantize_image, quantize_with_palette};

/// The largest image side a session accepts — Pincel's largest canvas side.
pub const MAX_SIDE: u32 = 32_767;

/// Most colours in colour mode, and in a pinned palette.
pub const MAX_COLORS: u8 = 16;

/// The monochrome threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Threshold {
    /// Chosen by Otsu's method.
    Auto,
    /// A fixed brightness, 0 – 255: darker pixels are filled.
    Manual(u8),
}

/// What a trace produces.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// Potrace per colour after reducing the image to a few colours.
    Color {
        /// Colours to reduce to, 2 – 16; ignored with a pinned palette.
        colors: u8,
        /// Fixed colours (at most 16) instead of a computed palette.
        pinned: Option<Vec<Rgb>>,
    },
    /// Potrace of the pixels darker than a threshold.
    Mono {
        /// The brightness threshold.
        threshold: Threshold,
        /// Fill the brighter pixels instead.
        invert: bool,
        /// The colour of the one layer.
        fill: Rgb,
    },
    /// One layer per distinct colour, outlines along the pixel edges; at
    /// most 256 colours. Despeckle and the Potrace parameters do not apply.
    PixelExact,
}

/// Options of one trace (`docs/specs/vectorize-core.md`, Terms: default
/// options).
#[derive(Clone, Debug, PartialEq)]
pub struct TraceOptions {
    /// The mode.
    pub mode: Mode,
    /// Smooth out single-pixel noise first (3 × 3 median).
    pub despeckle: bool,
    /// Potrace parameters.
    pub potrace: PotraceParams,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            mode: Mode::Color {
                colors: 6,
                pinned: None,
            },
            despeckle: false,
            potrace: PotraceParams::default(),
        }
    }
}

/// Length of the option vector [`TraceOptions::from_wire`] reads.
pub const WIRE_LEN: usize = 9;

impl TraceOptions {
    /// Options from the flat wire format both wasm layers pass
    /// (`docs/specs/vectorize-dialog.md`): `mode` is 0 colours, 1 monochrome,
    /// 2 pixel-exact; `params` is `[colors, threshold (−1 = automatic),
    /// invert, fill (0xRRGGBB), despeckle, min_area, alpha_max, opt_curve,
    /// opt_tolerance]`, flags as 0 / 1. Every field is read in every mode.
    /// Ranges of the Potrace options are checked by [`TraceSession::trace`].
    pub fn from_wire(mode: u8, params: &[f64]) -> Result<Self, TraceError> {
        if params.len() != WIRE_LEN {
            return Err(invalid(
                "params",
                format!("{} values, expected {WIRE_LEN}", params.len()),
            ));
        }
        let int = |name: &'static str, v: f64, max: f64| {
            if v.is_finite() && v.fract() == 0.0 && (0.0..=max).contains(&v) {
                Ok(v)
            } else {
                Err(invalid(
                    name,
                    format!("{v} is not a whole number in 0 – {max}"),
                ))
            }
        };
        let colors = int("colors", params[0], 255.0)? as u8;
        let threshold = if params[1] == -1.0 {
            Threshold::Auto
        } else {
            Threshold::Manual(int("threshold", params[1], 255.0)? as u8)
        };
        let fill = int("fill", params[3], f64::from(0xff_ffff))? as u32;
        let fill = Rgb::new((fill >> 16) as u8, (fill >> 8) as u8, fill as u8);
        let min_area = int("min_area", params[5], f64::from(u32::MAX))? as u32;
        let mode = match mode {
            0 => Mode::Color {
                colors,
                pinned: None,
            },
            1 => Mode::Mono {
                threshold,
                invert: params[2] != 0.0,
                fill,
            },
            2 => Mode::PixelExact,
            other => return Err(invalid("mode", format!("{other} is not 0, 1 or 2"))),
        };
        Ok(Self {
            mode,
            despeckle: params[4] != 0.0,
            potrace: PotraceParams {
                min_area,
                alpha_max: params[6],
                opt_curve: params[7] != 0.0,
                opt_tolerance: params[8],
                ..PotraceParams::default()
            },
        })
    }
}

/// Which mode a result came from; the SVG document depends on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultKind {
    /// Colour mode.
    Color,
    /// Monochrome mode.
    Mono,
    /// Pixel-exact mode.
    PixelExact,
}

/// One colour and its outlines.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// The layer's colour.
    pub color: Rgba,
    /// Pixels assigned to the colour.
    pub area: u64,
    /// The outlines as SVG path data.
    pub path_data: String,
}

/// Counts over all layers of a result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TraceStats {
    /// Outlines.
    pub paths: u64,
    /// Segments (curves plus straight segments).
    pub segments: u64,
    /// Bézier curve segments.
    pub curves: u64,
    /// Straight segments (Potrace corners, or every step in pixel-exact
    /// mode).
    pub lines: u64,
}

/// The result of one trace.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceResult {
    /// Width of the traced image.
    pub width: u32,
    /// Height of the traced image.
    pub height: u32,
    /// The mode that produced it.
    pub kind: ResultKind,
    /// Layers in drawing order, largest area first.
    pub layers: Vec<Layer>,
    /// Counts over all layers.
    pub stats: TraceStats,
    /// The monochrome threshold used (chosen or given); `None` in colour mode.
    pub threshold_used: Option<u16>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum QuantKey {
    Free(u8, bool),
    Pinned(Vec<Rgb>, bool),
}

/// One image, traced any number of times.
pub struct TraceSession {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    despeckled: Option<Vec<u8>>,
    quant_cache: HashMap<QuantKey, Quantization>,
    otsu_cache: [Option<u16>; 2],
}

impl TraceSession {
    /// Copies an RGBA8 image (row-major, straight alpha) into a session.
    pub fn new(rgba: &[u8], width: u32, height: u32) -> Result<Self, TraceError> {
        if width == 0 || height == 0 {
            return Err(TraceError::EmptyImage { width, height });
        }
        if width > MAX_SIDE || height > MAX_SIDE {
            return Err(TraceError::TooLarge {
                width,
                height,
                max: MAX_SIDE,
            });
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(TraceError::TooLarge {
                width,
                height,
                max: MAX_SIDE,
            })?;
        if rgba.len() != expected {
            return Err(TraceError::BufferLength {
                width,
                height,
                expected,
                actual: rgba.len(),
            });
        }
        let mut copy = Vec::new();
        copy.try_reserve_exact(expected)
            .map_err(|_| TraceError::OutOfMemory)?;
        copy.extend_from_slice(rgba);
        Ok(Self {
            width,
            height,
            rgba: copy,
            despeckled: None,
            quant_cache: HashMap::new(),
            otsu_cache: [None; 2],
        })
    }

    /// Width of the image.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the image.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Traces the image with `opts`.
    pub fn trace(&mut self, opts: &TraceOptions) -> Result<TraceResult, TraceError> {
        validate(opts)?;
        match &opts.mode {
            Mode::Color { colors, pinned } => self.trace_color(*colors, pinned.as_deref(), opts),
            Mode::Mono {
                threshold,
                invert,
                fill,
            } => self.trace_mono(*threshold, *invert, *fill, opts),
            Mode::PixelExact => self.trace_pixel_exact(),
        }
    }

    fn source(&mut self, use_despeckle: bool) -> Result<&[u8], TraceError> {
        if !use_despeckle {
            return Ok(&self.rgba);
        }
        if self.despeckled.is_none() {
            self.despeckled = Some(despeckle(
                &self.rgba,
                self.width as usize,
                self.height as usize,
            )?);
        }
        Ok(self.despeckled.as_deref().unwrap_or(&self.rgba))
    }

    fn quantization(
        &mut self,
        colors: u8,
        pinned: Option<&[Rgb]>,
        use_despeckle: bool,
    ) -> Result<Quantization, TraceError> {
        let key = match pinned {
            Some(p) => QuantKey::Pinned(p.to_vec(), use_despeckle),
            None => QuantKey::Free(colors, use_despeckle),
        };
        if let Some(q) = self.quant_cache.get(&key) {
            return Ok(q.clone());
        }
        let src = self.source(use_despeckle)?;
        let q = match pinned {
            Some(p) => quantize_with_palette(src, p.to_vec())?,
            None => quantize_image(src, usize::from(colors))?,
        };
        self.quant_cache.insert(key, q.clone());
        Ok(q)
    }

    fn bitmap(&self) -> Result<Bitmap, TraceError> {
        Bitmap::new(self.width as usize, self.height as usize)
    }

    fn trace_color(
        &mut self,
        colors: u8,
        pinned: Option<&[Rgb]>,
        opts: &TraceOptions,
    ) -> Result<TraceResult, TraceError> {
        let q = self.quantization(colors, pinned, opts.despeckle)?;
        let mut order: Vec<usize> = (0..q.palette.len()).filter(|&i| q.area[i] > 0).collect();
        // Stable: equal areas keep palette order.
        order.sort_by(|&a, &b| q.area[b].cmp(&q.area[a]));
        let mut layers = Vec::new();
        let mut stats = TraceStats::default();
        let mut bm = self.bitmap()?;
        for j in order {
            for (cell, &ix) in bm.data.iter_mut().zip(&q.idx) {
                *cell = u8::from(ix >= 0 && ix as usize == j);
            }
            let paths = potrace(&bm, &opts.potrace)?;
            let d = path_data(&paths)?;
            if d.is_empty() {
                continue;
            }
            add_stats(&mut stats, &paths);
            layers.push(Layer {
                color: Rgba::opaque(q.palette[j]),
                area: q.area[j],
                path_data: d,
            });
        }
        Ok(TraceResult {
            width: self.width,
            height: self.height,
            kind: ResultKind::Color,
            layers,
            stats,
            threshold_used: None,
        })
    }

    fn trace_mono(
        &mut self,
        threshold: Threshold,
        invert: bool,
        fill: Rgb,
        opts: &TraceOptions,
    ) -> Result<TraceResult, TraceError> {
        let th = match threshold {
            Threshold::Manual(t) => u16::from(t),
            Threshold::Auto => {
                let slot = usize::from(opts.despeckle);
                match self.otsu_cache[slot] {
                    Some(t) => t,
                    None => {
                        let t = otsu(self.source(opts.despeckle)?);
                        self.otsu_cache[slot] = Some(t);
                        t
                    }
                }
            }
        };
        let mut bm = self.bitmap()?;
        let src = self.source(opts.despeckle)?;
        let thf = f64::from(th);
        let mut area = 0_u64;
        for (cell, px) in bm.data.iter_mut().zip(src.as_chunks::<4>().0) {
            let filled = (effective_brightness(px) < thf) != invert;
            *cell = u8::from(filled);
            area += u64::from(filled);
        }
        let paths = potrace(&bm, &opts.potrace)?;
        let d = path_data(&paths)?;
        let mut stats = TraceStats::default();
        let mut layers = Vec::new();
        if !d.is_empty() {
            add_stats(&mut stats, &paths);
            layers.push(Layer {
                color: Rgba::opaque(fill),
                area,
                path_data: d,
            });
        }
        Ok(TraceResult {
            width: self.width,
            height: self.height,
            kind: ResultKind::Mono,
            layers,
            stats,
            threshold_used: Some(th),
        })
    }
}

impl TraceSession {
    fn trace_pixel_exact(&mut self) -> Result<TraceResult, TraceError> {
        let exact = trace_exact(&self.rgba, self.width as usize, self.height as usize)?;
        let mut stats = TraceStats::default();
        let layers = exact
            .into_iter()
            .map(|l| {
                stats.paths += l.outlines;
                stats.segments += l.steps;
                stats.lines += l.steps;
                Layer {
                    color: l.color,
                    area: l.area,
                    path_data: l.path_data,
                }
            })
            .collect();
        Ok(TraceResult {
            width: self.width,
            height: self.height,
            kind: ResultKind::PixelExact,
            layers,
            stats,
            threshold_used: None,
        })
    }
}

fn add_stats(stats: &mut TraceStats, paths: &[TracedPath]) {
    for p in paths {
        stats.paths += 1;
        stats.segments += p.curve.n as u64;
        for &tag in &p.curve.tag {
            match tag {
                Tag::Curve => stats.curves += 1,
                Tag::Corner => stats.lines += 1,
            }
        }
    }
}

fn invalid(option: &'static str, reason: impl Into<String>) -> TraceError {
    TraceError::InvalidOption {
        option,
        reason: reason.into(),
    }
}

/// Checks every option, in every mode, in the order the spec names (AC-19).
fn validate(opts: &TraceOptions) -> Result<(), TraceError> {
    if let Mode::Color { colors, pinned } = &opts.mode {
        match pinned {
            None if !(2..=MAX_COLORS).contains(colors) => {
                return Err(invalid(
                    "colors",
                    format!("{colors} is outside 2 – {MAX_COLORS}"),
                ));
            }
            Some(p) if p.len() > usize::from(MAX_COLORS) => {
                return Err(invalid(
                    "pinned",
                    format!("{} colours, at most {MAX_COLORS}", p.len()),
                ));
            }
            _ => {}
        }
    }
    let a = opts.potrace.alpha_max;
    if !(a.is_finite() && (0.0..=1.34).contains(&a)) {
        return Err(invalid("alpha_max", format!("{a} is outside 0 – 1.34")));
    }
    let t = opts.potrace.opt_tolerance;
    if !(t.is_finite() && t >= 0.0) {
        return Err(invalid(
            "opt_tolerance",
            format!("{t} must be a finite number ≥ 0"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantize::build_palette;

    #[test]
    fn trace_color_equal_areas_keep_palette_order() {
        // Four colours in four equal quarters (AC-16).
        let quarters = [[200, 30, 30], [30, 200, 30], [30, 30, 200], [220, 220, 40]];
        let mut rgba = Vec::new();
        for y in 0..8 {
            for x in 0..8 {
                let [r, g, b] = quarters[(y / 4) * 2 + x / 4];
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
        }
        let palette = build_palette(&rgba, 4);
        assert_eq!(palette.len(), 4);
        let mut s = TraceSession::new(&rgba, 8, 8).expect("valid image");
        let opts = TraceOptions {
            mode: Mode::Color {
                colors: 4,
                pinned: None,
            },
            ..TraceOptions::default()
        };
        let r = s.trace(&opts).expect("trace");
        assert!(r.layers.iter().all(|l| l.area == 16));
        let got: Vec<Rgb> = r.layers.iter().map(|l| l.color.rgb()).collect();
        assert_eq!(got, palette);
    }
}
