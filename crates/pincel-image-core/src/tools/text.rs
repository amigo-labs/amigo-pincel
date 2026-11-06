//! The Text tool (spec §9.2 Text).
//!
//! Rasterizes a string onto the current layer and emits one [`SetPixels`] on
//! commit (ADR-003 — text is pixels, not a vector layer). The caller supplies
//! the font as raw TrueType/OpenType bytes (Option B): `pincel-image-core` stays
//! platform-free and never loads a font from disk or the system. Bold and
//! italic are synthesized (faux-bold by horizontal smearing, faux-italic by a
//! shear) since Phase 1 selects a single font face.

use super::src_over;
use crate::color::Color;
use crate::command::SetPixels;
use crate::document::Document;
use crate::geometry::{Point, Rect};
use ab_glyph::{Font, FontVec, GlyphId, OutlinedGlyph, PxScale, ScaleFont};

/// Horizontal alignment of each text line about the placement point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Lines start at the placement x.
    #[default]
    Left,
    /// Lines are centered on the placement x.
    Center,
    /// Lines end at the placement x.
    Right,
}

/// Shear applied for faux-italic (≈12°).
const ITALIC_SHEAR: f32 = 0.21;

/// Styling for a [`Text`] commit (spec §9.2 Text "Options").
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Font size in pixels, clamped to 1–2000.
    pub size: f32,
    /// Fill color (alpha honored).
    pub color: Color,
    /// Synthesize bold by smearing each glyph horizontally.
    pub bold: bool,
    /// Synthesize italic by shearing each glyph.
    pub italic: bool,
    /// Anti-alias glyph edges; when off, coverage is thresholded at 0.5.
    pub anti_alias: bool,
    /// Per-line horizontal alignment about the placement point.
    pub align: TextAlign,
    /// A stroke below each line's baseline (docs/specs/image-live-text.md AC-10).
    pub underline: bool,
    /// A stroke through each line's lowercase letters.
    pub strikethrough: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: 24.0,
            color: Color::BLACK,
            bold: false,
            italic: false,
            anti_alias: true,
            align: TextAlign::Left,
            underline: false,
            strikethrough: false,
        }
    }
}

/// The Text tool — rasterizes `content` at `position` with `style` (spec §9.2).
///
/// `position.y` is the top of the first line's em box (the first baseline sits
/// `ascent` below it; later `\n`-separated lines advance by the font's line
/// height). `position.x` is the per-line alignment anchor: the left edge for
/// `TextAlign::Left`, the center for `Center`, the right edge for `Right`.
#[derive(Debug, Clone)]
pub struct Text {
    /// The text to rasterize (may contain `\n` line breaks).
    pub content: String,
    /// Placement point: `y` is the top of the first line; `x` is the alignment
    /// anchor per [`TextStyle::align`] (left edge / center / right edge).
    pub position: Point,
    /// Styling.
    pub style: TextStyle,
}

/// Underline top below the baseline, in em (Liberation Sans, Arial metrics).
const UNDERLINE_TOP: f32 = 0.106;
/// Underline thickness, in em.
const UNDERLINE_THICKNESS: f32 = 0.073;
/// Strikethrough centre above the baseline, in em.
const STRIKE_CENTER: f32 = 0.259;
/// Strikethrough thickness, in em.
const STRIKE_THICKNESS: f32 = 0.049;

/// Where a text's lines and caret positions lie (docs/specs/image-live-text.md
/// AC-22): the layout [`Text::render`] paints with.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TextLayout {
    /// One entry per `\n`-separated line, top to bottom.
    pub lines: Vec<LineLayout>,
    /// Distance between consecutive lines' tops.
    pub line_height: f32,
    /// Width of the widest line (the text box width).
    pub width: f32,
}

