//! `DrawShape` command — rasterize a rectangle, rounded rectangle,
//! ellipse or regular polygon into an image cel with a configurable
//! stroke width and outline / fill / fill+outline mode (Fineliner
//! parity; the spec §5.2 Rectangle / Ellipse tools are the 1-px cases).
//!
//! The shape is built as coverage masks in sprite space: `fill` is the
//! solid shape, `inner` is `fill` contracted by the stroke width, and the
//! stroke is `fill − inner`. Pixel-art strokes therefore grow inward from
//! the shape's edge and stay a uniform `stroke_width` pixels thick.
//! Pixels outside the target cel are skipped; prior values are captured
//! per written pixel for `revert`.

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};
use crate::geometry::Rect;
use crate::selection::SelectionMask;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Geometry of a [`DrawShape`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapeKind {
    Rectangle,
    /// Corner radius in pixels.
    RoundedRectangle {
        radius: f64,
    },
    Ellipse,
    /// Regular polygon with `sides` vertices, rotated `rotation_deg`
    /// clockwise from "first vertex up".
    Polygon {
        sides: u32,
        rotation_deg: f64,
    },
}

impl ShapeKind {
    /// Stable snake_case wire name of the variant.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            Self::RoundedRectangle { .. } => "rounded_rectangle",
            Self::Ellipse => "ellipse",
            Self::Polygon { .. } => "polygon",
        }
    }
}

/// Which parts of the shape are painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShapeMode {
    #[default]
    Outline,
    Fill,
    FillOutline,
}

impl ShapeMode {
    /// Stable snake_case wire name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Outline => "outline",
            Self::Fill => "fill",
            Self::FillOutline => "fill_outline",
        }
    }

    /// Inverse of [`ShapeMode::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Outline, Self::Fill, Self::FillOutline]
            .into_iter()
            .find(|m| m.name() == name)
    }
}

/// Paint style of a [`DrawShape`]: what is drawn and in which colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeStyle {
    pub mode: ShapeMode,
    /// Stroke thickness in pixels (clamped to at least 1 on use).
    pub stroke_width: u32,
    pub stroke: Rgba,
    pub fill: Rgba,
}

/// Draw a shape into the bounding rect between two sprite-space corners
/// (inclusive, order-independent).
#[derive(Debug, Clone, PartialEq)]
pub struct DrawShape {
    layer: LayerId,
    frame: FrameIndex,
    bounds: Rect,
    kind: ShapeKind,
    style: ShapeStyle,
    previous: Option<Vec<(u32, u32, Rgba)>>,
}

impl DrawShape {
    /// Shape spanning the inclusive corners `start` and `end` on the cel at
    /// `(layer, frame)`.
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        start: (i32, i32),
        end: (i32, i32),
        kind: ShapeKind,
        style: ShapeStyle,
    ) -> Self {
        let min_x = start.0.min(end.0);
        let min_y = start.1.min(end.1);
        let w =
            u32::try_from(i64::from(start.0.max(end.0)) - i64::from(min_x) + 1).unwrap_or(u32::MAX);
        let h =
            u32::try_from(i64::from(start.1.max(end.1)) - i64::from(min_y) + 1).unwrap_or(u32::MAX);
        Self {
            layer,
            frame,
            bounds: Rect::new(min_x, min_y, w, h),
            kind,
            style: ShapeStyle {
                stroke_width: style.stroke_width.max(1),
                ..style
            },
            previous: None,
        }
    }

    /// Inclusive bounding rect of the shape in sprite space.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Coverage of the solid shape inside `work` (a sprite-space rect;
    /// the mask's origin is `work`'s top-left).
    fn coverage(&self, work: Rect) -> SelectionMask {
        let local = Rect::new(
            self.bounds.x.wrapping_sub(work.x),
            self.bounds.y.wrapping_sub(work.y),
            self.bounds.width,
            self.bounds.height,
        );
        let (w, h) = (work.width, work.height);
        match self.kind {
            ShapeKind::Rectangle => SelectionMask::rect(w, h, local),
            ShapeKind::RoundedRectangle { radius } => {
                SelectionMask::rounded_rect(w, h, local, radius)
            }
            ShapeKind::Ellipse => SelectionMask::ellipse(w, h, local),
            ShapeKind::Polygon {
                sides,
                rotation_deg,
            } => SelectionMask::regular_polygon(w, h, local, sides, rotation_deg),
        }
    }
}

