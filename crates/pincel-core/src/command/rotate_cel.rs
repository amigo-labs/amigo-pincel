//! `RotateCel` command — rotate the image content of one cel by any angle
//! with nearest neighbour or RotSprite. See `docs/specs/rotsprite.md`.
//!
//! Without a selection the input is the bounding box of the cel's visible
//! (alpha > 0) pixels; with one it is the selection rect (and mask). The
//! input turns about its centre, the cel's bounds grow to hold the whole
//! result (never cropped), and visible rotated pixels replace what lies
//! under them. `revert` restores the exact prior cel.

use crate::document::{Cel, CelData, CelMap, ColorMode, FrameIndex, LayerId, PixelBuffer, Sprite};
use crate::geometry::Rect;
use crate::selection::SelectionMask;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;
use super::rotsprite::{ROTSPRITE_MAX_SIDE, RotateMethod, rotate};
use super::transform_support::{Raster, rgba_buffer};

/// Rotate the image content of the cel at `(layer, frame)` clockwise by
/// `angle` degrees.
#[derive(Debug, Clone)]
pub struct RotateCel {
    layer: LayerId,
    frame: FrameIndex,
    angle: i32,
    method: RotateMethod,
    within: Option<Rect>,
    mask: Option<SelectionMask>,
    prior: Option<Cel>,
    dirty: Option<Rect>,
}

impl RotateCel {
    /// Rotate the whole visible content of the cel by `angle` degrees
    /// (positive turns clockwise) with `method`.
    pub fn new(layer: LayerId, frame: FrameIndex, angle: i32, method: RotateMethod) -> Self {
        Self {
            layer,
            frame,
            angle,
            method,
            within: None,
            mask: None,
            prior: None,
            dirty: None,
        }
    }

    /// Rotate only the pixels inside `rect` (sprite space, the selection),
    /// about the rect's centre.
    pub fn within(mut self, rect: Rect) -> Self {
        self.within = Some(rect);
        self
    }

    /// Restrict the rotated pixels to a shaped selection; only meaningful
    /// together with [`RotateCel::within`] (the mask's bounding box).
    pub fn masked(mut self, mask: SelectionMask) -> Self {
        self.mask = Some(mask);
        self
    }

    /// The sprite-space rect this rotation turns, or `None` when there is
    /// nothing visible to rotate. Errors when the cel is missing or not an
    /// RGBA image cel.
    pub fn input_rect(&self, cels: &CelMap) -> Result<Option<Rect>, CommandError> {
        let cel = self.cel(cels)?;
        let buffer = self.buffer(cel)?;
        let cel_rect = Rect::new(cel.position.0, cel.position.1, buffer.width, buffer.height);
        let mut bounds: Option<(i32, i32, i32, i32)> = None;
        for y in 0..buffer.height {
            for x in 0..buffer.width {
                let (sx, sy) = (cel_rect.x + x as i32, cel_rect.y + y as i32);
                if !self.selected(sx, sy) || pixel(buffer, x, y)[3] == 0 {
                    continue;
                }
                bounds = Some(match bounds {
                    None => (sx, sy, sx, sy),
                    Some((x0, y0, x1, y1)) => (x0.min(sx), y0.min(sy), x1.max(sx), y1.max(sy)),
                });
            }
        }
        let Some((x0, y0, x1, y1)) = bounds else {
            return Ok(None);
        };
        Ok(Some(self.within.unwrap_or(Rect::new(
            x0,
            y0,
            (x1 - x0 + 1) as u32,
            (y1 - y0 + 1) as u32,
        ))))
    }

    fn cel<'a>(&self, cels: &'a CelMap) -> Result<&'a Cel, CommandError> {
        cels.get(self.layer, self.frame)
            .ok_or(CommandError::MissingCel {
                layer: self.layer,
                frame: self.frame,
            })
    }

    fn buffer<'a>(&self, cel: &'a Cel) -> Result<&'a PixelBuffer, CommandError> {
        rgba_buffer(cel).ok_or(CommandError::NotAnImageCel {
            layer: self.layer,
            frame: self.frame,
        })
    }

    /// `true` when the sprite pixel belongs to the rotated content.
    fn selected(&self, x: i32, y: i32) -> bool {
        let in_rect = self.within.is_none_or(|r| {
            x >= r.x && y >= r.y && x < r.x + r.width as i32 && y < r.y + r.height as i32
        });
        in_rect && self.mask.as_ref().is_none_or(|m| m.contains(x, y))
    }
}

