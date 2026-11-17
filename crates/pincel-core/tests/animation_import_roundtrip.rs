//! An animation exported by `export_animation` and read back by
//! `import_animation` (docs/specs/animated-image-import.md AC-11).

use pincel_core::{
    AnimationFormat, AnimationOptions, Cel, CelMap, ColorMode, ComposeRequest, Frame, FrameIndex,
    Layer, LayerId, PixelBuffer, Sprite, compose, export_animation, import_animation,
};

/// A 3×2 sprite with one frame per entry of `frames`, `(duration, pixels)`.
fn sprite(frames: &[(u16, [[u8; 4]; 6])]) -> (Sprite, CelMap) {
    let mut builder = Sprite::builder(3, 2).add_layer(Layer::image(LayerId::new(0), "art"));
    for (duration, _) in frames {
        builder = builder.add_frame(Frame::new(*duration));
    }
    let sprite = builder.build().expect("sprite builds");
    let mut cels = CelMap::new();
    for (i, (_, px)) in frames.iter().enumerate() {
        let mut buf = PixelBuffer::empty(3, 2, ColorMode::Rgba);
        buf.data.copy_from_slice(&px.concat());
        let index = u32::try_from(i).expect("few frames");
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(index), buf));
    }
    (sprite, cels)
}

fn composed(sprite: &Sprite, cels: &CelMap, frame: u32) -> Vec<u8> {
    let mut out = Vec::new();
    compose(
        sprite,
        cels,
        &ComposeRequest::full(FrameIndex::new(frame), sprite.width, sprite.height),
        &mut out,
    )
    .expect("compose");
    out
}

const R: [u8; 4] = [255, 0, 0, 255];
const G: [u8; 4] = [0, 200, 0, 255];
const B: [u8; 4] = [0, 0, 255, 255];
const T: [u8; 4] = [0, 0, 0, 0];
const HALF: [u8; 4] = [10, 20, 30, 128];

#[test]
fn import_animation_reads_back_an_apng_export_exactly() {
    let (sprite, cels) = sprite(&[
        (100, [R, G, B, T, HALF, R]),
        (1, [T, T, T, T, T, T]),
        (230, [HALF, HALF, G, G, B, B]),
        (65535, [B, T, R, T, G, T]),
    ]);
    let bytes = export_animation(
        &sprite,
        &cels,
        &AnimationOptions::new(AnimationFormat::Apng),
    )
    .expect("export");
    let anim = import_animation(&bytes).expect("import");
    assert_eq!((anim.width, anim.height), (3, 2));
    assert_eq!(anim.frames.len(), 4);
    for (i, frame) in anim.frames.iter().enumerate() {
        let index = u32::try_from(i).expect("few");
        assert_eq!(
            frame.rgba,
            composed(&sprite, &cels, index),
            "frame {i} pixels"
        );
        assert_eq!(
            frame.duration_ms, sprite.frames[i].duration_ms,
            "frame {i} duration"
        );
    }
}

#[test]
fn import_animation_reads_back_a_gif_export_where_it_was_exact() {
    let (sprite, cels) = sprite(&[
        (20, [R, G, B, T, R, R]),
        (70, [T, T, T, T, T, B]),
        (230, [G, G, G, T, B, B]),
    ]);
    let bytes = export_animation(&sprite, &cels, &AnimationOptions::new(AnimationFormat::Gif))
        .expect("export");
    let anim = import_animation(&bytes).expect("import");
    assert_eq!(anim.frames.len(), 3);
    for (i, frame) in anim.frames.iter().enumerate() {
        let index = u32::try_from(i).expect("few");
        assert_eq!(
            frame.rgba,
            composed(&sprite, &cels, index),
            "frame {i} pixels"
        );
        assert_eq!(
            frame.duration_ms, sprite.frames[i].duration_ms,
            "frame {i} duration"
        );
    }
}
