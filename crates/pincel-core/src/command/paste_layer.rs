//! `PasteLayer` command — an image as a new image layer, in one undo step
//! (spec §5.4 Paste, Decision Log 2026-10-29). Backs Edit ▸ Paste and
//! File ▸ Import PNG as layer.
//!
//! On apply: optionally grow the canvas (top-left anchor) so it covers the
//! image at its position, insert the layer, and give it a canvas-sized cel
//! on the target frame holding the image at `(x, y)`, cropped to the
//! canvas. Revert undoes the three in reverse.

use crate::document::{Cel, CelMap, ColorMode, FrameIndex, Layer, LayerId, PixelBuffer, Sprite};

use super::error::CommandError;
use super::{AddLayer, Anchor, Command, ReframeCanvas};

/// Decoded straight-alpha RGBA8 pixels to paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PastedImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major RGBA8, `width × height × 4` bytes.
    pub rgba: Vec<u8>,
}

/// Paste an image as a new image layer. See the module docs.
#[derive(Debug, Clone)]
pub struct PasteLayer {
    layer: Box<Layer>,
    frame: FrameIndex,
    image: PastedImage,
    x: i32,
    y: i32,
    above: Option<LayerId>,
    expand: bool,
    applied: Option<Box<Applied>>,
}

#[derive(Debug, Clone)]
struct Applied {
    reframe: Option<ReframeCanvas>,
    add: AddLayer,
}

impl PasteLayer {
    /// Paste `image` as `layer` (an image layer whose id is not in use) on
    /// `frame`, at the canvas origin, on top of the stack, without growing
    /// the canvas. Refine with [`Self::at`], [`Self::above`] and
    /// [`Self::expanding`].
    pub fn new(layer: Layer, frame: FrameIndex, image: PastedImage) -> Self {
        Self {
            layer: Box::new(layer),
            frame,
            image,
            x: 0,
            y: 0,
            above: None,
            expand: false,
            applied: None,
        }
    }

    /// Put the image's top-left corner at sprite `(x, y)`.
    pub fn at(mut self, x: i32, y: i32) -> Self {
        self.x = x;
        self.y = y;
        self
    }

    /// Insert the layer directly above `layer` (above its whole subtree for
    /// a group), as its sibling. An unknown id falls back to the top.
    pub fn above(mut self, layer: Option<LayerId>) -> Self {
        self.above = layer;
        self
    }

    /// With `true`, grow the canvas (anchored top-left) so it covers the
    /// image at its position; otherwise the image is cropped to the canvas.
    pub fn expanding(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }

    /// Z-index and parent for the new layer.
    fn placement(&self, doc: &Sprite) -> (usize, Option<LayerId>) {
        let Some(anchor) = self.above.and_then(|id| doc.layer(id)) else {
            return (doc.layers.len(), None);
        };
        let top = doc
            .layers
            .iter()
            .enumerate()
            .filter(|(_, l)| descends_from(doc, l.id, anchor.id))
            .map(|(i, _)| i)
            .max()
            .unwrap_or(doc.layers.len().saturating_sub(1));
        (top + 1, anchor.parent)
    }
}

/// `true` when `id` is `root` or nested (at any depth) in it.
fn descends_from(doc: &Sprite, id: LayerId, root: LayerId) -> bool {
    let mut next = Some(id);
    for _ in 0..=doc.layers.len() {
        match next {
            Some(i) if i == root => return true,
            Some(i) => next = doc.layer(i).and_then(|l| l.parent),
            None => return false,
        }
    }
    false
}

impl Command for PasteLayer {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        // Writing indexed cels is Phase 2 (spec §3.7): refuse, unchanged.
        if !matches!(doc.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }
        let img = &self.image;
        if img.width == 0
            || img.height == 0
            || img.rgba.len() != img.width as usize * img.height as usize * 4
        {
            return Err(CommandError::EmptyRegion);
        }
        if doc.layers.iter().any(|l| l.id == self.layer.id) {
            return Err(CommandError::DuplicateLayerId(self.layer.id.0));
        }
        let (index, parent) = self.placement(doc);