fn pixel(buffer: &PixelBuffer, x: u32, y: u32) -> [u8; 4] {
    let i = (y as usize * buffer.width as usize + x as usize) * 4;
    [
        buffer.data[i],
        buffer.data[i + 1],
        buffer.data[i + 2],
        buffer.data[i + 3],
    ]
}

/// Smallest rect holding both `a` and `b`.
fn union(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = (a.x + a.width as i32).max(b.x + b.width as i32);
    let y1 = (a.y + a.height as i32).max(b.y + b.height as i32);
    Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32)
}

/// Bounding box of the visible pixels of `r`, relative to `r`.
fn visible_bounds(r: &Raster) -> Option<Rect> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for y in 0..r.height {
        for x in 0..r.width {
            if r.get(x, y)[3] > 0 {
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
    }
    b.map(|(x0, y0, x1, y1)| Rect::new(x0 as i32, y0 as i32, x1 - x0 + 1, y1 - y0 + 1))
}

impl Command for RotateCel {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let target = self.input_rect(cels)?.ok_or(CommandError::EmptyRegion)?;
        if self.method == RotateMethod::RotSprite
            && (target.width > ROTSPRITE_MAX_SIDE || target.height > ROTSPRITE_MAX_SIDE)
        {
            return Err(CommandError::RotSpriteTooLarge {
                width: target.width,
                height: target.height,
                max: ROTSPRITE_MAX_SIDE,
            });
        }
        let cel = self.cel(cels)?;
        let buffer = self.buffer(cel)?;
        let cel_rect = Rect::new(cel.position.0, cel.position.1, buffer.width, buffer.height);

        // The rotated content: selected visible pixels of the target box.
        let mut src = Raster::transparent(target.width, target.height);
        for y in 0..target.height {
            for x in 0..target.width {
                let (sx, sy) = (target.x + x as i32, target.y + y as i32);
                let (cx, cy) = (sx - cel_rect.x, sy - cel_rect.y);
                if cx < 0 || cy < 0 || cx >= buffer.width as i32 || cy >= buffer.height as i32 {
                    continue;
                }
                if self.selected(sx, sy) {
                    src.put(x, y, pixel(buffer, cx as u32, cy as u32));
                }
            }
        }
        let rotated = rotate(&src, self.angle, self.method);
        let placed = Rect::new(
            target.x + rotated.offset.0 as i32,
            target.y + rotated.offset.1 as i32,
            rotated.raster.width,
            rotated.raster.height,
        );
        // Grow the cel to hold every visible rotated pixel.
        let bounds = visible_bounds(&rotated.raster).map_or(cel_rect, |v| {
            union(
                cel_rect,
                Rect::new(placed.x + v.x, placed.y + v.y, v.width, v.height),
            )
        });
        let mut out = Raster::transparent(bounds.width, bounds.height);
        let old = Raster {
            width: buffer.width,
            height: buffer.height,
            data: buffer.data.clone(),
        };
        out.blit(
            &old,
            i64::from(cel_rect.x - bounds.x),
            i64::from(cel_rect.y - bounds.y),
        );
        // Lift the content off; fully transparent pixels stay byte-exact.
        for y in 0..target.height {
            for x in 0..target.width {
                if src.get(x, y)[3] > 0 {
                    let ox = (target.x + x as i32 - bounds.x) as u32;
                    let oy = (target.y + y as i32 - bounds.y) as u32;
                    out.put(ox, oy, [0, 0, 0, 0]);
                }
            }
        }
        // Visible rotated pixels replace what is under them (no blending,
        // so no new colours).
        for y in 0..placed.height {
            for x in 0..placed.width {
                let px = rotated.raster.get(x, y);
                if px[3] > 0 {
                    let ox = (placed.x + x as i32 - bounds.x) as u32;
                    let oy = (placed.y + y as i32 - bounds.y) as u32;
                    out.put(ox, oy, px);
                }
            }
        }

        let prior = cels
            .remove(self.layer, self.frame)
            .expect("cel presence checked above");
        let mut next = prior.clone();
        next.position = (bounds.x, bounds.y);
        let mut buf = PixelBuffer::empty(bounds.width, bounds.height, ColorMode::Rgba);
        buf.data = out.data;
        next.data = CelData::Image(buf);
        cels.insert(next);
        self.prior = Some(prior);
        self.dirty = Some(union(target, placed));
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(prior) = self.prior.take() else {
            return;
        };
        cels.insert(prior);
    }

    fn dirty_region(&self) -> DirtyRegion {
        match self.dirty {
            Some(r) => DirtyRegion::layer_rect(self.layer, self.frame, r),
            None => DirtyRegion::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::rotsprite::rotate;
    use crate::command::{Bus, Orientation, TransformCel};
    use crate::document::{Frame, Layer};

    const L: LayerId = LayerId::new(0);
    const L2: LayerId = LayerId::new(1);
    const F: FrameIndex = FrameIndex::new(0);
    const F2: FrameIndex = FrameIndex::new(1);
    const METHODS: [RotateMethod; 2] = [RotateMethod::Nearest, RotateMethod::RotSprite];

    fn sprite(w: u32, h: u32) -> Sprite {
        Sprite::builder(w, h)
            .add_layer(Layer::image(L, "a"))
            .add_layer(Layer::image(L2, "b"))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(100))
            .build()
            .unwrap()
    }

    /// A cel at `pos` whose pixel `(x, y)` is `f(x, y)`.
    fn cel(
        layer: LayerId,
        frame: FrameIndex,
        pos: (i32, i32),
        w: u32,
        h: u32,
        f: impl Fn(u32, u32) -> [u8; 4],
    ) -> Cel {
        let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
        for y in 0..h {
            for x in 0..w {
                let i = (y as usize * w as usize + x as usize) * 4;
                buf.data[i..i + 4].copy_from_slice(&f(x, y));
            }
        }
        let mut c = Cel::image(layer, frame, buf);
        c.position = pos;
        c
    }

    /// Distinct opaque colour per pixel.
    fn pattern(x: u32, y: u32) -> [u8; 4] {
        [
            (x * 31 + 7) as u8,
            (y * 47 + 3) as u8,
            ((x ^ y) * 13) as u8,
            255,
        ]
    }

    /// The pixel at sprite `(x, y)` of the cel, transparent outside it.
    fn at(cels: &CelMap, layer: LayerId, frame: FrameIndex, x: i32, y: i32) -> [u8; 4] {
        let c = cels.get(layer, frame).unwrap();
        let CelData::Image(b) = &c.data else {
            unreachable!()
        };
        let (cx, cy) = (x - c.position.0, y - c.position.1);
        if cx < 0 || cy < 0 || cx >= b.width as i32 || cy >= b.height as i32 {
            return [0, 0, 0, 0];
        }
        pixel(b, cx as u32, cy as u32)
    }

    fn bounds(cels: &CelMap) -> Rect {
        let c = cels.get(L, F).unwrap();
        let CelData::Image(b) = &c.data else {
            unreachable!()
        };
        Rect::new(c.position.0, c.position.1, b.width, b.height)
    }

    /// The rotation of `src` (sitting at `at_rect`) placed in sprite space.
    fn expected(src: &Raster, at_rect: Rect, angle: i32, method: RotateMethod) -> (Rect, Raster) {
        let r = rotate(src, angle, method);
        let placed = Rect::new(
            at_rect.x + r.offset.0 as i32,
            at_rect.y + r.offset.1 as i32,
            r.raster.width,
            r.raster.height,
        );
        (placed, r.raster)
    }

    #[test]
    fn rotate_cel_by_zero_keeps_every_pixel_and_the_bounds() {
        for method in METHODS {
            let mut s = sprite(6, 5);
            let mut cels = CelMap::new();
            // A trimmed cel with a hidden-colour transparent pixel.
            cels.insert(cel(L, F, (2, 1), 3, 3, |x, y| {
                if (x, y) == (1, 1) {
                    [5, 5, 5, 0]
                } else {
                    pattern(x, y)
                }
            }));
            let before = cels.get(L, F).unwrap().clone();
            RotateCel::new(L, F, 0, method)
                .apply(&mut s, &mut cels)
                .unwrap();
            assert_eq!(cels.get(L, F).unwrap(), &before, "{method:?}");
            // With a selection that reaches past the cel too.
            RotateCel::new(L, F, 0, method)
                .within(Rect::new(1, 0, 3, 3))
                .apply(&mut s, &mut cels)
                .unwrap();
            assert_eq!(cels.get(L, F).unwrap(), &before, "{method:?} selection");
        }
    }

    #[test]
    fn rotate_cel_rotsprite_90_matches_the_quarter_turn() {
        let mut s = sprite(5, 5);
        let mut a = CelMap::new();
        a.insert(cel(L, F, (0, 0), 5, 5, pattern));
        let mut b = a.clone();
        RotateCel::new(L, F, 90, RotateMethod::RotSprite)
            .apply(&mut s, &mut a)
            .unwrap();
        TransformCel::new(L, F, Orientation::Rotate90Cw)
            .apply(&mut s, &mut b)
            .unwrap();
        assert_eq!(a.get(L, F), b.get(L, F));
    }

    #[test]
    fn rotate_cel_adds_no_colours() {
        let palette = [[200, 10, 10, 255], [10, 200, 10, 128], [0, 0, 0, 0]];
        for method in METHODS {
            for within in [None, Some(Rect::new(1, 1, 5, 4))] {
                let mut s = sprite(8, 8);
                let mut cels = CelMap::new();
                cels.insert(cel(L, F, (0, 0), 8, 8, |x, y| {
                    palette[((x * 3 + y * 5) % 3) as usize]
                }));
                let mut cmd = RotateCel::new(L, F, 30, method);
                if let Some(r) = within {
                    cmd = cmd.within(r);
                }
                cmd.apply(&mut s, &mut cels).unwrap();
                let b = bounds(&cels);
                for y in b.y..b.y + b.height as i32 {
                    for x in b.x..b.x + b.width as i32 {
                        let p = at(&cels, L, F, x, y);
                        assert!(p[3] == 0 || palette.contains(&p), "{method:?}: {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn rotate_cel_grows_bounds_and_reverts() {
        for method in METHODS {
            // A bar along the top edge: turned 45° it reaches above the canvas.
            let mut s = sprite(8, 8);
            let mut cels = CelMap::new();
            cels.insert(cel(L, F, (0, 0), 8, 8, |x, y| {
                if y < 2 { pattern(x, y) } else { [0, 0, 0, 0] }
            }));
            let before = cels.get(L, F).unwrap().clone();
            let mut src = Raster::transparent(8, 2);
            for y in 0..2 {
                for x in 0..8 {
                    src.put(x, y, pattern(x, y));
                }
            }
            let (placed, rot) = expected(&src, Rect::new(0, 0, 8, 2), 45, method);
            let mut bus = Bus::new();
            bus.execute(RotateCel::new(L, F, 45, method).into(), &mut s, &mut cels)
                .unwrap();
            let b = bounds(&cels);
            assert!(b.y < 0, "{method:?}: bounds grew past the top edge: {b:?}");
            for y in 0..rot.height {
                for x in 0..rot.width {
                    let p = rot.get(x, y);
                    if p[3] > 0 {
                        let (sx, sy) = (placed.x + x as i32, placed.y + y as i32);
                        assert_eq!(at(&cels, L, F, sx, sy), p, "{method:?} at ({sx},{sy})");
                    }
                }
            }
            assert_eq!(bus.undo_depth(), 1, "{method:?}: one undo entry");
            assert!(bus.undo(&mut s, &mut cels));
            assert_eq!(
                cels.get(L, F).unwrap(),
                &before,
                "{method:?}: undo restores"
            );
            assert_eq!(bus.redo(&mut s, &mut cels), Ok(true));
            assert_eq!(bounds(&cels), b, "{method:?}: redo");
        }
    }

    #[test]
    fn rotate_cel_within_selection_keeps_uncovered_outside_pixels() {
        let shapes = [
            (Rect::new(2, 1, 4, 3), false),
            (Rect::new(2, 2, 5, 5), true),
        ];
        for method in METHODS {
            for (sel, shaped) in shapes {
                let mut s = sprite(9, 9);
                let mut cels = CelMap::new();
                cels.insert(cel(L, F, (0, 0), 9, 9, pattern));
                let before = cels.clone();
                let mask = SelectionMask::ellipse(9, 9, sel);
                let inside = |x: i32, y: i32| {
                    x >= sel.x
                        && y >= sel.y
                        && x < sel.x + sel.width as i32
                        && y < sel.y + sel.height as i32
                        && (!shaped || mask.contains(x, y))
                };
                let mut src = Raster::transparent(sel.width, sel.height);
                for y in 0..sel.height {
                    for x in 0..sel.width {
                        let (sx, sy) = (sel.x + x as i32, sel.y + y as i32);
                        if inside(sx, sy) {
                            src.put(x, y, pattern(sx as u32, sy as u32));
                        }
                    }
                }
                let (placed, rot) = expected(&src, sel, 45, method);
                let covered = |x: i32, y: i32| {
                    let (rx, ry) = (x - placed.x, y - placed.y);
                    rx >= 0
                        && ry >= 0
                        && rx < rot.width as i32
                        && ry < rot.height as i32
                        && rot.get(rx as u32, ry as u32)[3] > 0
                };
                let mut cmd = RotateCel::new(L, F, 45, method).within(sel);
                if shaped {
                    cmd = cmd.masked(mask.clone());
                }
                cmd.apply(&mut s, &mut cels).unwrap();
                for y in -3..12 {
                    for x in -3..12 {
                        let got = at(&cels, L, F, x, y);
                        let ctx = format!("{method:?} shaped={shaped} ({x},{y})");
                        if covered(x, y) {
                            let p = rot.get((x - placed.x) as u32, (y - placed.y) as u32);
                            assert_eq!(got, p, "{ctx}: rotated content");
                        } else if inside(x, y) {
                            assert_eq!(got[3], 0, "{ctx}: lifted off");
                        } else {
                            assert_eq!(got, at(&before, L, F, x, y), "{ctx}: untouched");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rotsprite_input_over_512_is_refused_and_nearest_is_not() {
        let mut s = sprite(600, 10);
        let mut cels = CelMap::new();
        cels.insert(cel(L, F, (0, 0), 513, 10, |_, _| [9, 9, 9, 255]));
        let before = cels.clone();
        let mut bus = Bus::new();
        assert_eq!(
            bus.execute(
                RotateCel::new(L, F, 30, RotateMethod::RotSprite).into(),
                &mut s,
                &mut cels
            ),
            Err(CommandError::RotSpriteTooLarge {
                width: 513,
                height: 10,
                max: ROTSPRITE_MAX_SIDE
            })
        );
        assert_eq!(cels, before);
        assert_eq!(bus.undo_depth(), 0);
        // 512 still passes the cap.
        let mut at_cap = CelMap::new();
        at_cap.insert(cel(L, F, (0, 0), 512, 2, |_, _| [9, 9, 9, 255]));
        RotateCel::new(L, F, 30, RotateMethod::RotSprite)
            .apply(&mut s, &mut at_cap)
            .unwrap();
        bus.execute(
            RotateCel::new(L, F, 30, RotateMethod::Nearest).into(),
            &mut s,
            &mut cels,
        )
        .unwrap();
        assert_ne!(cels, before);
    }

    #[test]
    fn rotate_cel_without_visible_content_changes_nothing() {
        let mut s = sprite(6, 6);
        let mut cels = CelMap::new();
        cels.insert(cel(L, F, (0, 0), 6, 6, |x, y| {
            if (x, y) == (5, 5) {
                [1, 2, 3, 255]
            } else {
                [7, 7, 7, 0]
            }
        }));
        let before = cels.clone();
        let mut bus = Bus::new();
        // A selection over transparent pixels only, and a layer without a cel.
        for cmd in [
            RotateCel::new(L, F, 30, RotateMethod::Nearest).within(Rect::new(0, 0, 3, 3)),
            RotateCel::new(L2, F, 30, RotateMethod::Nearest),
        ] {
            assert!(bus.execute(cmd.into(), &mut s, &mut cels).is_err());
        }
        let mut empty = CelMap::new();
        empty.insert(cel(L, F, (0, 0), 6, 6, |_, _| [0, 0, 0, 0]));
        let empty_before = empty.clone();
        assert_eq!(
            bus.execute(
                RotateCel::new(L, F, 30, RotateMethod::RotSprite).into(),
                &mut s,
                &mut empty
            ),
            Err(CommandError::EmptyRegion)
        );
        assert_eq!(cels, before);
        assert_eq!(empty, empty_before);
        assert_eq!(bus.undo_depth(), 0);
    }

    #[test]
    fn rotate_cel_leaves_other_layers_and_frames_alone() {
        let mut s = sprite(6, 6);
        let mut cels = CelMap::new();
        for (l, f) in [(L, F), (L, F2), (L2, F), (L2, F2)] {
            cels.insert(cel(l, f, (0, 0), 6, 6, pattern));
        }
        let before = cels.clone();
        RotateCel::new(L, F, 30, RotateMethod::RotSprite)
            .apply(&mut s, &mut cels)
            .unwrap();
        assert_ne!(cels.get(L, F), before.get(L, F));
        for (l, f) in [(L, F2), (L2, F), (L2, F2)] {
            assert_eq!(cels.get(l, f), before.get(l, f), "{l:?} {f:?}");
        }
    }
}
