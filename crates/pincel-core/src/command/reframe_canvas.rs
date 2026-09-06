//! `ReframeCanvas` command — make a sprite-space rect of the old canvas
//! the new canvas. Backs both "resize canvas with anchor" (the rect
//! extends past or inside the old canvas) and "crop to selection" (the
//! rect is the selection).
//!
//! Image cels are re-rendered canvas-sized at `(0, 0)` so painting keeps
//! working everywhere; pixels outside the new frame are dropped (undo
//! restores them). Tilemap cels keep their grid and are shifted by the
//! frame offset. Slice keys shift with the content; the selection is
//! cleared.

use crate::document::{CelMap, Slice, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::error::CommandError;
use super::transform_support::{Raster, cel_to_canvas, raster_to_cel, rgba_buffer};

/// Nine-point anchor for [`ReframeCanvas::resize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    /// Stable snake_case wire name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::Top => "top",
            Self::TopRight => "top_right",
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::BottomLeft => "bottom_left",
            Self::Bottom => "bottom",
            Self::BottomRight => "bottom_right",
        }
    }

    /// Inverse of [`Anchor::name`].
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::TopLeft,
            Self::Top,
            Self::TopRight,
            Self::Left,
            Self::Center,
            Self::Right,
            Self::BottomLeft,
            Self::Bottom,
            Self::BottomRight,
        ]
        .into_iter()
        .find(|a| a.name() == name)
    }

    /// Horizontal / vertical placement as thirds: `0` = start, `1` =
    /// middle, `2` = end.
    const fn thirds(self) -> (i64, i64) {
        match self {
            Self::TopLeft => (0, 0),
            Self::Top => (1, 0),
            Self::TopRight => (2, 0),
            Self::Left => (0, 1),
            Self::Center => (1, 1),
            Self::Right => (2, 1),
            Self::BottomLeft => (0, 2),
            Self::Bottom => (1, 2),
            Self::BottomRight => (2, 2),
        }
    }
}

/// Re-frame the canvas to `rect` (old sprite coordinates).
#[derive(Debug, Clone)]
pub struct ReframeCanvas {
    rect: Rect,
    prior: Option<Prior>,
}

#[derive(Debug, Clone)]
struct Prior {
    width: u32,
    height: u32,
    cels: CelMap,
    slices: Vec<Slice>,
    selection: Option<Rect>,
}

impl ReframeCanvas {
    /// The new canvas is `rect` of the old one.
    pub fn new(rect: Rect) -> Self {
        Self { rect, prior: None }
    }

    /// Resize the canvas to `width × height`, keeping the old content
    /// pinned at `anchor`.
    pub fn resize(old_w: u32, old_h: u32, width: u32, height: u32, anchor: Anchor) -> Self {
        let (tx, ty) = anchor.thirds();
        let dx = (i64::from(width) - i64::from(old_w)) * tx / 2;
        let dy = (i64::from(height) - i64::from(old_h)) * ty / 2;
        Self::new(Rect::new(
            (-dx).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            (-dy).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            width,
            height,
        ))
    }
}

