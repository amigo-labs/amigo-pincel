//! Fixture builders shared by the `render` module's unit tests.

use crate::document::{
    BlendMode, ColorMode, Frame, FrameIndex, Layer, LayerId, PixelBuffer, Sprite,
};

use super::request::ComposeRequest;

pub(super) fn solid(w: u32, h: u32, rgba: [u8; 4]) -> PixelBuffer {
    let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
    for px in buf.data.chunks_exact_mut(4) {
        px.copy_from_slice(&rgba);
    }
    buf
}

pub(super) fn one_layer_sprite(w: u32, h: u32, frames: u32) -> Sprite {
    let mut b = Sprite::builder(w, h).add_layer(Layer::image(LayerId::new(0), "bg"));
    for _ in 0..frames {
        b = b.add_frame(Frame::default());
    }
    b.build().expect("test sprite")
}

pub(super) fn full_req(w: u32, h: u32) -> ComposeRequest {
    ComposeRequest::full(FrameIndex::new(0), w, h)
}

/// Two stacked image layers, one frame. Layer `0` is the backdrop and blends
/// `Normal`; layer `1` sits on top and carries `top_mode`.
pub(super) fn stacked_sprite(w: u32, h: u32, top_mode: BlendMode) -> Sprite {
    let mut top = Layer::image(LayerId::new(1), "top");
    top.blend_mode = top_mode;
    Sprite::builder(w, h)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_layer(top)
        .add_frame(Frame::default())
        .build()
        .expect("test sprite")
}