/// One laid-out line of a [`TextLayout`], in canvas coordinates.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LineLayout {
    /// Top of the line's em box.
    pub top: f32,
    /// The baseline the glyphs sit on.
    pub baseline: f32,
    /// Pen x where the line starts (after alignment).
    pub left: f32,
    /// Advance width of the line, kerning included.
    pub width: f32,
    /// Pen x of every character boundary: before the first character, then
    /// after each character (where the next glyph is drawn, kerning included).
    pub carets: Vec<f32>,
    /// Advance end of the last non-whitespace character; `left` for a line of
    /// only whitespace. Underline and strikethrough run to here.
    pub ink_end: f32,
}

/// One laid-out glyph outline plus the baseline it sits on (for shear).
struct Placed {
    outline: OutlinedGlyph,
    baseline: f32,
}

impl Text {
    /// Creates a text commit.
    pub fn new(content: impl Into<String>, position: Point, style: TextStyle) -> Self {
        Self {
            content: content.into(),
            position,
            style,
        }
    }

    /// Rasterizes the text into a [`SetPixels`] command on `layer_index` using
    /// `font_bytes` (a parsed TrueType/OpenType face).
    ///
    /// Returns `None` if the layer index is invalid, the font fails to parse,
    /// the text is empty/whitespace, or the rendered glyphs miss the canvas.
    /// The active selection (if any) constrains where pixels are written; prior
    /// pixels are captured for undo.
    pub fn render(
        &self,
        layer_index: usize,
        doc: &Document,
        font_bytes: &[u8],
    ) -> Option<SetPixels> {
        let layer = doc.layers.get(layer_index)?;
        let font = FontVec::try_from_vec(font_bytes.to_vec()).ok()?;
        let size = self.style.size.clamp(1.0, 2000.0);
        let scaled = font.as_scaled(PxScale::from(size));
        let shear = if self.style.italic { ITALIC_SHEAR } else { 0.0 };
        let bold_dx = if self.style.bold {
            (size / 24.0).round().max(1.0) as i32
        } else {
            0
        };

        let placed = self.layout(&scaled);
        if placed.is_empty() {
            return None;
        }
        let strokes = self.decorations(&self.lines(&scaled), size, bold_dx);

        let region = self.region(&placed, &strokes, shear, bold_dx, &doc.canvas)?;
        let mut cov = rasterize_coverage(&placed, region, shear, bold_dx, true);
        for stroke in &strokes {
            stroke.cover(region, &mut cov);
        }
        if !self.style.anti_alias {
            for v in cov.iter_mut() {
                *v = if *v >= 0.5 { 1.0 } else { 0.0 };
            }
        }

        // Composite the coverage with the fill color over a copy of the region.
        let selection = doc.selection.as_ref();
        let mut after = layer.pixels.copy_region(region).ok()?;
        let color = self.style.color;
        let rw = region.w as usize;
        for ly in 0..region.h {
            for lx in 0..region.w {
                let c = cov[ly as usize * rw + lx as usize];
                if c <= 0.0 {
                    continue;
                }
                let cx = region.x + lx as i32;
                let cy = region.y + ly as i32;
                let sel =
                    selection.map_or(1.0, |s| s.coverage(cx as u32, cy as u32) as f32 / 255.0);
                if sel <= 0.0 {
                    continue;
                }
                let sa = color.a as f32 / 255.0 * c * sel;
                let dst = after.get_pixel(lx, ly).unwrap_or(Color::TRANSPARENT);
                after.set_pixel(lx, ly, src_over(color, sa, dst));
            }
        }

        Some(SetPixels::new(layer_index, region, after).with_label("Text"))
    }

    /// The line boxes and caret positions of this text with `font_bytes`;
    /// `None` when the font fails to parse.
    pub fn layout_lines(&self, font_bytes: &[u8]) -> Option<TextLayout> {
        let font = FontVec::try_from_vec(font_bytes.to_vec()).ok()?;
        let size = self.style.size.clamp(1.0, 2000.0);
        let scaled = font.as_scaled(PxScale::from(size));
        let lines = self.lines(&scaled);
        let width = lines.iter().map(|l| l.width).fold(0.0, f32::max);
        Some(TextLayout {
            lines,
            line_height: scaled.ascent() - scaled.descent() + scaled.line_gap(),
            width,
        })
    }

