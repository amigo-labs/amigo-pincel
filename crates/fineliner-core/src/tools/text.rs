//! The Text tool (spec §9.2 Text).
//!
//! Rasterizes a string onto the current layer and emits one [`SetPixels`] on
//! commit (ADR-003 — text is pixels, not a vector layer). The caller supplies
//! the font as raw TrueType/OpenType bytes (Option B): `fineliner-core` stays
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
#[derive(Debug, Clone)]
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

        let region = self.region(&placed, shear, bold_dx, &doc.canvas)?;
        let cov = rasterize_coverage(&placed, region, shear, bold_dx, self.style.anti_alias);

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
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::geometry::Rect;
    use crate::selection::SelectionMask;

    /// Liberation Sans (SIL OFL 1.1) — a test fixture; see the LICENSE alongside.
    const FONT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/LiberationSans-Regular.ttf"
    ));

    fn style(size: f32) -> TextStyle {
        TextStyle {
            size,
            ..TextStyle::default()
        }
    }

    fn painted_pixels(doc: &Document) -> usize {
        let mut n = 0;
        for y in 0..doc.canvas.height() {
            for x in 0..doc.canvas.width() {
                if doc.layers[0].pixels.get_pixel(x, y).map(|c| c.a) != Some(0) {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn renders_non_empty_text() {
        let mut doc = Document::new(200, 80).unwrap();
        let text = Text::new("Hello", Point::new(10.0, 10.0), style(32.0));
        let mut cmd = text.render(0, &doc, FONT).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert!(painted_pixels(&doc) > 0, "expected glyphs to paint pixels");
    }

    #[test]
    fn empty_text_returns_none() {
        let doc = Document::new(100, 40).unwrap();
        let text = Text::new("   ", Point::new(5.0, 5.0), style(20.0));
        // Whitespace produces no outlined glyphs.
        assert!(text.render(0, &doc, FONT).is_none());
    }

    #[test]
    fn invalid_font_returns_none() {
        let doc = Document::new(100, 40).unwrap();
        let text = Text::new("Hi", Point::new(5.0, 5.0), style(20.0));
        assert!(text.render(0, &doc, &[0u8, 1, 2, 3]).is_none());
    }

    #[test]
    fn commit_is_undoable() {
        let mut doc = Document::new(200, 80).unwrap();
        let text = Text::new("Ag", Point::new(10.0, 10.0), style(40.0));
        let mut cmd = text.render(0, &doc, FONT).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert!(painted_pixels(&doc) > 0);
        cmd.revert(&mut doc).unwrap();
        assert_eq!(painted_pixels(&doc), 0, "undo must clear all glyph pixels");
    }

    #[test]
    fn larger_size_paints_more_pixels() {
        let render_count = |size: f32| {
            let mut doc = Document::new(400, 200).unwrap();
            let mut cmd = Text::new("W", Point::new(10.0, 10.0), style(size))
                .render(0, &doc, FONT)
                .unwrap();
            cmd.apply(&mut doc).unwrap();
            painted_pixels(&doc)
        };
        assert!(
            render_count(60.0) > render_count(20.0),
            "a larger font size should cover more pixels"
        );
    }

    #[test]
    fn bold_paints_at_least_as_many_pixels_as_regular() {
        let mut regular_doc = Document::new(200, 80).unwrap();
        let mut regular = Text::new("ll", Point::new(10.0, 10.0), style(40.0))
            .render(0, &regular_doc, FONT)
            .unwrap();
        regular.apply(&mut regular_doc).unwrap();

        let mut bold_doc = Document::new(200, 80).unwrap();
        let bold_style = TextStyle {
            bold: true,
            ..style(40.0)
        };
        let mut bold = Text::new("ll", Point::new(10.0, 10.0), bold_style)
            .render(0, &bold_doc, FONT)
            .unwrap();
        bold.apply(&mut bold_doc).unwrap();

        assert!(painted_pixels(&bold_doc) > painted_pixels(&regular_doc));
    }

    #[test]
    fn respects_active_selection() {
        // Select only the left strip; text spanning the canvas paints only there.
        let mut doc = Document::new(200, 60).unwrap();
        doc.selection = Some(SelectionMask::rectangle(200, 60, Rect::new(0, 0, 60, 60)));
        let mut cmd = Text::new("WWWWWWWW", Point::new(2.0, 5.0), style(40.0))
            .render(0, &doc, FONT)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        // No pixel right of the selection boundary is painted.
        for y in 0..doc.canvas.height() {
            for x in 60..doc.canvas.width() {
                assert_eq!(
                    doc.layers[0].pixels.get_pixel(x, y).map(|c| c.a),
                    Some(0),
                    "pixel ({x},{y}) outside the selection was painted"
                );
            }
        }
    }
}