impl Command for ReframeCanvas {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        if self.rect.is_empty() {
            return Err(CommandError::EmptyRegion);
        }
        let (w, h) = (doc.width, doc.height);
        let (dx, dy) = (-i64::from(self.rect.x), -i64::from(self.rect.y));
        let mut new_cels = CelMap::new();
        for (&(layer, frame), cel) in cels.iter() {
            match rgba_buffer(cel) {
                Some(buffer) => {
                    let src = cel_to_canvas(cel, buffer, w, h);
                    let mut out = Raster::transparent(self.rect.width, self.rect.height);
                    out.blit(&src, dx, dy);
                    let mut c = raster_to_cel(layer, frame, out);
                    c.opacity = cel.opacity;
                    new_cels.insert(c);
                }
                None => {
                    let mut c = cel.clone();
                    c.position = (
                        (i64::from(c.position.0) + dx)
                            .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                            as i32,
                        (i64::from(c.position.1) + dy)
                            .clamp(i64::from(i32::MIN), i64::from(i32::MAX))
                            as i32,
                    );
                    new_cels.insert(c);
                }
            }
        }
        let shift = |r: Rect| {
            Rect::new(
                (i64::from(r.x) + dx) as i32,
                (i64::from(r.y) + dy) as i32,
                r.width,
                r.height,
            )
        };
        let new_slices: Vec<Slice> = doc
            .slices
            .iter()
            .map(|s| {
                let mut s = s.clone();
                for k in &mut s.keys {
                    k.bounds = shift(k.bounds);
                    k.center = k.center.map(shift);
                    k.pivot = k
                        .pivot
                        .map(|p| ((i64::from(p.0) + dx) as i32, (i64::from(p.1) + dy) as i32));
                }
                s
            })
            .collect();
        self.prior = Some(Prior {
            width: w,
            height: h,
            cels: std::mem::replace(cels, new_cels),
            slices: std::mem::replace(&mut doc.slices, new_slices),
            selection: doc.selection.take(),
        });
        doc.width = self.rect.width;
        doc.height = self.rect.height;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some(p) = self.prior.take() else {
            return;
        };
        doc.width = p.width;
        doc.height = p.height;
        *cels = p.cels;
        doc.slices = p.slices;
        doc.selection = p.selection;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{
        Cel, CelData, ColorMode, Frame, FrameIndex, Layer, LayerId, PixelBuffer,
    };

    const L: LayerId = LayerId::new(0);
    const F: FrameIndex = FrameIndex::new(0);

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(L, "a"))
            .add_frame(Frame::new(100))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        let mut buf = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        buf.data = (1u8..=4).flat_map(|v| [v, v, v, 255]).collect();
        cels.insert(Cel::image(L, F, buf));
        (sprite, cels)
    }

    fn greys(cels: &CelMap) -> Vec<u8> {
        match &cels.get(L, F).unwrap().data {
            CelData::Image(b) => b.data.iter().step_by(4).copied().collect(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn resize_center_pads_evenly_and_undo_restores() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        bus.execute(
            ReframeCanvas::resize(2, 2, 4, 4, Anchor::Center).into(),
            &mut s,
            &mut c,
        )
        .unwrap();
        assert_eq!((s.width, s.height), (4, 4));
        let g = greys(&c);
        assert_eq!(g.len(), 16);
        assert_eq!(&g[5..7], &[1, 2]);
        assert_eq!(&g[9..11], &[3, 4]);
        assert_eq!(g[0], 0);
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!((s.width, s.height), (2, 2));
        assert_eq!(greys(&c), vec![1, 2, 3, 4]);
    }

    #[test]
    fn resize_anchors_pin_the_content() {
        let (mut s, mut c) = doc();
        ReframeCanvas::resize(2, 2, 3, 3, Anchor::BottomRight)
            .apply(&mut s, &mut c)
            .unwrap();
        let g = greys(&c);
        assert_eq!(g, vec![0, 0, 0, 0, 1, 2, 0, 3, 4]);
        let (mut s, mut c) = doc();
        ReframeCanvas::resize(2, 2, 1, 1, Anchor::TopLeft)
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!(greys(&c), vec![1]);
    }

    #[test]
    fn crop_rect_keeps_only_that_region_and_clears_selection() {
        let (mut s, mut c) = doc();
        s.set_selection(Rect::new(1, 0, 1, 2));
        ReframeCanvas::new(Rect::new(1, 0, 1, 2))
            .apply(&mut s, &mut c)
            .unwrap();
        assert_eq!((s.width, s.height), (1, 2));
        assert_eq!(greys(&c), vec![2, 4]);
        assert!(!s.has_selection());
    }

    #[test]
    fn empty_rect_is_rejected() {
        let (mut s, mut c) = doc();
        assert_eq!(
            ReframeCanvas::new(Rect::new(0, 0, 0, 3)).apply(&mut s, &mut c),
            Err(CommandError::EmptyRegion)
        );
    }
}
