//! `DrawText` command — rasterize a string onto an image cel (Fineliner
//! parity; Decision Log 2026-09-06, `ab_glyph`).
//!
//! The caller supplies the font as raw TrueType / OpenType bytes; core
//! never loads a font from disk. Text is pixels on commit (no vector
//! layer). Bold and italic are synthesised — faux-bold by a horizontal
//! smear, faux-italic by a ≈12° shear — since one face ships. With
//! anti-aliasing off, glyph coverage is thresholded at 0.5 for crisp
//! pixel-art edges. An active selection (rect or shaped) clips the
//! written pixels. Prior pixels are captured for `revert`.

use ab_glyph::{Font, FontRef, GlyphId, OutlinedGlyph, PxScale, ScaleFont};

use crate::document::{BlendMode, CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};
use crate::geometry::Rect;
use crate::render::blend_pixel_into;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Horizontal alignment of each line about the placement x.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    /// Stable snake_case wire name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }

    /// Inverse of [`TextAlign::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Left, Self::Center, Self::Right]
            .into_iter()
            .find(|a| a.name() == name)
    }
}

/// Shear applied for faux-italic (≈12°).
const ITALIC_SHEAR: f32 = 0.21;

/// Styling for a [`DrawText`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Font size in pixels (clamped to 1–2000).
    pub size: f32,
    pub color: Rgba,
    pub bold: bool,
    pub italic: bool,
    /// Anti-alias glyph edges; off thresholds coverage at 0.5.
    pub anti_alias: bool,
    pub align: TextAlign,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: 12.0,
            color: Rgba::BLACK,
            bold: false,
            italic: false,
            anti_alias: false,
            align: TextAlign::Left,
        }
    }
}

/// One laid-out glyph outline plus its baseline (for the shear).
struct Placed {
    outline: OutlinedGlyph,
    baseline: f32,
}

/// Rasterize `text` at `(x, y)` — `y` is the top of the first line's em
/// box, `x` the per-line alignment anchor — onto the cel at
/// `(layer, frame)`.
#[derive(Debug, Clone)]
pub struct DrawText {
    layer: LayerId,
    frame: FrameIndex,
    x: i32,
    y: i32,
    text: String,
    font: Vec<u8>,
    style: TextStyle,
    /// Written pixels (cel-local) with their prior values, after `apply`.
    previous: Option<Vec<(u32, u32, Rgba)>>,
    bounds: Option<Rect>,
}

