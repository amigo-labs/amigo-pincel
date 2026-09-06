//! `MergeDown` command — composite an image layer onto the image layer
//! directly below it and remove the upper layer.
//!
//! Per frame, the lower cel's pixels are copied raw into a canvas-sized
//! buffer and the upper cel is composited over them with the upper
//! layer's opacity and blend mode (plus the cel opacity) — the same math
//! `compose()` uses, so the merged result looks exactly like the two
//! layers did. The lower layer keeps its own opacity / blend mode. Frames
//! where the upper layer has no cel are left untouched.
//!
//! The target must be an image layer that is the immediate lower sibling
//! (same parent) — the flat-Vec contiguity invariant means the layer at
//! `index - 1` is either that sibling or the tail of another subtree.

use crate::document::{
    Cel, CelData, CelMap, ColorMode, FrameIndex, Layer, LayerId, LayerKind, PixelBuffer, Sprite,
};
use crate::geometry::Rect;
use crate::render::{composite_image_cel, mul_u8};

use super::Command;
use super::error::CommandError;

/// State captured on `apply` so `revert` can restore both layers exactly.
#[derive(Debug, Clone)]
struct Prior {
    upper_index: usize,
    upper: Layer,
    upper_cels: Vec<Cel>,
    /// The lower layer's cel per touched frame before the merge (`None`
    /// when the lower layer had no cel there).
    lower_cels: Vec<(FrameIndex, Option<Cel>)>,
}

/// Merge the layer `upper` into the image layer directly below it.
#[derive(Debug, Clone)]
pub struct MergeDown {
    upper: LayerId,
    prior: Option<Prior>,
}

impl MergeDown {
    pub fn new(upper: LayerId) -> Self {
        Self { upper, prior: None }
    }
}

/// Copy `src` raw (no blending, no opacity) into the canvas-sized `dst`.
fn blit_raw(dst: &mut [u8], canvas: Rect, cel_pos: (i32, i32), src: &PixelBuffer) {
    let stride = canvas.width as usize * 4;
    for ly in 0..src.height {
        let y = i64::from(cel_pos.1) + i64::from(ly);
        if y < 0 || y >= i64::from(canvas.height) {
            continue;
        }
        for lx in 0..src.width {
            let x = i64::from(cel_pos.0) + i64::from(lx);
            if x < 0 || x >= i64::from(canvas.width) {
                continue;
            }
            let s = ((ly * src.width + lx) * 4) as usize;
            let d = y as usize * stride + x as usize * 4;
            dst[d..d + 4].copy_from_slice(&src.data[s..s + 4]);
        }
    }
}

impl Command for MergeDown {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let index = doc
            .layers
            .iter()
            .position(|l| l.id == self.upper)
            .ok_or(CommandError::UnknownLayer(self.upper.0))?;
        if !matches!(doc.layers[index].kind, LayerKind::Image) {
            return Err(CommandError::UnsupportedLayerKind(self.upper.0));
        }
        if index == 0 {
            return Err(CommandError::NoMergeTarget(self.upper.0));
        }
        let lower_id = doc.layers[index - 1].id;
        {
            let lower = &doc.layers[index - 1];
            if !matches!(lower.kind, LayerKind::Image) || lower.parent != doc.layers[index].parent {
                return Err(CommandError::NoMergeTarget(self.upper.0));
            }
        }
        let upper_layer = doc.layers[index].clone();
        let canvas = Rect::new(0, 0, doc.width, doc.height);

        let mut lower_prior = Vec::new();
        let mut upper_cels = Vec::new();
        for f in 0..doc.frames.len() {
            let frame = FrameIndex::new(f as u32);
            let Some(upper_cel) = cels.remove(self.upper, frame) else {
                continue;
            };
            let CelData::Image(upper_buf) = &upper_cel.data else {
                // Put it back before bailing so the document is untouched.
                cels.insert(upper_cel);
                self.restore(doc, cels, index, upper_layer, upper_cels, lower_prior);
                return Err(CommandError::NotAnImageCel {
                    layer: self.upper,
                    frame,
                });
            };
            if upper_buf.color_mode != ColorMode::Rgba {
                cels.insert(upper_cel);
                self.restore(doc, cels, index, upper_layer, upper_cels, lower_prior);
                return Err(CommandError::UnsupportedColorMode);
            }

            let mut out = vec![0u8; (doc.width as usize) * (doc.height as usize) * 4];
            let prev_lower = cels.remove(lower_id, frame);
            if let Some(lower_cel) = &prev_lower
                && let CelData::Image(lower_buf) = &lower_cel.data
            {
                blit_raw(&mut out, canvas, lower_cel.position, lower_buf);
            }
            let combined = mul_u8(upper_layer.opacity, upper_cel.opacity);
            composite_image_cel(
                &mut out,
                canvas,
                upper_cel.position,
                upper_buf,
                combined,
                upper_layer.blend_mode,
            );
            let mut buf = PixelBuffer::empty(doc.width, doc.height, ColorMode::Rgba);
            buf.data = out;
            cels.insert(Cel::image(lower_id, frame, buf));
            lower_prior.push((frame, prev_lower));
            upper_cels.push(upper_cel);
        }
        doc.layers.remove(index);
        self.prior = Some(Prior {
            upper_index: index,
            upper: upper_layer,
            upper_cels,
            lower_cels: lower_prior,
        });
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some(p) = self.prior.take() else {
            return;
        };
        self.restore(
            doc,
            cels,
            p.upper_index,
            p.upper,
            p.upper_cels,
            p.lower_cels,
        );
    }
}