    /// Lays out each line: alignment, kerning, caret positions.
    fn lines<F: Font, SF: ScaleFont<F>>(&self, scaled: &SF) -> Vec<LineLayout> {
        let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();
        self.content
            .split('\n')
            .enumerate()
            .map(|(li, line)| {
                let top = self.position.y + li as f32 * line_height;
                let width = line_width(scaled, line);
                let left = match self.style.align {
                    TextAlign::Left => self.position.x,
                    TextAlign::Center => self.position.x - width / 2.0,
                    TextAlign::Right => self.position.x - width,
                };
                let mut carets = vec![left];
                let mut pen = left;
                let mut ink_end = left;
                let mut prev: Option<GlyphId> = None;
                for c in line.chars() {
                    let id = scaled.glyph_id(c);
                    if let Some(p) = prev {
                        pen += scaled.kern(p, id);
                        if let Some(last) = carets.last_mut() {
                            *last = pen;
                        }
                    }
                    pen += scaled.h_advance(id);
                    if !c.is_whitespace() {
                        ink_end = pen;
                    }
                    carets.push(pen);
                    prev = Some(id);
                }
                LineLayout {
                    top,
                    baseline: top + scaled.ascent(),
                    left,
                    width,
                    carets,
                    ink_end,
                }
            })
            .collect()
    }

    /// The underline and strikethrough rects of `lines`; a whitespace-only
    /// line gets none.
    fn decorations(&self, lines: &[LineLayout], size: f32, bold_dx: i32) -> Vec<Stroke> {
        let mut out = Vec::new();
        let thicken = bold_dx as f32;
        for line in lines.iter().filter(|l| l.ink_end > l.left) {
            if self.style.underline {
                let h = (UNDERLINE_THICKNESS * size).max(1.0) + thicken;
                out.push(Stroke {
                    x0: line.left,
                    x1: line.ink_end,
                    y0: line.baseline + UNDERLINE_TOP * size,
                    y1: line.baseline + UNDERLINE_TOP * size + h,
                });
            }
            if self.style.strikethrough {
                let h = (STRIKE_THICKNESS * size).max(1.0) + thicken;
                let center = line.baseline - STRIKE_CENTER * size;
                out.push(Stroke {
                    x0: line.left,
                    x1: line.ink_end,
                    y0: center - h / 2.0,
                    y1: center + h / 2.0,
                });
            }
        }
        out
    }

    /// Lays out every glyph: splits lines on `\n`, applies alignment and
    /// kerning, and outlines each glyph at its pen position.
    fn layout<F: Font, SF: ScaleFont<F>>(&self, scaled: &SF) -> Vec<Placed> {
        let scale = scaled.scale();
        let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();
        let mut placed = Vec::new();
        for (li, line) in self.content.split('\n').enumerate() {
            let baseline = self.position.y + scaled.ascent() + li as f32 * line_height;
            let width = line_width(scaled, line);
            let start_x = match self.style.align {
                TextAlign::Left => self.position.x,
                TextAlign::Center => self.position.x - width / 2.0,
                TextAlign::Right => self.position.x - width,
            };
            let mut pen = start_x;
            let mut prev: Option<GlyphId> = None;
            for c in line.chars() {
                let id = scaled.glyph_id(c);
                if let Some(p) = prev {
                    pen += scaled.kern(p, id);
                }
                let glyph = id.with_scale_and_position(scale, ab_glyph::point(pen, baseline));
                if let Some(outline) = scaled.outline_glyph(glyph) {
                    placed.push(Placed { outline, baseline });
                }
                pen += scaled.h_advance(id);
                prev = Some(id);
            }
        }
        placed
    }

