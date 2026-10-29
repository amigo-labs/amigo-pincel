//! `PatchCel` — swap one rectangle of an image cel's pixels, undoably —
//! and `AddEmptyCel`, which seeds a transparent cel without storing its
//! pixels.
//!
//! The undo payload is the changed rectangle only (dirty-rect deltas, spec
//! §6.3): `apply` and `revert` both swap the held bytes with the cel's, so
//! the command always holds exactly one copy of the rectangle.

use crate::document::{Cel, CelData, CelMap, ColorMode, FrameIndex, LayerId, PixelBuffer, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Replace the cel-local rectangle `(x, y, width, height)` of the image
/// cel at `(layer, frame)` with `pixels` (RGBA8, row-major).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchCel {
    layer: LayerId,
    frame: FrameIndex,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    /// Before `apply`: the new pixels. After `apply`: the prior pixels.
    pixels: Vec<u8>,
    /// Sprite-space rect of the patch, captured on `apply`.
    sprite_rect: Option<Rect>,
    applied: bool,
}

impl PatchCel {
    /// Patch the cel-local rectangle at `(x, y)` of `width`×`height` with
    /// `pixels` (`width * height * 4` bytes).
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        (x, y): (u32, u32),
        (width, height): (u32, u32),
        pixels: Vec<u8>,
    ) -> Self {
        Self {
            layer,
            frame,
            x,
            y,
            width,
            height,
            pixels,
            sprite_rect: None,
            applied: false,
        }
    }

    /// Bytes of pixel data this command holds (the patch rectangle).
    pub fn stored_bytes(&self) -> usize {
        self.pixels.len()
    }

    /// Swap the held bytes with the cel's rectangle.
    fn swap(&mut self, buffer: &mut PixelBuffer) {
        let stride = buffer.width as usize * 4;
        let span = self.width as usize * 4;
        for row in 0..self.height as usize {
            let start = (self.y as usize + row) * stride + self.x as usize * 4;
            buffer.data[start..start + span]
                .swap_with_slice(&mut self.pixels[row * span..(row + 1) * span]);
        }
    }
}

impl Command for PatchCel {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let (layer, frame) = (self.layer, self.frame);
        let cel = cels
            .get_mut(layer, frame)
            .ok_or(CommandError::MissingCel { layer, frame })?;
        let pos = cel.position;
        let CelData::Image(buffer) = &mut cel.data else {
            return Err(CommandError::NotAnImageCel { layer, frame });
        };
        if buffer.color_mode != ColorMode::Rgba {
            return Err(CommandError::UnsupportedColorMode);
        }
        let fits = u64::from(self.x) + u64::from(self.width) <= u64::from(buffer.width)
            && u64::from(self.y) + u64::from(self.height) <= u64::from(buffer.height);
        if !fits {
            return Err(CommandError::PatchOutOfCel {
                x: self.x,
                y: self.y,
                width: self.width,
                height: self.height,
                cel_width: buffer.width,
                cel_height: buffer.height,
            });
        }
        let expected = self.width as usize * self.height as usize * 4;
        if self.pixels.len() != expected {
            return Err(CommandError::BufferLengthMismatch {
                expected,
                got: self.pixels.len(),
            });
        }
        self.swap(buffer);
        self.sprite_rect = Some(Rect::new(
            pos.0 + self.x as i32,
            pos.1 + self.y as i32,
            self.width,
            self.height,
        ));
        self.applied = true;
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        if !self.applied {
            return;
        }
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        if u64::from(self.x) + u64::from(self.width) <= u64::from(buffer.width)
            && u64::from(self.y) + u64::from(self.height) <= u64::from(buffer.height)
        {
            self.swap(buffer);
        }
        self.applied = false;
    }

    fn dirty_region(&self) -> DirtyRegion {
        match self.sprite_rect {
            Some(r) => DirtyRegion::layer_rect(self.layer, self.frame, r),
            None => DirtyRegion::None,
        }
    }
}