impl Command for DrawShape {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let (canvas_w, canvas_h) = (doc.width, doc.height);
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

        // Work area: the shape bounds clipped to the canvas grown by the
        // stroke width, so a shape running off-canvas keeps its true edge
        // (the contract step treats the work-area border as "outside",
        // and that border is then safely off-canvas).
        let margin = i64::from(self.style.stroke_width) + 1;
        let grown = Rect::new(
            (-margin).max(i64::from(i32::MIN)) as i32,
            (-margin).max(i64::from(i32::MIN)) as i32,
            (i64::from(canvas_w) + 2 * margin).min(i64::from(u32::MAX)) as u32,
            (i64::from(canvas_h) + 2 * margin).min(i64::from(u32::MAX)) as u32,
        );
        let work = self.bounds.intersect(grown);
        if work.is_empty() {
            self.previous = Some(Vec::new());
            return Ok(());
        }
        let fill_mask = self.coverage(work);
        let inner = if matches!(self.style.mode, ShapeMode::Fill) {
            fill_mask.clone()
        } else {
            fill_mask.contract(self.style.stroke_width)
        };
        let region = work.intersect(Rect::new(0, 0, canvas_w, canvas_h));
        let mut prior = Vec::new();
        for y in 0..region.height as i32 {
            for x in 0..region.width as i32 {
                let sx = region.x + x;
                let sy = region.y + y;
                let (mx, my) = (sx - work.x, sy - work.y);
                if !fill_mask.contains(mx, my) {
                    continue;
                }
                let is_inner = inner.contains(mx, my);
                let color = match (self.style.mode, is_inner) {
                    (ShapeMode::Fill, _) => self.style.fill,
                    (ShapeMode::Outline, true) => continue,
                    (ShapeMode::Outline, false) => self.style.stroke,
                    (ShapeMode::FillOutline, true) => self.style.fill,
                    (ShapeMode::FillOutline, false) => self.style.stroke,
                };
                let lx = i64::from(sx) - i64::from(cel_pos.0);
                let ly = i64::from(sy) - i64::from(cel_pos.1);
                if lx < 0
                    || ly < 0
                    || lx >= i64::from(buffer.width)
                    || ly >= i64::from(buffer.height)
                {
                    continue;
                }
                let (lx, ly) = (lx as u32, ly as u32);
                let off = ((ly * buffer.width + lx) * 4) as usize;
                let before = Rgba::new(
                    buffer.data[off],
                    buffer.data[off + 1],
                    buffer.data[off + 2],
                    buffer.data[off + 3],
                );
                buffer.data[off] = color.r;
                buffer.data[off + 1] = color.g;
                buffer.data[off + 2] = color.b;
                buffer.data[off + 3] = color.a;
                prior.push((lx, ly, before));
            }
        }
        self.previous = Some(prior);
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
        for (lx, ly, before) in prior {
            if lx >= buffer.width || ly >= buffer.height {
                continue;
            }
            let off = ((ly * buffer.width + lx) * 4) as usize;
            buffer.data[off] = before.r;
            buffer.data[off + 1] = before.g;
            buffer.data[off + 2] = before.b;
            buffer.data[off + 3] = before.a;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        if self.previous.is_none() {
            return DirtyRegion::None;
        }
        DirtyRegion::layer_rect(self.layer, self.frame, self.bounds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{Cel, Frame, Layer, PixelBuffer};

    const L: LayerId = LayerId::new(0);
    const F: FrameIndex = FrameIndex::new(0);
    const S: Rgba = Rgba::new(255, 0, 0, 255);
    const FL: Rgba = Rgba::new(0, 0, 255, 255);

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

    fn px(cels: &CelMap, x: u32, y: u32) -> Rgba {
        let CelData::Image(b) = &cels.get(L, F).unwrap().data else {
            unreachable!()
        };
        let i = ((y * b.width + x) * 4) as usize;
        Rgba::new(b.data[i], b.data[i + 1], b.data[i + 2], b.data[i + 3])
    }

    fn style(mode: ShapeMode, width: u32) -> ShapeStyle {
        ShapeStyle {
            mode,
            stroke_width: width,
            stroke: S,
            fill: FL,
        }
    }

    fn shape(kind: ShapeKind, mode: ShapeMode, width: u32) -> DrawShape {
        DrawShape::new(L, F, (0, 0), (5, 5), kind, style(mode, width))
    }

    #[test]
    fn outline_rectangle_with_width_two_leaves_the_centre() {
        let (mut s, mut c) = doc(6, 6);
        shape(ShapeKind::Rectangle, ShapeMode::Outline, 2)
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!(px(&c, 0, 0), S);
        assert_eq!(px(&c, 1, 1), S, "second ring is stroke");
        assert_eq!(px(&c, 2, 2), Rgba::TRANSPARENT, "interior untouched");
    }

    #[test]
    fn fill_outline_uses_both_colours_and_fill_mode_only_the_fill() {
        let (mut s, mut c) = doc(6, 6);
        shape(ShapeKind::Rectangle, ShapeMode::FillOutline, 1)
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!(px(&c, 0, 0), S);
        assert_eq!(px(&c, 2, 2), FL);
        let (mut s, mut c) = doc(6, 6);
        shape(ShapeKind::Rectangle, ShapeMode::Fill, 3)
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!(px(&c, 0, 0), FL);
        assert_eq!(px(&c, 2, 2), FL);
    }

    #[test]
    fn rounded_rectangle_ellipse_and_polygon_skip_corners() {
        for kind in [
            ShapeKind::RoundedRectangle { radius: 3.0 },
            ShapeKind::Ellipse,
            ShapeKind::Polygon {
                sides: 3,
                rotation_deg: 0.0,
            },
        ] {
            let (mut s, mut c) = doc(6, 6);
            shape(kind, ShapeMode::Fill, 1)
                .apply(&mut s, &mut c)
                .unwrap();
            assert_eq!(px(&c, 0, 0), Rgba::TRANSPARENT, "{kind:?} corner");
            assert_eq!(px(&c, 2, 3), FL, "{kind:?} centre");
        }
    }

    #[test]
    fn undo_restores_and_dirty_region_is_the_bounds() {
        let (mut s, mut c) = doc(6, 6);
        let before = c.clone();
        let mut bus = Bus::new();
        let cmd = DrawShape::new(
            L,
            F,
            (5, 5),
            (1, 1),
            ShapeKind::Ellipse,
            style(ShapeMode::Outline, 1),
        );
        assert_eq!(cmd.bounds(), Rect::new(1, 1, 5, 5));
        bus.execute(cmd.into(), &mut s, &mut c).unwrap();
        assert_eq!(
            bus.last_dirty_region(),
            DirtyRegion::layer_rect(L, F, Rect::new(1, 1, 5, 5))
        );
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(c, before);
    }

    #[test]
    fn shape_partially_off_cel_is_clipped_not_an_error() {
        let (mut s, mut c) = doc(4, 4);
        let mut cmd = DrawShape::new(
            L,
            F,
            (-3, -3),
            (2, 2),
            ShapeKind::Rectangle,
            style(ShapeMode::Outline, 1),
        );
        cmd.apply(&mut s, &mut c).unwrap();
        assert_eq!(px(&c, 2, 0), S, "right edge is on canvas");
        assert_eq!(
            px(&c, 0, 0),
            Rgba::TRANSPARENT,
            "no phantom stroke at the canvas edge"
        );
        assert_eq!(px(&c, 1, 2), S, "bottom edge");
    }

    #[test]
    fn mode_names_round_trip() {
        for m in [ShapeMode::Outline, ShapeMode::Fill, ShapeMode::FillOutline] {
            assert_eq!(ShapeMode::from_name(m.name()), Some(m));
        }
        assert_eq!(ShapeKind::Ellipse.name(), "ellipse");
    }
}