        let mut reframe = None;
        if self.expand {
            let right = i64::from(self.x) + i64::from(img.width);
            let bottom = i64::from(self.y) + i64::from(img.height);
            let w = right.max(i64::from(doc.width));
            let h = bottom.max(i64::from(doc.height));
            let (w, h) = (
                u32::try_from(w).map_err(|_| CommandError::EmptyRegion)?,
                u32::try_from(h).map_err(|_| CommandError::EmptyRegion)?,
            );
            if (w, h) != (doc.width, doc.height) {
                let mut cmd = ReframeCanvas::resize(doc.width, doc.height, w, h, Anchor::TopLeft);
                cmd.apply(doc, cels)?;
                reframe = Some(cmd);
            }
        }

        let mut layer = (*self.layer).clone();
        layer.parent = parent;
        let mut add = AddLayer::at(layer, index);
        if let Err(err) = add.apply(doc, cels) {
            if let Some(mut r) = reframe {
                r.revert(doc, cels);
            }
            return Err(err);
        }

        let (cw, ch) = (doc.width, doc.height);
        let mut buf = PixelBuffer::empty(cw, ch, ColorMode::Rgba);
        for sy in 0..img.height {
            let cy = i64::from(self.y) + i64::from(sy);
            if !(0..i64::from(ch)).contains(&cy) {
                continue;
            }
            for sx in 0..img.width {
                let cx = i64::from(self.x) + i64::from(sx);
                if !(0..i64::from(cw)).contains(&cx) {
                    continue;
                }
                let si = ((sy * img.width + sx) * 4) as usize;
                let di = ((cy as usize) * cw as usize + cx as usize) * 4;
                buf.data[di..di + 4].copy_from_slice(&img.rgba[si..si + 4]);
            }
        }
        cels.insert(Cel::image(self.layer.id, self.frame, buf));
        self.applied = Some(Box::new(Applied { reframe, add }));
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some(mut applied) = self.applied.take() else {
            return;
        };
        cels.remove(self.layer.id, self.frame);
        applied.add.revert(doc, cels);
        if let Some(mut r) = applied.reframe {
            r.revert(doc, cels);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{CelData, Frame};

    const GREEN: [u8; 4] = [0, 255, 0, 255];

    fn doc(w: u32, h: u32) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(w, h)
            .add_layer(Layer::image(LayerId::new(0), "a"))
            .add_layer(Layer::image(LayerId::new(1), "b"))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite");
        (sprite, CelMap::new())
    }

    fn image(w: u32, h: u32) -> PastedImage {
        PastedImage {
            width: w,
            height: h,
            rgba: GREEN.repeat((w * h) as usize),
        }
    }

    fn pixel(cels: &CelMap, layer: u32, frame: u32, x: u32, y: u32) -> [u8; 4] {
        let cel = cels
            .get(LayerId::new(layer), FrameIndex::new(frame))
            .expect("cel");
        let CelData::Image(buf) = &cel.data else {
            panic!("not an image cel")
        };
        let i = ((y * buf.width + x) * 4) as usize;
        [
            buf.data[i],
            buf.data[i + 1],
            buf.data[i + 2],
            buf.data[i + 3],
        ]
    }

    fn paste(img: PastedImage) -> PasteLayer {
        PasteLayer::new(
            Layer::image(LayerId::new(7), "pasted"),
            FrameIndex::new(1),
            img,
        )
    }

