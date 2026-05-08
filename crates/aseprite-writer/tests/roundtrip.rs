//! Round-trip tests: build an [`AseFile`] in memory, write it with
//! `aseprite-writer`, then parse the bytes with `aseprite-loader` and
//! assert the structural fields that survive the trip.
//!
//! The format reader and writer are independent, so a passing test
//! means both sides agree on the on-disk layout.

use aseprite_loader::binary::blend_mode::BlendMode as LoaderBlendMode;
use aseprite_loader::binary::chunk::Chunk;
use aseprite_loader::binary::chunks::layer::LayerType as LoaderLayerType;
use aseprite_loader::binary::chunks::tags::AnimationDirection as LoaderDirection;
use aseprite_loader::binary::color_depth::ColorDepth as LoaderColorDepth;
use aseprite_loader::binary::file::parse_file;
use aseprite_loader::binary::raw_file::parse_raw_file;
use aseprite_writer::{
    AnimationDirection, AseFile, BlendMode, Color, ColorDepth, Frame, Header, LayerChunk,
    LayerFlags, LayerType, PaletteChunk, PaletteEntry, Tag, write,
};

fn write_to_vec(file: &AseFile) -> Vec<u8> {
    let mut buf = Vec::new();
    write(file, &mut buf).expect("write succeeds");
    buf
}

#[test]
fn empty_rgba_sprite_roundtrips_through_loader() {
    let file = AseFile {
        header: Header::new(16, 16, ColorDepth::Rgba),
        layers: Vec::new(),
        palette: None,
        tags: Vec::new(),
        frames: vec![Frame::new(100)],
    };
    let bytes = write_to_vec(&file);

    let parsed = parse_file(&bytes).expect("loader parses our output");
    assert_eq!(parsed.header.width, 16);
    assert_eq!(parsed.header.height, 16);
    assert_eq!(parsed.header.color_depth, LoaderColorDepth::Rgba);
    assert_eq!(parsed.header.frames, 1);
    assert_eq!(parsed.frames.len(), 1);
    assert_eq!(parsed.frames[0].duration, 100);
    assert_eq!(parsed.layers.len(), 0);
    assert_eq!(parsed.tags.len(), 0);
}

#[test]
fn three_layers_with_blend_modes_roundtrip() {
    let layers = vec![
        LayerChunk {
            flags: LayerFlags::VISIBLE | LayerFlags::EDITABLE,
            layer_type: LayerType::Normal,
            child_level: 0,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            name: "Background".into(),
            tileset_index: None,
        },
        LayerChunk {
            flags: LayerFlags::VISIBLE,
            layer_type: LayerType::Normal,
            child_level: 0,
            blend_mode: BlendMode::Multiply,
            opacity: 200,
            name: "Shadow".into(),
            tileset_index: None,
        },
        LayerChunk {
            flags: LayerFlags::VISIBLE,
            layer_type: LayerType::Normal,
            child_level: 0,
            blend_mode: BlendMode::Divide,
            opacity: 128,
            name: "Highlight".into(),
            tileset_index: None,
        },
    ];

    let file = AseFile {
        header: Header::new(32, 32, ColorDepth::Rgba),
        layers,
        palette: None,
        tags: Vec::new(),
        frames: vec![Frame::new(80)],
    };

    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("loader parses our output");

    assert_eq!(parsed.layers.len(), 3);
    assert_eq!(parsed.layers[0].name, "Background");
    assert_eq!(parsed.layers[0].layer_type, LoaderLayerType::Normal);
    assert_eq!(parsed.layers[0].blend_mode, LoaderBlendMode::Normal);
    assert_eq!(parsed.layers[0].opacity, 255);

    assert_eq!(parsed.layers[1].name, "Shadow");
    assert_eq!(parsed.layers[1].blend_mode, LoaderBlendMode::Multiply);
    assert_eq!(parsed.layers[1].opacity, 200);

    assert_eq!(parsed.layers[2].name, "Highlight");
    assert_eq!(parsed.layers[2].blend_mode, LoaderBlendMode::Divide);
    assert_eq!(parsed.layers[2].opacity, 128);
}