    /// Union bounding box of all placed glyphs (after shear + bold expansion),
    /// clamped to the canvas. `None` if nothing lands on-canvas.
    fn region(
        &self,
        placed: &[Placed],
        strokes: &[Stroke],
        shear: f32,
        bold_dx: i32,
        canvas: &crate::document::CanvasSize,
    ) -> Option<Rect> {
        let cw = canvas.width() as f32;
        let ch = canvas.height() as f32;
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for p in placed {
            let b = p.outline.px_bounds();
            // Shear shifts x by (baseline - y) * shear across the glyph's rows.
            let top = (p.baseline - b.min.y) * shear;
            let bot = (p.baseline - b.max.y) * shear;
            min_x = min_x.min(b.min.x + top.min(bot));
            max_x = max_x.max(b.max.x + top.max(bot));
            min_y = min_y.min(b.min.y);
            max_y = max_y.max(b.max.y);
        }
        for st in strokes {
            min_x = min_x.min(st.x0);
            max_x = max_x.max(st.x1);
            min_y = min_y.min(st.y0);
            max_y = max_y.max(st.y1);
        }
        let pad = 1.0 + bold_dx as f32;
        let x0 = (min_x - pad).floor().clamp(0.0, cw) as i32;
        let y0 = (min_y - 1.0).floor().clamp(0.0, ch) as i32;
        let x1 = (max_x + pad).ceil().clamp(0.0, cw) as i32;
        let y1 = (max_y + 1.0).ceil().clamp(0.0, ch) as i32;
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
    }
}

/// A solid underline or strikethrough rect, in canvas coordinates.
struct Stroke {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
}

impl Stroke {
    /// Adds the rect's area coverage to a region-sized coverage buffer
    /// (combined by max, like overlapping glyphs).
    fn cover(&self, region: Rect, cov: &mut [f32]) {
        let rw = region.w as usize;
        let overlap = |a0: f32, a1: f32, b0: f32, b1: f32| (a1.min(b1) - a0.max(b0)).max(0.0);
        for ly in 0..region.h {
            let y = (region.y + ly as i32) as f32;
            let cy = overlap(y, y + 1.0, self.y0, self.y1);
            if cy <= 0.0 {
                continue;
            }
            for lx in 0..region.w {
                let x = (region.x + lx as i32) as f32;
                let c = cy * overlap(x, x + 1.0, self.x0, self.x1);
                let idx = ly as usize * rw + lx as usize;
                if c > cov[idx] {
                    cov[idx] = c;
                }
            }
        }
    }
}

/// Advance width of `line` in pixels (with kerning), used for alignment.
fn line_width<F: Font, SF: ScaleFont<F>>(scaled: &SF, line: &str) -> f32 {
    let mut width = 0.0;
    let mut prev: Option<GlyphId> = None;
    for c in line.chars() {
        let id = scaled.glyph_id(c);
        if let Some(p) = prev {
            width += scaled.kern(p, id);
        }
        width += scaled.h_advance(id);
        prev = Some(id);
    }
    width
}

/// Accumulates glyph coverage into a region-sized buffer, combining overlaps by
/// max and applying faux-italic shear and faux-bold smear. Thresholds at 0.5
/// when anti-aliasing is disabled.
fn rasterize_coverage(
    placed: &[Placed],
    region: Rect,
    shear: f32,
    bold_dx: i32,
    anti_alias: bool,
) -> Vec<f32> {
    let rw = region.w as usize;
    let rh = region.h as usize;
    let mut cov = vec![0.0f32; rw * rh];
    let mut put = |x: i32, y: i32, c: f32| {
        let lx = x - region.x;
        let ly = y - region.y;
        if lx >= 0 && ly >= 0 && (lx as u32) < region.w && (ly as u32) < region.h {
            let idx = ly as usize * rw + lx as usize;
            if c > cov[idx] {
                cov[idx] = c;
            }
        }
    };
    for p in placed {
        let b = p.outline.px_bounds();
        let base = p.baseline;
        p.outline.draw(|gx, gy, c| {
            let px = b.min.x as i32 + gx as i32;
            let py = b.min.y as i32 + gy as i32;
            let sx = (px as f32 + (base - py as f32) * shear).round() as i32;
            put(sx, py, c);
            if bold_dx > 0 {
                put(sx + bold_dx, py, c);
            }
        });
    }
    if !anti_alias {
        for v in cov.iter_mut() {
            *v = if *v >= 0.5 { 1.0 } else { 0.0 };
        }
    }
    cov
}

#[cfg(test)]
mod tests;