/// Insert a transparent RGBA image cel at `(layer, frame)` with the given
/// size and position when none exists there. Holds only the geometry; a
/// no-op when a cel already exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddEmptyCel {
    layer: LayerId,
    frame: FrameIndex,
    width: u32,
    height: u32,
    position: (i32, i32),
    inserted: bool,
}

impl AddEmptyCel {
    /// Seed a `width`×`height` transparent cel at `position` (sprite space).
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        (width, height): (u32, u32),
        position: (i32, i32),
    ) -> Self {
        Self {
            layer,
            frame,
            width,
            height,
            position,
            inserted: false,
        }
    }
}

impl Command for AddEmptyCel {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        if doc.layer(self.layer).is_none() {
            return Err(CommandError::UnknownLayer(self.layer.0));
        }
        self.inserted = cels.get(self.layer, self.frame).is_none();
        if self.inserted {
            let mut cel = Cel::image(
                self.layer,
                self.frame,
                PixelBuffer::empty(self.width, self.height, ColorMode::Rgba),
            );
            cel.position = self.position;
            cels.insert(cel);
        }
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        if std::mem::take(&mut self.inserted) {
            cels.remove(self.layer, self.frame);
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        DirtyRegion::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Layer;

    fn doc(w: u32, h: u32) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(w, h)
            .add_layer(Layer::image(LayerId::new(0), "L"))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            PixelBuffer::empty(w, h, ColorMode::Rgba),
        ));
        (sprite, cels)
    }

    fn data(cels: &CelMap) -> Vec<u8> {
        match &cels
            .get(LayerId::new(0), FrameIndex::new(0))
            .expect("cel")
            .data
        {
            CelData::Image(b) => b.data.clone(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn apply_swaps_rect_and_revert_restores() {
        let (mut s, mut c) = doc(4, 3);
        let before = data(&c);
        let mut cmd = PatchCel::new(
            LayerId::new(0),
            FrameIndex::new(0),
            (1, 1),
            (2, 1),
            vec![9; 8],
        );
        cmd.apply(&mut s, &mut c).expect("apply");
        let after = data(&c);
        assert_eq!(&after[(4 + 1) * 4..(4 + 3) * 4], &[9; 8]);
        assert_eq!(after.iter().filter(|&&b| b == 9).count(), 8);
        assert_eq!(cmd.stored_bytes(), 8);
        cmd.revert(&mut s, &mut c);
        assert_eq!(data(&c), before);
        cmd.apply(&mut s, &mut c).expect("redo");
        assert_eq!(data(&c), after);
    }

    #[test]
    fn apply_out_of_cel_or_wrong_length_returns_err() {
        let (mut s, mut c) = doc(4, 3);
        let mut cmd = PatchCel::new(
            LayerId::new(0),
            FrameIndex::new(0),
            (3, 0),
            (2, 1),
            vec![0; 8],
        );
        assert!(matches!(
            cmd.apply(&mut s, &mut c),
            Err(CommandError::PatchOutOfCel { .. })
        ));
        let mut cmd = PatchCel::new(
            LayerId::new(0),
            FrameIndex::new(0),
            (0, 0),
            (2, 1),
            vec![0; 4],
        );
        assert!(cmd.apply(&mut s, &mut c).is_err());
    }

    #[test]
    fn add_empty_cel_inserts_once_and_revert_removes() {
        let (mut s, mut c) = doc(2, 2);
        let mut cmd = AddEmptyCel::new(LayerId::new(0), FrameIndex::new(1), (2, 2), (0, 0));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert!(c.get(LayerId::new(0), FrameIndex::new(1)).is_some());
        cmd.revert(&mut s, &mut c);
        assert!(c.get(LayerId::new(0), FrameIndex::new(1)).is_none());
        // Existing cel: no-op both ways.
        let mut cmd = AddEmptyCel::new(LayerId::new(0), FrameIndex::new(0), (2, 2), (0, 0));
        cmd.apply(&mut s, &mut c).expect("apply");
        cmd.revert(&mut s, &mut c);
        assert!(c.get(LayerId::new(0), FrameIndex::new(0)).is_some());
    }
}
