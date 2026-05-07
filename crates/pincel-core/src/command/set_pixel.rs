//! `SetPixel` command — write a single RGBA pixel into an image cel.
//!
//! Pixel coordinates are sprite-global; the command translates them into the
//! cel's local buffer space via the cel's `position`. Indexed and grayscale
//! cels are rejected for now (Phase 1 RGBA-first; see `docs/specs/pincel.md`
//! §4.1).

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};

use super::Command;
use super::error::CommandError;

/// Write a single pixel into an image cel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPixel {
    layer: LayerId,
    frame: FrameIndex,
    sprite_x: i32,
    sprite_y: i32,
    new_color: Rgba,
    /// `Some` after a successful `apply`; carries the prior pixel value used
    /// by `revert`.
    previous: Option<Rgba>,
}

impl SetPixel {
    /// Build a new `SetPixel` targeting `(sprite_x, sprite_y)` on the cel at
    /// `(layer, frame)`.
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        sprite_x: i32,
        sprite_y: i32,
        color: Rgba,
    ) -> Self {
        Self {
            layer,
            frame,
            sprite_x,
            sprite_y,
            new_color: color,
            previous: None,
        }
    }
}

impl Command for SetPixel {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let cel = cels
            .get_mut(self.layer, self.frame)
            .ok_or(CommandError::MissingCel {
                layer: self.layer,
                frame: self.frame,
            })?;

        let CelData::Image(buffer) = &mut cel.data else {
            return Err(CommandError::NotAnImageCel {
                layer: self.layer,
                frame: self.frame,
            });
        };

        if !matches!(buffer.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }

        let local = local_coords(self.sprite_x, self.sprite_y, cel.position).ok_or(
            CommandError::PixelOutOfBounds {
                x: self.sprite_x,
                y: self.sprite_y,
                width: buffer.width,
                height: buffer.height,
                position: cel.position,
            },
        )?;
        if local.0 >= buffer.width || local.1 >= buffer.height {
            return Err(CommandError::PixelOutOfBounds {
                x: self.sprite_x,
                y: self.sprite_y,
                width: buffer.width,
                height: buffer.height,
                position: cel.position,
            });
        }

        let offset = ((local.1 * buffer.width + local.0) * 4) as usize;
        let prior = Rgba {
            r: buffer.data[offset],
            g: buffer.data[offset + 1],
            b: buffer.data[offset + 2],
            a: buffer.data[offset + 3],
        };
        buffer.data[offset] = self.new_color.r;
        buffer.data[offset + 1] = self.new_color.g;
        buffer.data[offset + 2] = self.new_color.b;
        buffer.data[offset + 3] = self.new_color.a;
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
        let Some(local) = local_coords(self.sprite_x, self.sprite_y, cel.position) else {
            return;
        };
        if local.0 >= buffer.width || local.1 >= buffer.height {
            return;
        }
        let offset = ((local.1 * buffer.width + local.0) * 4) as usize;
        buffer.data[offset] = prior.r;
        buffer.data[offset + 1] = prior.g;
        buffer.data[offset + 2] = prior.b;
        buffer.data[offset + 3] = prior.a;
    }
}

fn local_coords(sprite_x: i32, sprite_y: i32, position: (i32, i32)) -> Option<(u32, u32)> {
    let lx = sprite_x.checked_sub(position.0)?;
    let ly = sprite_y.checked_sub(position.1)?;
    if lx < 0 || ly < 0 {
        return None;
    }
    Some((lx as u32, ly as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, ColorMode, PixelBuffer};

    fn fixture() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(8, 8)
            .add_layer(crate::document::Layer::image(LayerId::new(1), "bg"))
            .add_frame(crate::document::Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBuffer::empty(8, 8, ColorMode::Rgba),
        ));
        (sprite, cels)
    }

    fn pixel(cels: &CelMap, x: u32, y: u32) -> Rgba {
        let cel = cels
            .get(LayerId::new(1), FrameIndex::new(0))
            .expect("cel exists");
        let CelData::Image(buf) = &cel.data else {
            panic!("expected image cel");
        };
        let off = ((y * buf.width + x) * 4) as usize;
        Rgba {
            r: buf.data[off],
            g: buf.data[off + 1],
            b: buf.data[off + 2],
            a: buf.data[off + 3],
        }
    }

    #[test]
    fn apply_writes_pixel_and_records_previous() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetPixel::new(LayerId::new(1), FrameIndex::new(0), 2, 3, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply succeeds");
        assert_eq!(pixel(&cels, 2, 3), Rgba::WHITE);
        assert_eq!(cmd.previous, Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn revert_restores_previous_pixel() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetPixel::new(LayerId::new(1), FrameIndex::new(0), 4, 4, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply succeeds");
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(pixel(&cels, 4, 4), Rgba::TRANSPARENT);
    }

    #[test]
    fn apply_then_revert_then_apply_round_trip() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetPixel::new(LayerId::new(1), FrameIndex::new(0), 0, 0, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply 1");
        cmd.revert(&mut sprite, &mut cels);
        cmd.apply(&mut sprite, &mut cels).expect("apply 2");
        assert_eq!(pixel(&cels, 0, 0), Rgba::WHITE);
    }

    #[test]
    fn missing_cel_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetPixel::new(LayerId::new(99), FrameIndex::new(0), 0, 0, Rgba::WHITE);
        let err = cmd.apply(&mut sprite, &mut cels).unwrap_err();
        assert_eq!(
            err,
            CommandError::MissingCel {
                layer: LayerId::new(99),
                frame: FrameIndex::new(0),
            }
        );
    }

    #[test]
    fn out_of_bounds_pixel_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetPixel::new(LayerId::new(1), FrameIndex::new(0), 100, 100, Rgba::WHITE);
        let err = cmd.apply(&mut sprite, &mut cels).unwrap_err();
        assert!(matches!(err, CommandError::PixelOutOfBounds { .. }));
    }

    #[test]
    fn coords_outside_cel_position_are_rejected() {
        let (mut sprite, mut cels) = fixture();
        // Place cel offset to (4, 4); painting (3, 3) on sprite is outside the cel.
        cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            .unwrap()
            .position = (4, 4);
        let mut cmd = SetPixel::new(LayerId::new(1), FrameIndex::new(0), 3, 3, Rgba::WHITE);
        let err = cmd.apply(&mut sprite, &mut cels).unwrap_err();
        assert!(matches!(err, CommandError::PixelOutOfBounds { .. }));
    }

    #[test]
    fn does_not_merge_with_other_set_pixel() {
        let mut a = SetPixel::new(LayerId::new(1), FrameIndex::new(0), 0, 0, Rgba::WHITE);
        let b = SetPixel::new(LayerId::new(1), FrameIndex::new(0), 1, 0, Rgba::WHITE);
        assert!(!a.merge(&b));
    }
}