#[test]
fn palette_roundtrips_via_raw_file() {
    let palette = PaletteChunk {
        first_color: 0,
        entries: vec![
            PaletteEntry {
                color: Color::rgba(0, 0, 0, 0),
                name: None,
            },
            PaletteEntry {
                color: Color::rgba(172, 50, 50, 255),
                name: Some("Red".into()),
            },
            PaletteEntry {
                color: Color::rgba(106, 190, 48, 255),
                name: Some("Green".into()),
            },
            PaletteEntry {
                color: Color::rgba(91, 110, 225, 255),
                name: None,
            },
        ],
    };

    let file = AseFile {
        header: Header::new(8, 8, ColorDepth::Rgba),
        layers: Vec::new(),
        palette: Some(palette),
        tags: Vec::new(),
        frames: vec![Frame::new(100)],
    };

    let bytes = write_to_vec(&file);

    // The high-level `parse_file` only attaches a palette for indexed
    // sprites, so dive into the raw chunks for RGBA palette assertions.
    let raw = parse_raw_file(&bytes).expect("raw parse");
    let palette_chunk = raw.frames[0]
        .chunks
        .iter()
        .find_map(|c| match c {
            Chunk::Palette(p) => Some(p),
            _ => None,
        })
        .expect("palette chunk emitted in frame 0");

    assert_eq!(*palette_chunk.indices.start(), 0);
    assert_eq!(*palette_chunk.indices.end(), 3);
    assert_eq!(palette_chunk.entries.len(), 4);
    assert_eq!(palette_chunk.entries[0].color.alpha, 0);
    assert_eq!(palette_chunk.entries[1].color.red, 172);
    assert_eq!(palette_chunk.entries[1].name, Some("Red"));
    assert_eq!(palette_chunk.entries[2].name, Some("Green"));
    assert_eq!(palette_chunk.entries[3].name, None);
}

#[test]
fn tags_roundtrip_with_directions() {
    let tags = vec![
        Tag {
            from_frame: 0,
            to_frame: 2,
            direction: AnimationDirection::Forward,
            repeat: 0,
            color: [0xFF, 0x00, 0x00],
            name: "walk".into(),
        },
        Tag {
            from_frame: 3,
            to_frame: 5,
            direction: AnimationDirection::PingPong,
            repeat: 3,
            color: [0x00, 0xFF, 0x00],
            name: "swing".into(),
        },
        Tag {
            from_frame: 6,
            to_frame: 6,
            direction: AnimationDirection::Reverse,
            repeat: 1,
            color: [0x00, 0x00, 0xFF],
            name: "blink".into(),
        },
    ];

    let file = AseFile {
        header: Header::new(8, 8, ColorDepth::Rgba),
        layers: Vec::new(),
        palette: None,
        tags,
        frames: (0..7).map(|_| Frame::new(50)).collect(),
    };

    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("loader parses our output");

    assert_eq!(parsed.tags.len(), 3);
    assert_eq!(parsed.tags[0].name, "walk");
    assert_eq!(parsed.tags[0].frames, 0..=2);
    assert!(matches!(
        parsed.tags[0].animation_direction,
        LoaderDirection::Forward
    ));

    assert_eq!(parsed.tags[1].name, "swing");
    assert_eq!(parsed.tags[1].frames, 3..=5);
    assert!(matches!(
        parsed.tags[1].animation_direction,
        LoaderDirection::PingPong
    ));
    assert_eq!(parsed.tags[1].animation_repeat, 3);

    assert_eq!(parsed.tags[2].name, "blink");
    assert_eq!(parsed.tags[2].frames, 6..=6);
    assert!(matches!(
        parsed.tags[2].animation_direction,
        LoaderDirection::Reverse
    ));
}

#[test]
fn full_file_with_layers_palette_tags_roundtrips() {
    let file = AseFile {
        header: Header::new(64, 64, ColorDepth::Rgba),
        layers: vec![LayerChunk {
            flags: LayerFlags::VISIBLE | LayerFlags::EDITABLE,
            layer_type: LayerType::Normal,
            child_level: 0,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            name: "main".into(),
            tileset_index: None,
        }],
        palette: Some(PaletteChunk {
            first_color: 0,
            entries: vec![
                PaletteEntry {
                    color: Color::rgba(0, 0, 0, 0),
                    name: None,
                },
                PaletteEntry {
                    color: Color::rgba(255, 255, 255, 255),
                    name: Some("White".into()),
                },
            ],
        }),
        tags: vec![Tag {
            from_frame: 0,
            to_frame: 1,
            direction: AnimationDirection::Forward,
            repeat: 0,
            color: [0, 0, 0],
            name: "all".into(),
        }],
        frames: vec![Frame::new(100), Frame::new(150)],
    };

    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("loader parses our output");
    assert_eq!(parsed.header.width, 64);
    assert_eq!(parsed.header.frames, 2);
    assert_eq!(parsed.layers.len(), 1);
    assert_eq!(parsed.layers[0].name, "main");
    assert_eq!(parsed.tags.len(), 1);
    assert_eq!(parsed.frames.len(), 2);
    assert_eq!(parsed.frames[0].duration, 100);
    assert_eq!(parsed.frames[1].duration, 150);

    // file_size in the header should match buffer length exactly.
    let raw = parse_raw_file(&bytes).expect("raw parse");
    assert_eq!(raw.header.file_size as usize, bytes.len());
}
