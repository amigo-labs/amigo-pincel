//! Generation info on Pixel layers (docs/specs/comfyui-generation-info.md):
//! kept in `.aseprite`, through undo / redo, Duplicate, Merge down and
//! Flatten.

use pincel_core::generate::add_generated_layer;
use pincel_core::{
    Bus, Cel, CelMap, ColorMode, DuplicateLayer, FlattenImage, Frame, FrameIndex, GenerationInfo,
    HistoryKind, Layer, LayerId, MergeDown, PixelBuffer, Rect, Sprite, UserData, read_aseprite,
    write_aseprite,
};

const F0: FrameIndex = FrameIndex::new(0);

fn info(prompt: &str) -> GenerationInfo {
    GenerationInfo {
        prompt: prompt.into(),
        negative: "blurry, text".into(),
        model: "sdxl.safetensors".into(),
        lora: "pixel-art.safetensors".into(),
        lora_strength: 0.75,
        strength: 100.0,
        seed: 42,
    }
}

fn doc() -> (Sprite, CelMap) {
    let sprite = Sprite::builder(4, 4)
        .add_layer(Layer::image(LayerId::new(0), "Background"))
        .add_frame(Frame::new(100))
        .build()
        .unwrap();
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        F0,
        PixelBuffer::empty(4, 4, ColorMode::Rgba),
    ));
    (sprite, cels)
}

fn round_trip(sprite: &Sprite, cels: &CelMap) -> Sprite {
    let mut bytes = Vec::new();
    write_aseprite(sprite, cels, &mut bytes).expect("writer succeeds");
    read_aseprite(&bytes)
        .expect("reader accepts our own output")
        .sprite
}

/// Adds a generated layer with id 7 and `info` as one undo step.
fn generate(bus: &mut Bus, s: &mut Sprite, c: &mut CelMap, info: &GenerationInfo) {
    let pixels = PixelBuffer {
        width: 1,
        height: 1,
        color_mode: ColorMode::Rgba,
        data: vec![255, 0, 0, 255],
    };
    let layer = Layer::image(LayerId::new(7), "Generated: slime").with_generation(info);
    let cmd = add_generated_layer(
        s,
        layer,
        Some(LayerId::new(0)),
        F0,
        Rect::new(1, 1, 1, 1),
        &pixels,
    );
    bus.execute_as(HistoryKind::Generate, cmd.into(), s, c)
        .expect("apply");
}

fn layer(s: &Sprite, id: u32) -> &Layer {
    s.layers.iter().find(|l| l.id.0 == id).expect("layer")
}

#[test]
fn generated_layer_keeps_its_generation_info_through_undo_redo_and_aseprite() {
    let (mut s, mut c) = doc();
    let mut bus = Bus::new();
    let info = info("a small green slime\nwith a hat");
    generate(&mut bus, &mut s, &mut c, &info);
    assert_eq!(layer(&s, 7).generation(), Some(info.clone()));
    assert_eq!(layer(&s, 0).generation(), None);
    assert!(bus.undo(&mut s, &mut c));
    assert!(bus.redo(&mut s, &mut c).unwrap());
    assert_eq!(layer(&s, 7).generation(), Some(info.clone()));
    // AC-10: the file keeps every value, as the text form in user data.
    // The reader numbers layers by position: the generated one is second.
    let back = round_trip(&s, &c);
    assert_eq!(back.layers[1].generation(), Some(info.clone()));
    assert_eq!(
        back.layers[1]
            .user_data
            .as_ref()
            .and_then(|u| u.text.clone()),
        Some(info.to_text())
    );
}

#[test]
fn foreign_user_data_text_is_no_generation_info_and_survives_saving() {
    let (mut s, c) = doc();
    let mut note = info("x").to_text();
    note.push_str("\nmy own note");
    s.layers[0].user_data = Some(UserData {
        text: Some(note.clone()),
        ..UserData::default()
    });
    assert_eq!(s.layers[0].generation(), None);
    let back = round_trip(&s, &c);
    assert_eq!(back.layers[0].generation(), None);
    assert_eq!(
        back.layers[0]
            .user_data
            .as_ref()
            .and_then(|u| u.text.clone()),
        Some(note)
    );
}

#[test]
fn duplicate_copies_merge_down_keeps_the_lower_and_flatten_drops_generation_info() {
    let (mut s, mut c) = doc();
    let mut bus = Bus::new();
    let lower = info("lower");
    s.layers[0] = s.layers[0].clone().with_generation(&lower);
    generate(&mut bus, &mut s, &mut c, &info("upper"));

    bus.execute(DuplicateLayer::new(LayerId::new(7)).into(), &mut s, &mut c)
        .unwrap();
    let copy = s.layers.last().expect("copy");
    assert_eq!(copy.generation(), Some(info("upper")));
    assert!(bus.undo(&mut s, &mut c));

    bus.execute(MergeDown::new(LayerId::new(7)).into(), &mut s, &mut c)
        .unwrap();
    assert_eq!(s.layers.len(), 1);
    assert_eq!(s.layers[0].generation(), Some(lower.clone()));
    assert!(bus.undo(&mut s, &mut c));
    assert_eq!(layer(&s, 7).generation(), Some(info("upper")));

    bus.execute(FlattenImage::new().into(), &mut s, &mut c)
        .unwrap();
    assert_eq!(s.layers.len(), 1);
    assert_eq!(s.layers[0].generation(), None);
    assert!(bus.undo(&mut s, &mut c));
    assert_eq!(layer(&s, 0).generation(), Some(lower));
    assert_eq!(layer(&s, 7).generation(), Some(info("upper")));
}