impl MergeDown {
    /// Put the upper layer and every touched cel back. Shared by `revert`
    /// and the mid-apply bail-out paths.
    fn restore(
        &self,
        doc: &mut Sprite,
        cels: &mut CelMap,
        upper_index: usize,
        upper: Layer,
        upper_cels: Vec<Cel>,
        lower_cels: Vec<(FrameIndex, Option<Cel>)>,
    ) {
        let lower_id = if upper_index > 0 {
            doc.layers.get(upper_index - 1).map(|l| l.id)
        } else {
            None
        };
        if !doc.layers.iter().any(|l| l.id == upper.id) {
            doc.layers.insert(upper_index.min(doc.layers.len()), upper);
        }
        for cel in upper_cels {
            cels.insert(cel);
        }
        for (frame, prev) in lower_cels {
            match prev {
                Some(cel) => {
                    cels.insert(cel);
                }
                None => {
                    if let Some(lid) = lower_id {
                        cels.remove(lid, frame);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{BlendMode, Frame};

    const LOWER: LayerId = LayerId::new(0);
    const UPPER: LayerId = LayerId::new(1);
    const F0: FrameIndex = FrameIndex::new(0);

    fn solid(w: u32, h: u32, px: [u8; 4]) -> PixelBuffer {
        let mut b = PixelBuffer::empty(w, h, ColorMode::Rgba);
        b.data = px.repeat((w * h) as usize);
        b
    }

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(LOWER, "lower"))
            .add_layer(Layer::image(UPPER, "upper"))
            .add_frame(Frame::new(100))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LOWER, F0, solid(2, 2, [255, 0, 0, 255])));
        // Upper covers only the top-left pixel, offset cel.
        cels.insert(Cel::image(UPPER, F0, solid(1, 1, [0, 0, 255, 255])));
        (sprite, cels)
    }

    fn px(cels: &CelMap, layer: LayerId, i: usize) -> [u8; 4] {
        let CelData::Image(b) = &cels.get(layer, F0).unwrap().data else {
            unreachable!()
        };
        [
            b.data[i * 4],
            b.data[i * 4 + 1],
            b.data[i * 4 + 2],
            b.data[i * 4 + 3],
        ]
    }

    #[test]
    fn apply_composites_upper_onto_lower_and_removes_upper() {
        let (mut s, mut c) = doc();
        let mut cmd = MergeDown::new(UPPER);
        cmd.apply(&mut s, &mut c).unwrap();
        assert_eq!(s.layers.len(), 1);
        assert_eq!(s.layers[0].id, LOWER);
        assert!(c.get(UPPER, F0).is_none());
        assert_eq!(px(&c, LOWER, 0), [0, 0, 255, 255], "upper pixel wins");
        assert_eq!(px(&c, LOWER, 3), [255, 0, 0, 255], "lower shows through");
    }

    #[test]
    fn apply_uses_upper_layer_opacity() {
        let (mut s, mut c) = doc();
        s.layers[1].opacity = 0;
        let mut cmd = MergeDown::new(UPPER);
        cmd.apply(&mut s, &mut c).unwrap();
        assert_eq!(
            px(&c, LOWER, 0),
            [255, 0, 0, 255],
            "fully transparent upper is a no-op"
        );
    }

    #[test]
    fn apply_uses_upper_blend_mode() {
        let (mut s, mut c) = doc();
        s.layers[1].blend_mode = BlendMode::Multiply;
        let mut cmd = MergeDown::new(UPPER);
        cmd.apply(&mut s, &mut c).unwrap();
        assert_eq!(px(&c, LOWER, 0), [0, 0, 0, 255], "red × blue = black");
    }

    #[test]
    fn revert_restores_both_layers_and_cels_exactly() {
        let (mut s, mut c) = doc();
        let (s0, c0) = (s.clone(), c.clone());
        let mut bus = Bus::new();
        bus.execute(MergeDown::new(UPPER).into(), &mut s, &mut c)
            .unwrap();
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(s, s0);
        assert_eq!(c, c0);
        assert!(bus.redo(&mut s, &mut c).unwrap());
        assert_eq!(s.layers.len(), 1);
    }

    #[test]
    fn merge_creates_a_lower_cel_when_none_existed() {
        let (mut s, mut c) = doc();
        c.remove(LOWER, F0);
        let mut cmd = MergeDown::new(UPPER);
        cmd.apply(&mut s, &mut c).unwrap();
        assert_eq!(px(&c, LOWER, 0), [0, 0, 255, 255]);
        assert_eq!(px(&c, LOWER, 3), [0, 0, 0, 0]);
        cmd.revert(&mut s, &mut c);
        assert!(c.get(LOWER, F0).is_none());
        assert!(c.get(UPPER, F0).is_some());
    }

    #[test]
    fn bottom_layer_group_and_cross_parent_are_rejected() {
        let (mut s, mut c) = doc();
        assert_eq!(
            MergeDown::new(LOWER).apply(&mut s, &mut c),
            Err(CommandError::NoMergeTarget(0))
        );
        s.layers.push(Layer::group(LayerId::new(2), "g"));
        assert_eq!(
            MergeDown::new(LayerId::new(2)).apply(&mut s, &mut c),
            Err(CommandError::UnsupportedLayerKind(2))
        );
        let mut child = Layer::image(LayerId::new(3), "child");
        child.parent = Some(LayerId::new(2));
        s.layers.push(child);
        // `child` sits directly above the group entry, not an image sibling.
        assert_eq!(
            MergeDown::new(LayerId::new(3)).apply(&mut s, &mut c),
            Err(CommandError::NoMergeTarget(3))
        );
    }
}
