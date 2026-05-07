//! Integration tests for [`pincel_core::compose`]. See `docs/specs/pincel.md` §4.

use pincel_core::{
    Cel, CelMap, ColorMode, ComposeRequest, Frame, FrameIndex, Layer, LayerId, PixelBuffer, Rect,
    Sprite, compose,
};

fn solid(w: u32, h: u32, rgba: [u8; 4]) -> PixelBuffer {
    let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
    for px in buf.data.chunks_exact_mut(4) {
        px.copy_from_slice(&rgba);
    }
    buf
}

#[test]
fn end_to_end_two_layer_sprite_composes_to_expected_pixels() {
    // 4×4 sprite. Bottom layer: solid red. Top layer: 2×2 white square at (1,1).
    let sprite = Sprite::builder(4, 4)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_layer(Layer::image(LayerId::new(1), "fg"))
        .add_frame(Frame::new(100))
        .build()
        .expect("sprite builds");

    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        solid(4, 4, [255, 0, 0, 255]),
    ));
    let mut top = Cel::image(
        LayerId::new(1),
        FrameIndex::new(0),
        solid(2, 2, [255, 255, 255, 255]),
    );
    top.position = (1, 1);
    cels.insert(top);

    let result = compose(
        &sprite,
        &cels,
        &ComposeRequest::full(FrameIndex::new(0), 4, 4),
    )
    .expect("compose succeeds");

    assert_eq!((result.width, result.height), (4, 4));
    assert_eq!(result.pixels.len(), 4 * 4 * 4);

    // Build the expected 4×4 RGBA buffer.
    let mut expected = vec![0u8; 4 * 4 * 4];
    for y in 0..4 {
        for x in 0..4 {
            let idx = (y * 4 + x) * 4;
            let inside_white = (1..3).contains(&x) && (1..3).contains(&y);
            let rgba = if inside_white {
                [255, 255, 255, 255]
            } else {
                [255, 0, 0, 255]
            };
            expected[idx..idx + 4].copy_from_slice(&rgba);
        }
    }
    assert_eq!(result.pixels, expected);
}

#[test]
fn viewport_subregion_with_zoom_returns_zoomed_subregion() {
    let sprite = Sprite::builder(4, 4)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_frame(Frame::new(100))
        .build()
        .unwrap();

    // Place a recognizable 2x2 in the lower-right corner.
    let mut buf = PixelBuffer::empty(4, 4, ColorMode::Rgba);
    for y in 0..4u32 {
        for x in 0..4u32 {
            let idx = ((y * 4 + x) * 4) as usize;
            let v = if x >= 2 && y >= 2 { 200 } else { 0 };
            buf.data[idx..idx + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    let mut cels = CelMap::new();
    cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));

    let req = ComposeRequest {
        viewport: Rect::new(2, 2, 2, 2),
        zoom: 4,
        ..ComposeRequest::full(FrameIndex::new(0), 4, 4)
    };
    let result = compose(&sprite, &cels, &req).expect("compose succeeds");

    assert_eq!((result.width, result.height), (8, 8));
    // All pixels in the zoomed sub-region should match the source value 200.
    for px in result.pixels.chunks_exact(4) {
        assert_eq!(px, &[200, 200, 200, 255]);
    }
}

#[test]
fn frame_navigation_picks_per_frame_cel_data() {
    let sprite = Sprite::builder(1, 1)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_frame(Frame::new(100))
        .add_frame(Frame::new(100))
        .build()
        .unwrap();

    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        solid(1, 1, [10, 0, 0, 255]),
    ));
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(1),
        solid(1, 1, [0, 20, 0, 255]),
    ));

    let r0 = compose(
        &sprite,
        &cels,
        &ComposeRequest::full(FrameIndex::new(0), 1, 1),
    )
    .unwrap();
    let r1 = compose(
        &sprite,
        &cels,
        &ComposeRequest::full(FrameIndex::new(1), 1, 1),
    )
    .unwrap();

    assert_eq!(r0.pixels, vec![10, 0, 0, 255]);
    assert_eq!(r1.pixels, vec![0, 20, 0, 255]);
}