impl DrawText {
    /// New text command. `font` is the raw TrueType / OpenType file.
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        position: (i32, i32),
        text: impl Into<String>,
        font: Vec<u8>,
        style: TextStyle,
    ) -> Self {
        Self {
            layer,
            frame,
            x: position.0,
            y: position.1,
            text: text.into(),
            font,
            style,
            previous: None,
            bounds: None,
        }
    }

    /// Sprite-space bounding rect of the pixels the last `apply` touched
    /// (`None` before `apply` or when nothing landed on the canvas).
    pub fn bounds(&self) -> Option<Rect> {
        self.bounds
    }

    /// Lay out every glyph: `\n` splits lines, alignment and kerning apply.
    fn layout<F: Font, SF: ScaleFont<F>>(&self, scaled: &SF) -> Vec<Placed> {
        let scale = scaled.scale();
        let line_height = scaled.ascent() - scaled.descent() + scaled.line_gap();
        let mut placed = Vec::new();
        for (li, line) in self.text.split('\n').enumerate() {
            let baseline = self.y as f32 + scaled.ascent() + li as f32 * line_height;
            let width = line_width(scaled, line);
            let start_x = match self.style.align {
                TextAlign::Left => self.x as f32,
                TextAlign::Center => self.x as f32 - width / 2.0,
                TextAlign::Right => self.x as f32 - width,
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
}

/// Advance width of `line` in pixels (with kerning).
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

/// Union bounding box of the placed glyphs after shear / bold expansion,
/// clamped to the canvas. `None` when nothing lands on-canvas.
fn region(
    placed: &[Placed],
    shear: f32,
    bold_dx: i32,
    canvas_w: u32,
    canvas_h: u32,
) -> Option<Rect> {
    let cw = canvas_w as f32;
    let ch = canvas_h as f32;
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for p in placed {
        let b = p.outline.px_bounds();
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

/// Glyph coverage over `region` (max-combined), with faux-italic shear and
/// faux-bold smear; thresholded at 0.5 when `anti_alias` is off.
fn rasterize_coverage(
    placed: &[Placed],
    region: Rect,
    shear: f32,
    bold_dx: i32,
    anti_alias: bool,
) -> Vec<f32> {
    let rw = region.width as usize;
    let rh = region.height as usize;
    let mut cov = vec![0.0f32; rw * rh];
    let mut put = |x: i32, y: i32, c: f32| {
        let lx = x - region.x;
        let ly = y - region.y;
        if lx >= 0 && ly >= 0 && (lx as u32) < region.width && (ly as u32) < region.height {
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

impl Command for DrawText {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let font =
            FontRef::try_from_slice(&self.font).map_err(|e| CommandError::Font(e.to_string()))?;
        let size = self.style.size.clamp(1.0, 2000.0);
        let scaled = font.as_scaled(PxScale::from(size));
        let shear = if self.style.italic { ITALIC_SHEAR } else { 0.0 };
        let bold_dx = if self.style.bold {
            (size / 24.0).round().max(1.0) as i32
        } else {
            0
        };
        let placed = self.layout(&scaled);
        let Some(region) = region(&placed, shear, bold_dx, doc.width, doc.height) else {
            self.previous = Some(Vec::new());
            self.bounds = None;
            return Ok(());
        };
        let cov = rasterize_coverage(&placed, region, shear, bold_dx, self.style.anti_alias);
        let has_selection = doc.has_selection();

        let cel = cels
            .get_mut(self.layer, self.frame)
            .ok_or(CommandError::MissingCel {
                layer: self.layer,
                frame: self.frame,
            })?;
        let cel_pos = cel.position;
        let CelData::Image(buffer) = &mut cel.data else {
            return Err(CommandError::NotAnImageCel {
                layer: self.layer,
                frame: self.frame,
            });
        };
        if !matches!(buffer.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }

        let mut prior = Vec::new();
        let rw = region.width as usize;
        for ly in 0..region.height {
            for lx in 0..region.width {
                let c = cov[ly as usize * rw + lx as usize];
                if c <= 0.0 {
                    continue;
                }
                let sx = region.x + lx as i32;
                let sy = region.y + ly as i32;
                if has_selection && !doc.selection_contains(sx, sy) {
                    continue;
                }
                let cx = i64::from(sx) - i64::from(cel_pos.0);
                let cy = i64::from(sy) - i64::from(cel_pos.1);
                if cx < 0
                    || cy < 0
                    || cx >= i64::from(buffer.width)
                    || cy >= i64::from(buffer.height)
                {
                    continue;
                }
                let (cx, cy) = (cx as u32, cy as u32);
                let off = ((cy * buffer.width + cx) * 4) as usize;
                let before = Rgba::new(
                    buffer.data[off],
                    buffer.data[off + 1],
                    buffer.data[off + 2],
                    buffer.data[off + 3],
                );
                let alpha = (f32::from(self.style.color.a) * c)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                blend_pixel_into(
                    BlendMode::Normal,
                    &mut buffer.data[off..off + 4],
                    self.style.color.r,
                    self.style.color.g,
                    self.style.color.b,
                    alpha,
                );
                prior.push((cx, cy, before));
            }
        }
        self.previous = Some(prior);
        self.bounds = Some(region);
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(prior) = self.previous.take() else {
            return;
        };
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        for (cx, cy, before) in prior {
            if cx >= buffer.width || cy >= buffer.height {
                continue;
            }
            let off = ((cy * buffer.width + cx) * 4) as usize;
            buffer.data[off] = before.r;
            buffer.data[off + 1] = before.g;
            buffer.data[off + 2] = before.b;
            buffer.data[off + 3] = before.a;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        match (self.previous.is_some(), self.bounds) {
            (true, Some(r)) => DirtyRegion::layer_rect(self.layer, self.frame, r),
            _ => DirtyRegion::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{Cel, Frame, Layer, PixelBuffer};

    /// Liberation Sans (SIL OFL 1.1) — test fixture, licence alongside.
    const FONT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/LiberationSans-Regular.ttf"
    ));

    const L: LayerId = LayerId::new(0);
    const F: FrameIndex = FrameIndex::new(0);

    fn doc(w: u32, h: u32) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(w, h)
            .add_layer(Layer::image(L, "a"))
            .add_frame(Frame::new(100))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(L, F, PixelBuffer::empty(w, h, ColorMode::Rgba)));
        (sprite, cels)
    }

    fn opaque_count(cels: &CelMap) -> usize {
        let CelData::Image(b) = &cels.get(L, F).unwrap().data else {
            unreachable!()
        };
        b.data.chunks_exact(4).filter(|p| p[3] > 0).count()
    }

    fn style(size: f32) -> TextStyle {
        TextStyle {
            size,
            color: Rgba::WHITE,
            ..TextStyle::default()
        }
    }

    #[test]
    fn apply_writes_glyph_pixels_and_undo_restores() {
        let (mut s, mut c) = doc(64, 32);
        let mut bus = Bus::new();
        let cmd = DrawText::new(L, F, (2, 2), "Hi", FONT.to_vec(), style(20.0));
        bus.execute(cmd.into(), &mut s, &mut c).unwrap();
        assert!(opaque_count(&c) > 20, "glyphs landed");
        assert!(matches!(bus.last_dirty_region(), DirtyRegion::Layer { .. }));
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(opaque_count(&c), 0);
    }

    #[test]
    fn aliased_text_is_binary_and_anti_aliased_has_partial_alpha() {
        let (mut s, mut c) = doc(64, 32);
        DrawText::new(L, F, (0, 0), "O", FONT.to_vec(), style(24.0))
            .apply(&mut s, &mut c)
            .unwrap();
        let CelData::Image(b) = &c.get(L, F).unwrap().data else {
            unreachable!()
        };
        assert!(b.data.chunks_exact(4).all(|p| p[3] == 0 || p[3] == 255));
        let (mut s, mut c) = doc(64, 32);
        let aa = TextStyle {
            anti_alias: true,
            ..style(24.0)
        };
        DrawText::new(L, F, (0, 0), "O", FONT.to_vec(), aa)
            .apply(&mut s, &mut c)
            .unwrap();
        let CelData::Image(b) = &c.get(L, F).unwrap().data else {
            unreachable!()
        };
        assert!(b.data.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
    }

    #[test]
    fn alignment_and_styles_shift_pixels() {
        let extent = |st: TextStyle, x: i32| {
            let (mut s, mut c) = doc(128, 32);
            let mut cmd = DrawText::new(L, F, (x, 0), "ab", FONT.to_vec(), st);
            cmd.apply(&mut s, &mut c).unwrap();
            cmd.bounds().unwrap()
        };
        let left = extent(style(16.0), 64);
        let right = extent(
            TextStyle {
                align: TextAlign::Right,
                ..style(16.0)
            },
            64,
        );
        assert!(right.x < left.x);
        let bold = extent(
            TextStyle {
                bold: true,
                ..style(16.0)
            },
            0,
        );
        assert!(bold.width >= left.width);
        let italic = extent(
            TextStyle {
                italic: true,
                ..style(16.0)
            },
            0,
        );
        assert!(italic.width >= left.width);
    }

    #[test]
    fn multiline_text_extends_downwards() {
        let one = {
            let (mut s, mut c) = doc(64, 64);
            let mut cmd = DrawText::new(L, F, (0, 0), "a", FONT.to_vec(), style(12.0));
            cmd.apply(&mut s, &mut c).unwrap();
            cmd.bounds().unwrap()
        };
        let two = {
            let (mut s, mut c) = doc(64, 64);
            let mut cmd = DrawText::new(L, F, (0, 0), "a\na", FONT.to_vec(), style(12.0));
            cmd.apply(&mut s, &mut c).unwrap();
            cmd.bounds().unwrap()
        };
        assert!(two.height > one.height);
    }

    #[test]
    fn selection_clips_and_off_canvas_is_a_noop() {
        let (mut s, mut c) = doc(64, 32);
        s.set_selection(Rect::new(0, 0, 1, 1));
        DrawText::new(L, F, (10, 4), "MM", FONT.to_vec(), style(20.0))
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!(
            opaque_count(&c),
            0,
            "everything outside the 1×1 selection is clipped"
        );
        s.clear_selection();
        let mut off = DrawText::new(L, F, (500, 500), "x", FONT.to_vec(), style(12.0));
        off.apply(&mut s, &mut c).unwrap();
        assert!(off.bounds().is_none());
        assert!(off.dirty_region().is_none());
    }

    #[test]
    fn bad_font_bytes_error() {
        let (mut s, mut c) = doc(8, 8);
        assert!(matches!(
            DrawText::new(L, F, (0, 0), "a", vec![1, 2, 3], style(12.0)).apply(&mut s, &mut c),
            Err(CommandError::Font(_))
        ));
    }
}
