//! Round-trip tests: build an [`AseFile`] in memory, write it with
//! `aseprite-writer`, then parse the bytes with `aseprite-loader` and
//! assert the structural fields that survive the trip.
//!
//! The format reader and writer are independent, so a passing test
//! means both sides agree on the on-disk layout.

use aseprite_loader::binary::blend_mode::BlendMode as LoaderBlendMode;
use aseprite_loader::binary::chunk::Chunk;
use aseprite_loader::binary::chunks::cel::CelContent as LoaderCelContent;
use aseprite_loader::binary::chunks::layer::LayerType as LoaderLayerType;
use aseprite_loader::binary::chunks::tags::AnimationDirection as LoaderDirection;
use aseprite_loader::binary::color_depth::ColorDepth as LoaderColorDepth;
use aseprite_loader::binary::file::parse_file;
use aseprite_loader::binary::raw_file::parse_raw_file;
use aseprite_loader::loader::decompress;
use aseprite_writer::{
    AnimationDirection, AseFile, BlendMode, CelChunk, CelContent, Color, ColorDepth, Frame, Header,
    LayerChunk, LayerFlags, LayerType, PaletteChunk, PaletteEntry, Tag, TilesetChunk, write,
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
        tilesets: Vec::new(),
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
        tilesets: Vec::new(),
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
        tilesets: Vec::new(),
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
        tilesets: Vec::new(),
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

fn rgba_layer(name: &str) -> LayerChunk {
    LayerChunk {
        flags: LayerFlags::VISIBLE,
        layer_type: LayerType::Normal,
        child_level: 0,
        blend_mode: BlendMode::Normal,
        opacity: 255,
        name: name.into(),
        tileset_index: None,
    }
}

fn checker_rgba(width: u16, height: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(usize::from(width) * usize::from(height) * 4);
    for y in 0..height {
        for x in 0..width {
            let on = (x ^ y) & 1 == 0;
            let (r, g, b, a) = if on {
                (255, 0, 128, 255)
            } else {
                (0, 64, 200, 200)
            };
            buf.extend_from_slice(&[r, g, b, a]);
        }
    }
    buf
}

#[test]
fn single_image_cel_roundtrips_with_pixels() {
    let pixels = checker_rgba(4, 4);
    let file = AseFile {
        header: Header::new(8, 8, ColorDepth::Rgba),
        layers: vec![rgba_layer("L0")],
        palette: None,
        tags: Vec::new(),
        tilesets: Vec::new(),
        frames: vec![Frame {
            duration: 100,
            cels: vec![CelChunk {
                layer_index: 0,
                x: 2,
                y: 1,
                opacity: 200,
                z_index: 0,
                content: CelContent::Image {
                    width: 4,
                    height: 4,
                    data: pixels.clone(),
                },
            }],
        }],
    };
    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("loader parses our output");

    assert_eq!(parsed.frames.len(), 1);
    assert_eq!(parsed.layers.len(), 1);
    let cel = parsed.frames[0].cels[0]
        .as_ref()
        .expect("cel slot is populated for layer 0");
    assert_eq!(cel.x, 2);
    assert_eq!(cel.y, 1);
    assert_eq!(cel.opacity, 200);
    let image = match &cel.content {
        LoaderCelContent::Image(img) => img,
        other => panic!("expected Image cel content, got {other:?}"),
    };
    assert_eq!(image.width, 4);
    assert_eq!(image.height, 4);
    assert!(image.compressed);

    let mut decoded = vec![0u8; pixels.len()];
    decompress(image.data, &mut decoded).expect("zlib decompresses");
    assert_eq!(decoded, pixels);
}

#[test]
fn linked_cel_roundtrips_pointing_at_source_frame() {
    let pixels = checker_rgba(2, 2);
    let file = AseFile {
        header: Header::new(4, 4, ColorDepth::Rgba),
        layers: vec![rgba_layer("L0")],
        palette: None,
        tags: Vec::new(),
        tilesets: Vec::new(),
        frames: vec![
            Frame {
                duration: 100,
                cels: vec![CelChunk {
                    layer_index: 0,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    z_index: 0,
                    content: CelContent::Image {
                        width: 2,
                        height: 2,
                        data: pixels.clone(),
                    },
                }],
            },
            Frame {
                duration: 100,
                cels: vec![CelChunk {
                    layer_index: 0,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    z_index: 0,
                    content: CelContent::Linked { frame_position: 0 },
                }],
            },
        ],
    };
    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("loader parses our output");

    assert_eq!(parsed.frames.len(), 2);
    let f0 = parsed.frames[0].cels[0].as_ref().unwrap();
    assert!(matches!(&f0.content, LoaderCelContent::Image(_)));
    let f1 = parsed.frames[1].cels[0].as_ref().unwrap();
    match &f1.content {
        LoaderCelContent::LinkedCel { frame_position } => assert_eq!(*frame_position, 0),
        other => panic!("expected LinkedCel content, got {other:?}"),
    }
}

#[test]
fn multi_cel_across_layers_and_frames_roundtrips() {
    let red = vec![255, 0, 0, 255]; // 1x1 RGBA
    let green = vec![0, 255, 0, 255];
    let blue = vec![0, 0, 255, 255];

    let file = AseFile {
        header: Header::new(4, 4, ColorDepth::Rgba),
        layers: vec![rgba_layer("Bottom"), rgba_layer("Top")],
        palette: None,
        tags: Vec::new(),
        tilesets: Vec::new(),
        frames: vec![
            Frame {
                duration: 100,
                cels: vec![
                    CelChunk {
                        layer_index: 0,
                        x: 0,
                        y: 0,
                        opacity: 255,
                        z_index: 0,
                        content: CelContent::Image {
                            width: 1,
                            height: 1,
                            data: red.clone(),
                        },
                    },
                    CelChunk {
                        layer_index: 1,
                        x: 1,
                        y: 1,
                        opacity: 255,
                        z_index: 0,
                        content: CelContent::Image {
                            width: 1,
                            height: 1,
                            data: green.clone(),
                        },
                    },
                ],
            },
            Frame {
                duration: 100,
                cels: vec![CelChunk {
                    layer_index: 1,
                    x: 2,
                    y: 2,
                    opacity: 128,
                    z_index: 0,
                    content: CelContent::Image {
                        width: 1,
                        height: 1,
                        data: blue.clone(),
                    },
                }],
            },
        ],
    };
    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("loader parses our output");

    assert_eq!(parsed.layers.len(), 2);
    assert_eq!(parsed.frames.len(), 2);

    // Frame 0: both layers populated.
    let f0_l0 = parsed.frames[0].cels[0].as_ref().unwrap();
    let f0_l1 = parsed.frames[0].cels[1].as_ref().unwrap();
    assert_eq!((f0_l0.x, f0_l0.y), (0, 0));
    assert_eq!((f0_l1.x, f0_l1.y), (1, 1));

    // Frame 1: only layer 1 populated.
    assert!(parsed.frames[1].cels[0].is_none());
    let f1_l1 = parsed.frames[1].cels[1].as_ref().unwrap();
    assert_eq!(f1_l1.opacity, 128);
    assert_eq!((f1_l1.x, f1_l1.y), (2, 2));

    // Spot-check pixel data on one cel.
    if let LoaderCelContent::Image(img) = &f0_l0.content {
        let mut decoded = vec![0u8; 4];
        decompress(img.data, &mut decoded).unwrap();
        assert_eq!(decoded, red);
    } else {
        panic!("expected image cel for f0_l0");
    }
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
        tilesets: Vec::new(),
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

// ---- M8.5: tileset + tilemap cel round-trips ---------------------------

const TILE_ID_MASK: u32 = 0x1fff_ffff;
const Y_FLIP_MASK: u32 = 0x2000_0000;
const X_FLIP_MASK: u32 = 0x4000_0000;
const DIAG_FLIP_MASK: u32 = 0x8000_0000;

/// Build an [`AseFile`] with one image layer, one tilemap layer, one
/// inline tileset, and one tilemap cel. Reused by the two round-trip
/// tests below.
fn build_tilemap_file() -> AseFile {
    // 2x2 RGBA tiles: tile 0 transparent, tile 1 four red pixels.
    let tile0 = vec![0u8; 2 * 2 * 4];
    let tile1: Vec<u8> = (0..4).flat_map(|_| [255, 0, 0, 255]).collect();
    let mut tile_pixels = Vec::with_capacity(tile0.len() + tile1.len());
    tile_pixels.extend_from_slice(&tile0);
    tile_pixels.extend_from_slice(&tile1);

    AseFile {
        header: Header::new(8, 8, ColorDepth::Rgba),
        layers: vec![
            LayerChunk {
                flags: LayerFlags::VISIBLE | LayerFlags::EDITABLE,
                layer_type: LayerType::Normal,
                child_level: 0,
                blend_mode: BlendMode::Normal,
                opacity: 255,
                name: "img".into(),
                tileset_index: None,
            },
            LayerChunk {
                flags: LayerFlags::VISIBLE | LayerFlags::EDITABLE,
                layer_type: LayerType::Tilemap,
                child_level: 0,
                blend_mode: BlendMode::Normal,
                opacity: 200,
                name: "tiles".into(),
                tileset_index: Some(0),
            },
        ],
        palette: None,
        tags: Vec::new(),
        tilesets: vec![TilesetChunk {
            id: 0,
            number_of_tiles: 2,
            tile_width: 2,
            tile_height: 2,
            base_index: 1,
            name: "ts".into(),
            tile_pixels,
        }],
        frames: vec![Frame {
            duration: 100,
            cels: vec![CelChunk {
                layer_index: 1,
                x: 0,
                y: 0,
                opacity: 255,
                z_index: 0,
                content: CelContent::Tilemap {
                    width: 2,
                    height: 2,
                    bits_per_tile: 32,
                    bitmask_tile_id: TILE_ID_MASK,
                    bitmask_x_flip: X_FLIP_MASK,
                    bitmask_y_flip: Y_FLIP_MASK,
                    bitmask_diagonal_flip: DIAG_FLIP_MASK,
                    tiles: vec![0, 1, 1 | X_FLIP_MASK, Y_FLIP_MASK],
                },
            }],
        }],
    }
}

#[test]
fn tileset_chunk_roundtrips_through_loader() {
    let bytes = write_to_vec(&build_tilemap_file());

    // The high-level `parse_file` discards `Chunk::Tileset` entries
    // (see aseprite-loader 0.4.2 file.rs); use the raw parser to
    // pick the tileset chunk out by hand.
    let raw = parse_raw_file(&bytes).expect("raw parse");
    let mut tileset = None;
    for frame in &raw.frames {
        for chunk in &frame.chunks {
            if let Chunk::Tileset(ts) = chunk {
                tileset = Some(ts);
            }
        }
    }
    let tileset = tileset.expect("emitted tileset chunk is recovered by aseprite-loader");
    assert_eq!(tileset.id, 0);
    assert_eq!(tileset.number_of_tiles, 2);
    assert_eq!(tileset.width, 2);
    assert_eq!(tileset.height, 2);
    assert_eq!(tileset.base_index, 1);
    assert_eq!(tileset.name, "ts");

    let tiles_block = tileset.tiles.as_ref().expect("inline tile data is emitted");
    let mut decoded = vec![0u8; 2 * 2 * 2 * 4];
    decompress(tiles_block.data, &mut decoded).expect("tile zlib data decompresses");
    // Tile 0 is all-zero (transparent placeholder).
    assert!(decoded[..16].iter().all(|&b| b == 0));
    // Tile 1 is four red pixels.
    assert_eq!(decoded[16..], [255, 0, 0, 255].repeat(4));
}

#[test]
fn tilemap_cel_roundtrips_through_loader() {
    let bytes = write_to_vec(&build_tilemap_file());

    let parsed = parse_file(&bytes).expect("loader parses our output");
    assert_eq!(parsed.layers.len(), 2);
    assert_eq!(parsed.layers[1].layer_type, LoaderLayerType::Tilemap);
    assert_eq!(parsed.layers[1].tileset_index, Some(0));
    assert_eq!(parsed.layers[1].opacity, 200);
    let cel = parsed.frames[0].cels[1]
        .as_ref()
        .expect("tilemap cel present on layer 1 / frame 0");
    let LoaderCelContent::CompressedTilemap {
        width,
        height,
        bits_per_tile,
        bitmask_tile_id,
        bitmask_x_flip,
        bitmask_y_flip,
        bitmask_diagonal_flip,
        data,
    } = &cel.content
    else {
        panic!("expected CompressedTilemap, got {:?}", cel.content);
    };
    assert_eq!(*width, 2);
    assert_eq!(*height, 2);
    assert_eq!(*bits_per_tile, 32);
    assert_eq!(*bitmask_tile_id, TILE_ID_MASK);
    assert_eq!(*bitmask_x_flip, X_FLIP_MASK);
    assert_eq!(*bitmask_y_flip, Y_FLIP_MASK);
    assert_eq!(*bitmask_diagonal_flip, DIAG_FLIP_MASK);

    let mut buf = vec![0u8; 4 * 4];
    decompress(data, &mut buf).expect("tilemap zlib data decompresses");
    let mut iter = buf.chunks_exact(4);
    let read = |it: &mut std::slice::ChunksExact<'_, u8>| {
        let c = it.next().unwrap();
        u32::from_le_bytes([c[0], c[1], c[2], c[3]])
    };
    assert_eq!(read(&mut iter), 0);
    assert_eq!(read(&mut iter), 1);
    assert_eq!(read(&mut iter), 1 | X_FLIP_MASK);
    assert_eq!(read(&mut iter), Y_FLIP_MASK);
}