    #[test]
    fn paste_layer_inserts_above_the_anchor_on_the_frame_at_the_position() {
        let (mut sprite, mut cels) = doc(8, 8);
        let mut cmd = paste(image(2, 3)).at(5, 4).above(Some(LayerId::new(0)));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        let ids: Vec<u32> = sprite.layers.iter().map(|l| l.id.0).collect();
        assert_eq!(ids, vec![0, 7, 1]);
        assert_eq!(pixel(&cels, 7, 1, 5, 4), GREEN);
        assert_eq!(pixel(&cels, 7, 1, 6, 6), GREEN);
        assert_eq!(pixel(&cels, 7, 1, 4, 4), [0; 4]);
        assert_eq!(pixel(&cels, 7, 1, 7, 7), [0; 4]);
        assert!(cels.get(LayerId::new(7), FrameIndex::new(0)).is_none());
    }

    #[test]
    fn paste_layer_without_expand_crops_to_the_canvas() {
        let (mut sprite, mut cels) = doc(4, 4);
        let mut cmd = paste(image(6, 6)).at(1, 2);
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!((sprite.width, sprite.height), (4, 4));
        assert_eq!(pixel(&cels, 7, 1, 3, 3), GREEN);
        assert_eq!(pixel(&cels, 7, 1, 0, 0), [0; 4]);
    }

    #[test]
    fn paste_layer_expand_grows_the_canvas_to_cover_the_image_at_its_position() {
        let (mut sprite, mut cels) = doc(4, 4);
        let mut cmd = paste(image(6, 5)).at(1, 2).expanding(true);
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!((sprite.width, sprite.height), (7, 7));
        assert_eq!(pixel(&cels, 7, 1, 6, 6), GREEN);
        assert_eq!(pixel(&cels, 7, 1, 1, 2), GREEN);
    }

    #[test]
    fn paste_layer_one_undo_removes_layer_pixels_and_expansion() {
        let (mut sprite, mut cels) = doc(4, 4);
        let mut bus = Bus::new();
        let cmd = paste(image(6, 5)).at(1, 2).expanding(true);
        bus.execute(cmd.into(), &mut sprite, &mut cels)
            .expect("paste");
        assert_eq!(bus.undo_depth(), 1);
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!((sprite.width, sprite.height), (4, 4));
        assert_eq!(sprite.layers.len(), 2);
        assert!(cels.get(LayerId::new(7), FrameIndex::new(1)).is_none());
        assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
        assert_eq!((sprite.width, sprite.height), (7, 7));
        assert_eq!(pixel(&cels, 7, 1, 6, 6), GREEN);
    }

    #[test]
    fn paste_layer_above_a_group_lands_above_its_subtree_as_its_sibling() {
        let (mut sprite, mut cels) = doc(4, 4);
        sprite.layers.push(Layer::group(LayerId::new(2), "g"));
        sprite.layers[1].parent = Some(LayerId::new(2));
        // Child above its group in the flat list, as some files store it.
        sprite.layers.swap(1, 2);
        let mut cmd = paste(image(1, 1)).above(Some(LayerId::new(2)));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        let ids: Vec<u32> = sprite.layers.iter().map(|l| l.id.0).collect();
        assert_eq!(ids, vec![0, 2, 1, 7]);
        assert_eq!(sprite.layers[3].parent, None);
    }

    #[test]
    fn paste_layer_into_indexed_sprite_is_refused_unchanged() {
        let (mut sprite, mut cels) = doc(4, 4);
        sprite.color_mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        let mut cmd = paste(image(1, 1));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels).err(),
            Some(CommandError::UnsupportedColorMode)
        );
        assert_eq!(sprite.layers.len(), 2);
        assert!(cels.is_empty());
    }

    #[test]
    fn paste_layer_rejects_a_taken_id_and_a_malformed_image() {
        let (mut sprite, mut cels) = doc(4, 4);
        let mut taken = PasteLayer::new(
            Layer::image(LayerId::new(1), "x"),
            FrameIndex::new(0),
            image(1, 1),
        );
        assert!(taken.apply(&mut sprite, &mut cels).is_err());
        let mut bad = paste(PastedImage {
            width: 2,
            height: 2,
            rgba: vec![0; 3],
        });
        assert!(bad.apply(&mut sprite, &mut cels).is_err());
        assert_eq!(sprite.layers.len(), 2);
    }
}
