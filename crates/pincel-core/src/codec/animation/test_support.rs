//! Sprite fixtures shared by the animated-export tests.

use crate::document::{
    Cel, CelMap, ColorMode, Frame, FrameIndex, Layer, LayerId, PixelBuffer, Sprite, Tag,
};
use crate::render::{ComposeRequest, compose};

/// A `w × h` sprite with one image layer and one frame per entry of
/// `frames`, each `(duration_ms, rgba pixels)`; `pixels` is a tight RGBA8
/// buffer of the full canvas.
pub(super) fn sprite_from(
    w: u32,
    h: u32,
    frames: &[(u16, Vec<u8>)],
    tags: Vec<Tag>,
) -> (Sprite, CelMap) {
    let mut builder = Sprite::builder(w, h).add_layer(Layer::image(LayerId::new(0), "art"));
    for (duration, _) in frames {
        builder = builder.add_frame(Frame::new(*duration));
    }
    for tag in tags {
        builder = builder.add_tag(tag);
    }
    let sprite = builder.build().expect("sprite builds");
    let mut cels = CelMap::new();
    for (i, (_, pixels)) in frames.iter().enumerate() {
        let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
        buf.data.copy_from_slice(pixels);
        let index = u32::try_from(i).expect("small fixture");
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(index), buf));
    }
    (sprite, cels)
}

/// Three distinguishable 2×2 frames: frame `i` has an opaque pixel of
/// shade `40 * (i + 1)` at a different position, the rest transparent.
/// Durations 100, 50 and 230 ms.
pub(super) fn three_frames(tags: Vec<Tag>) -> (Sprite, CelMap) {
    let frame = |i: usize| {
        let mut px = vec![0u8; 2 * 2 * 4];
        let shade = 40 * (u8::try_from(i).expect("small") + 1);
        px[i * 4..i * 4 + 4].copy_from_slice(&[shade, 10, 200, 255]);
        px
    };
    sprite_from(
        2,
        2,
        &[(100, frame(0)), (50, frame(1)), (230, frame(2))],
        tags,
    )
}

/// The pixels `compose()` produces for `frame` at zoom 1.
pub(super) fn composed(sprite: &Sprite, cels: &CelMap, frame: u32) -> Vec<u8> {
    let mut pixels = Vec::new();
    compose(
        sprite,
        cels,
        &ComposeRequest::full(FrameIndex::new(frame), sprite.width, sprite.height),
        &mut pixels,
    )
    .expect("compose succeeds");
    pixels
}

/// Nearest-neighbour reference upscale, written independently of the
/// production one.
pub(super) fn upscaled(src: &[u8], w: u32, scale: u32) -> Vec<u8> {
    let h = src.len() as u32 / 4 / w;
    let mut out = Vec::new();
    for y in 0..h * scale {
        for x in 0..w * scale {
            let i = (((y / scale) * w + x / scale) * 4) as usize;
            out.extend_from_slice(&src[i..i + 4]);
        }
    }
    out
}
