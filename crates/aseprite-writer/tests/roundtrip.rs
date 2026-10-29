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
    AnimationDirection, AseFile, BlendMode, CelChunk, CelContent, Color, ColorDepth, ExternalFile,
    Frame, Header, LayerChunk, LayerFlags, LayerType, NinePatch, PaletteChunk, PaletteEntry,
    SliceChunk, SliceKey, Tag, TilesetChunk, UserData, write,
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
        slices: Vec::new(),
        frames: vec![Frame::new(100)],
        user_data: None,
        external_files: Vec::new(),
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
            user_data: None,
        },
        LayerChunk {
            flags: LayerFlags::VISIBLE,
            layer_type: LayerType::Normal,
            child_level: 0,
            blend_mode: BlendMode::Multiply,
            opacity: 200,
            name: "Shadow".into(),
            tileset_index: None,
            user_data: None,
        },
        LayerChunk {
            flags: LayerFlags::VISIBLE,
            layer_type: LayerType::Normal,
            child_level: 0,
            blend_mode: BlendMode::Divide,
            opacity: 128,
            name: "Highlight".into(),
            tileset_index: None,
            user_data: None,
        },
    ];

    let file = AseFile {
        header: Header::new(32, 32, ColorDepth::Rgba),
        layers,
        palette: None,
        tags: Vec::new(),
        tilesets: Vec::new(),
        slices: Vec::new(),
        frames: vec![Frame::new(80)],
        user_data: None,
        external_files: Vec::new(),
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
        slices: Vec::new(),
        frames: vec![Frame::new(100)],
        user_data: None,
        external_files: Vec::new(),
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
            user_data: None,
        },
        Tag {
            from_frame: 3,
            to_frame: 5,
            direction: AnimationDirection::PingPong,
            repeat: 3,
            color: [0x00, 0xFF, 0x00],
            name: "swing".into(),
            user_data: None,
        },
        Tag {
            from_frame: 6,
            to_frame: 6,
            direction: AnimationDirection::Reverse,
            repeat: 1,
            color: [0x00, 0x00, 0xFF],
            name: "blink".into(),
            user_data: None,
        },
    ];

    let file = AseFile {
        header: Header::new(8, 8, ColorDepth::Rgba),
        layers: Vec::new(),
        palette: None,
        tags,
        tilesets: Vec::new(),
        slices: Vec::new(),
        frames: (0..7).map(|_| Frame::new(50)).collect(),
        external_files: Vec::new(),
        user_data: None,
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
        user_data: None,
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
        slices: Vec::new(),
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
                user_data: None,
            }],
        }],
        user_data: None,
        external_files: Vec::new(),
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
        slices: Vec::new(),
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
                    user_data: None,
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
                    user_data: None,
                }],
            },
        ],
        user_data: None,
        external_files: Vec::new(),
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
        slices: Vec::new(),
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
                        user_data: None,
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
                        user_data: None,
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
                    user_data: None,
                }],
            },
        ],
        user_data: None,
        external_files: Vec::new(),
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
            user_data: None,
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
            user_data: None,
        }],
        tilesets: Vec::new(),
        slices: Vec::new(),
        frames: vec![Frame::new(100), Frame::new(150)],
        user_data: None,
        external_files: Vec::new(),
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
                user_data: None,
            },
            LayerChunk {
                flags: LayerFlags::VISIBLE | LayerFlags::EDITABLE,
                layer_type: LayerType::Tilemap,
                child_level: 0,
                blend_mode: BlendMode::Normal,
                opacity: 200,
                name: "tiles".into(),
                tileset_index: Some(0),
                user_data: None,
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
            user_data: None,
            tile_user_data: Vec::new(),
        }],
        slices: Vec::new(),
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
                user_data: None,
            }],
        }],
        user_data: None,
        external_files: Vec::new(),
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
    let mut iter = buf.as_chunks::<4>().0.iter();
    let read = |it: &mut std::slice::Iter<'_, [u8; 4]>| u32::from_le_bytes(*it.next().unwrap());
    assert_eq!(read(&mut iter), 0);
    assert_eq!(read(&mut iter), 1);
    assert_eq!(read(&mut iter), 1 | X_FLIP_MASK);
    assert_eq!(read(&mut iter), Y_FLIP_MASK);
}

/// A raw chunk kind, enough to assert the User Data placement.
fn chunk_kinds(bytes: &[u8]) -> Vec<&'static str> {
    let raw = parse_raw_file(bytes).expect("raw parse");
    raw.frames
        .iter()
        .flat_map(|f| f.chunks.iter())
        .map(|c| match c {
            Chunk::Layer(_) => "layer",
            Chunk::Cel(_) => "cel",
            Chunk::Palette(_) => "palette",
            Chunk::Tags(_) => "tags",
            Chunk::UserData(_) => "ud",
            Chunk::Tileset(_) => "tileset",
            Chunk::ExternalFiles(_) => "external",
            Chunk::Slice(_) => "slice",
            _ => "other",
        })
        .collect()
}

#[test]
fn user_data_follows_its_layer_cel_palette_and_tags() {
    let ud = |text: &str| {
        Some(UserData {
            text: Some(text.into()),
            color: Some([1, 2, 3, 4]),
            properties: None,
        })
    };
    let mut layer = rgba_layer("bg");
    layer.user_data = ud("layer note");
    let file = AseFile {
        header: Header::new(2, 2, ColorDepth::Rgba),
        layers: vec![layer, rgba_layer("fg")],
        palette: Some(PaletteChunk {
            first_color: 0,
            entries: vec![PaletteEntry {
                color: Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 255,
                },
                name: None,
            }],
        }),
        tags: vec![
            Tag {
                from_frame: 0,
                to_frame: 0,
                direction: AnimationDirection::Forward,
                repeat: 0,
                color: [0, 0, 0],
                name: "a".into(),
                user_data: None,
            },
            Tag {
                from_frame: 0,
                to_frame: 0,
                direction: AnimationDirection::Forward,
                repeat: 0,
                color: [0, 0, 0],
                name: "b".into(),
                user_data: ud("tag b"),
            },
        ],
        tilesets: Vec::new(),
        slices: Vec::new(),
        frames: vec![Frame {
            duration: 100,
            cels: vec![CelChunk {
                layer_index: 1,
                x: 0,
                y: 0,
                opacity: 255,
                z_index: -2,
                content: CelContent::Image {
                    width: 2,
                    height: 2,
                    data: checker_rgba(2, 2),
                },
                user_data: ud("cel note"),
            }],
        }],
        user_data: ud("sprite note"),
        external_files: Vec::new(),
    };
    let bytes = write_to_vec(&file);
    assert_eq!(
        chunk_kinds(&bytes),
        vec![
            "palette", "ud", "tags", "ud", "ud", "layer", "ud", "layer", "cel", "ud"
        ]
    );
    let raw = parse_raw_file(&bytes).expect("raw parse");
    let texts: Vec<Option<&str>> = raw.frames[0]
        .chunks
        .iter()
        .filter_map(|c| match c {
            Chunk::UserData(u) => Some(u.text),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        vec![
            Some("sprite note"),
            None,
            Some("tag b"),
            Some("layer note"),
            Some("cel note")
        ]
    );
    let parsed = parse_file(&bytes).expect("parse");
    // `cels` is indexed by layer; the cel sits on layer 1.
    assert_eq!(parsed.frames[0].cels[1].as_ref().expect("cel").z_index, -2);
}

/// One property map (key 0) holding `hp: int32 = 7`, as raw block bytes.
fn hp_properties() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&0u32.to_le_bytes()); // size, patched below
    b.extend_from_slice(&1u32.to_le_bytes()); // one map
    b.extend_from_slice(&0u32.to_le_bytes()); // key 0: user properties
    b.extend_from_slice(&1u32.to_le_bytes()); // one property
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(b"hp");
    b.extend_from_slice(&0x0006u16.to_le_bytes()); // int32
    b.extend_from_slice(&7i32.to_le_bytes());
    let len = b.len() as u32;
    b[..4].copy_from_slice(&len.to_le_bytes());
    b
}

#[test]
fn user_data_properties_are_written_verbatim_and_parse() {
    use aseprite_loader::binary::chunks::user_data::Value;
    let props = hp_properties();
    let mut layer = rgba_layer("bg");
    layer.user_data = Some(UserData {
        text: Some("note".into()),
        color: None,
        properties: Some(props.clone()),
    });
    let file = AseFile {
        header: Header::new(2, 2, ColorDepth::Rgba),
        layers: vec![layer],
        palette: None,
        tags: Vec::new(),
        tilesets: Vec::new(),
        slices: Vec::new(),
        frames: vec![Frame::new(100)],
        user_data: None,
        external_files: Vec::new(),
    };
    let bytes = write_to_vec(&file);
    // The block sits verbatim at the end of the user-data chunk.
    assert!(bytes.windows(props.len()).any(|w| w == props.as_slice()));
    let raw = parse_raw_file(&bytes).expect("raw parse");
    let ud = raw.frames[0]
        .chunks
        .iter()
        .find_map(|c| match c {
            Chunk::UserData(u) => Some(u),
            _ => None,
        })
        .expect("user data chunk");
    assert_eq!(ud.text, Some("note"));
    let (_, maps) = ud
        .properties_maps
        .as_ref()
        .expect("properties flag")
        .as_ref()
        .expect("maps parse");
    assert_eq!(maps.len(), 1);
    assert_eq!(maps[0].extension_entry_id, 0);
    assert_eq!(maps[0].properties[0].name, "hp");
    assert!(matches!(maps[0].properties[0].value, Value::Int32(7)));
}

#[test]
fn user_data_properties_with_a_wrong_size_are_rejected() {
    let mut props = hp_properties();
    props.push(0); // one stray byte the size field does not count
    let mut layer = rgba_layer("bg");
    layer.user_data = Some(UserData {
        text: None,
        color: None,
        properties: Some(props),
    });
    let file = AseFile {
        header: Header::new(2, 2, ColorDepth::Rgba),
        layers: vec![layer],
        palette: None,
        tags: Vec::new(),
        tilesets: Vec::new(),
        slices: Vec::new(),
        frames: vec![Frame::new(100)],
        user_data: None,
        external_files: Vec::new(),
    };
    let mut buf = Vec::new();
    assert!(matches!(
        write(&file, &mut buf),
        Err(aseprite_writer::WriteError::UserDataPropertiesSize { .. })
    ));
}

#[test]
fn unknown_tag_direction_roundtrips_as_its_raw_byte() {
    let file = AseFile {
        header: Header::new(2, 2, ColorDepth::Rgba),
        layers: Vec::new(),
        palette: None,
        tags: vec![Tag {
            from_frame: 0,
            to_frame: 0,
            direction: AnimationDirection::Other(7),
            repeat: 0,
            color: [0, 0, 0],
            name: "future".into(),
            user_data: None,
        }],
        tilesets: Vec::new(),
        slices: Vec::new(),
        frames: vec![Frame::new(100)],
        user_data: None,
        external_files: Vec::new(),
    };
    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("parse");
    assert!(matches!(
        parsed.tags[0].animation_direction,
        LoaderDirection::Unknown(7)
    ));
}

#[test]
fn external_files_and_tileset_user_data_are_written_in_place() {
    use aseprite_loader::binary::chunks::external_files::ExternalFileType;
    let mut file = build_tilemap_file();
    file.external_files = vec![
        ExternalFile {
            id: 5,
            kind: 2,
            name: "my-extension".into(),
        },
        ExternalFile {
            id: 6,
            kind: 3,
            name: "tiles-ext".into(),
        },
    ];
    let note = |text: &str| UserData {
        text: Some(text.into()),
        color: None,
        properties: None,
    };
    file.tilesets[0].user_data = Some(note("tileset"));
    // Tile 0 has none (written empty, keeping tile 1 in place); nothing
    // past the last tile with data is written.
    file.tilesets[0].tile_user_data = vec![None, Some(note("tile 1")), None];
    let bytes = write_to_vec(&file);

    let kinds = chunk_kinds(&bytes);
    assert_eq!(kinds[0], "external");
    let ts = kinds.iter().position(|k| *k == "tileset").expect("tileset");
    assert_eq!(&kinds[ts..ts + 4], &["tileset", "ud", "ud", "ud"]);
    assert_ne!(kinds.get(ts + 4), Some(&"ud"));

    let raw = parse_raw_file(&bytes).expect("raw parse");
    let chunks: Vec<_> = raw.frames[0].chunks.iter().collect();
    let Chunk::ExternalFiles(ext) = chunks[0] else {
        panic!("external files chunk first");
    };
    assert_eq!(ext.files.len(), 2);
    assert_eq!(ext.files[0].entry_id, 5);
    assert_eq!(ext.files[0].file_name, "my-extension");
    assert!(matches!(
        ext.files[0].file_type,
        ExternalFileType::ExtensionNameForProperties
    ));
    // The loader cannot name type 3 (its `Unknown(Byte)` shares the
    // discriminant), so check the raw entry: id 6, then type byte 3.
    assert_eq!(ext.files[1].file_name, "tiles-ext");
    assert!(bytes.windows(5).any(|w| w == [6, 0, 0, 0, 3]));
    let texts: Vec<Option<&str>> = chunks[ts + 1..ts + 4]
        .iter()
        .map(|c| match c {
            Chunk::UserData(u) => u.text,
            _ => panic!("user data"),
        })
        .collect();
    assert_eq!(texts, vec![Some("tileset"), None, Some("tile 1")]);
}

/// A one-frame 4×4 RGBA file with nothing in it.
fn empty_file() -> AseFile {
    AseFile {
        header: Header::new(4, 4, ColorDepth::Rgba),
        layers: Vec::new(),
        palette: None,
        tags: Vec::new(),
        tilesets: Vec::new(),
        slices: Vec::new(),
        frames: vec![Frame::new(100)],
        user_data: None,
        external_files: Vec::new(),
    }
}

fn tag(name: &str, from: u16, to: u16, direction: AnimationDirection) -> Tag {
    Tag {
        from_frame: from,
        to_frame: to,
        direction,
        repeat: 0,
        color: [0, 0, 0],
        name: name.into(),
        user_data: None,
    }
}

#[test]
fn header_fields_roundtrip_through_loader() {
    let mut file = empty_file();
    file.header = Header {
        width: 300,
        height: 7,
        color_depth: ColorDepth::Rgba,
        flags: 0x1,
        speed: 42,
        transparent_index: 3,
        color_count: 9,
        pixel_width: 2,
        pixel_height: 3,
        grid_x: -4,
        grid_y: 5,
        grid_width: 7,
        grid_height: 11,
    };
    let bytes = write_to_vec(&file);
    let h = parse_file(&bytes).expect("parse").header;
    assert_eq!((h.width, h.height), (300, 7));
    assert_eq!(h.color_depth, LoaderColorDepth::Rgba);
    assert_eq!(h.frames, 1);
    assert_eq!(h.flags, 0x1);
    assert_eq!((h.transparent_index, h.color_count), (3, 9));
    assert_eq!((h.pixel_width, h.pixel_height), (2, 3));
    assert_eq!(
        (h.grid_x, h.grid_y, h.grid_width, h.grid_height),
        (-4, 5, 7, 11)
    );
}

#[test]
fn every_blend_mode_roundtrips_through_loader() {
    let modes = [
        (BlendMode::Normal, LoaderBlendMode::Normal),
        (BlendMode::Multiply, LoaderBlendMode::Multiply),
        (BlendMode::Screen, LoaderBlendMode::Screen),
        (BlendMode::Overlay, LoaderBlendMode::Overlay),
        (BlendMode::Darken, LoaderBlendMode::Darken),
        (BlendMode::Lighten, LoaderBlendMode::Lighten),
        (BlendMode::ColorDodge, LoaderBlendMode::ColorDodge),
        (BlendMode::ColorBurn, LoaderBlendMode::ColorBurn),
        (BlendMode::HardLight, LoaderBlendMode::HardLight),
        (BlendMode::SoftLight, LoaderBlendMode::SoftLight),
        (BlendMode::Difference, LoaderBlendMode::Difference),
        (BlendMode::Exclusion, LoaderBlendMode::Exclusion),
        (BlendMode::Hue, LoaderBlendMode::Hue),
        (BlendMode::Saturation, LoaderBlendMode::Saturation),
        (BlendMode::Color, LoaderBlendMode::Color),
        (BlendMode::Luminosity, LoaderBlendMode::Luminosity),
        (BlendMode::Addition, LoaderBlendMode::Addition),
        (BlendMode::Subtract, LoaderBlendMode::Subtract),
        (BlendMode::Divide, LoaderBlendMode::Divide),
    ];
    let mut file = empty_file();
    file.layers = modes
        .iter()
        .enumerate()
        .map(|(i, (mode, _))| {
            let mut layer = rgba_layer(&format!("L{i}"));
            layer.blend_mode = *mode;
            layer
        })
        .collect();
    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("parse");
    let got: Vec<LoaderBlendMode> = parsed.layers.iter().map(|l| l.blend_mode).collect();
    let want: Vec<LoaderBlendMode> = modes.iter().map(|(_, l)| *l).collect();
    assert_eq!(got, want);
}

#[test]
fn every_tag_direction_and_color_roundtrips_through_loader() {
    let mut file = empty_file();
    file.frames = (0..4).map(|_| Frame::new(100)).collect();
    file.tags = vec![
        tag("f", 0, 1, AnimationDirection::Forward),
        tag("r", 1, 2, AnimationDirection::Reverse),
        tag("p", 2, 3, AnimationDirection::PingPong),
        tag("pr", 0, 3, AnimationDirection::PingPongReverse),
    ];
    for (i, t) in file.tags.iter_mut().enumerate() {
        t.repeat = 5 + i as u16;
        t.color = [10 * i as u8 + 1, 20 * i as u8 + 2, 30 * i as u8 + 3];
    }
    let bytes = write_to_vec(&file);
    let parsed = parse_file(&bytes).expect("parse");
    // The loader's direction enum has no `PartialEq`; name it instead.
    let direction = |d: &LoaderDirection| match d {
        LoaderDirection::Forward => "forward",
        LoaderDirection::Reverse => "reverse",
        LoaderDirection::PingPong => "ping-pong",
        LoaderDirection::PingPongReverse => "ping-pong reverse",
        LoaderDirection::Unknown(_) => "unknown",
    };
    let got: Vec<(&str, u16, u16, &str, u16)> = parsed
        .tags
        .iter()
        .map(|t| {
            (
                t.name,
                *t.frames.start(),
                *t.frames.end(),
                direction(&t.animation_direction),
                t.animation_repeat,
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("f", 0, 1, "forward", 5),
            ("r", 1, 2, "reverse", 6),
            ("p", 2, 3, "ping-pong", 7),
            ("pr", 0, 3, "ping-pong reverse", 8),
        ]
    );
    // The loader deprecates the in-chunk tag color, so check each tag
    // entry's color bytes on disk: 6 reserved bytes, then RGB, then one
    // extra byte and the name.
    for t in &file.tags {
        let mut entry = vec![0u8; 6];
        entry.extend_from_slice(&t.color);
        entry.push(0);
        entry.extend_from_slice(&(t.name.len() as u16).to_le_bytes());
        entry.extend_from_slice(t.name.as_bytes());
        assert!(
            bytes.windows(entry.len()).any(|w| w == entry.as_slice()),
            "tag {:?} color {:?} written in its entry",
            t.name,
            t.color
        );
    }
}

#[test]
fn tags_get_no_user_data_chunks_when_no_tag_has_any() {
    let mut file = empty_file();
    file.layers = vec![rgba_layer("bg")];
    file.tags = vec![
        tag("a", 0, 0, AnimationDirection::Forward),
        tag("b", 0, 0, AnimationDirection::Forward),
    ];
    file.frames[0].cels = vec![CelChunk {
        layer_index: 0,
        x: 0,
        y: 0,
        opacity: 255,
        z_index: 0,
        content: CelContent::Image {
            width: 1,
            height: 1,
            data: vec![1, 2, 3, 4],
        },
        user_data: None,
    }];
    let bytes = write_to_vec(&file);
    assert_eq!(chunk_kinds(&bytes), vec!["tags", "layer", "cel"]);

    // Control: one tag with user data emits one chunk per tag up to it.
    file.tags[1].user_data = Some(UserData {
        text: Some("b".into()),
        color: None,
        properties: None,
    });
    let bytes = write_to_vec(&file);
    assert_eq!(
        chunk_kinds(&bytes),
        vec!["tags", "ud", "ud", "layer", "cel"]
    );
}

#[test]
fn slice_user_data_follows_its_slice_chunk() {
    let slice = |name: &str, user_data: Option<UserData>| SliceChunk {
        name: name.into(),
        keys: vec![SliceKey {
            frame: 0,
            x: 0,
            y: 0,
            width: 2,
            height: 2,
            nine_patch: None,
            pivot: None,
        }],
        user_data,
    };
    let mut file = empty_file();
    file.slices = vec![
        slice(
            "first",
            Some(UserData {
                text: Some("first note".into()),
                color: Some([9, 8, 7, 6]),
                properties: None,
            }),
        ),
        slice("plain", None),
        slice(
            "last",
            Some(UserData {
                text: Some("last note".into()),
                color: None,
                properties: None,
            }),
        ),
    ];
    let bytes = write_to_vec(&file);
    assert_eq!(
        chunk_kinds(&bytes),
        vec!["slice", "ud", "slice", "slice", "ud"]
    );
    let raw = parse_raw_file(&bytes).expect("raw parse");
    let chunks = &raw.frames[0].chunks;
    let Chunk::UserData(first) = &chunks[1] else {
        panic!("user data after the first slice");
    };
    assert_eq!(first.text, Some("first note"));
    let color = first.color.expect("first slice color");
    assert_eq!(
        (color.red, color.green, color.blue, color.alpha),
        (9, 8, 7, 6)
    );
    let Chunk::UserData(last) = &chunks[4] else {
        panic!("user data after the last slice");
    };
    assert_eq!(last.text, Some("last note"));
}

/// One of everything that goes into the first frame: external files, a
/// palette with sprite user data, a 3-tile tileset whose only user data
/// sits on tile 0, two tags (one with user data), an image layer with
/// user data, the tilemap layer, a slice and a cel.
fn first_frame_file() -> AseFile {
    let note = |text: &str| {
        Some(UserData {
            text: Some(text.into()),
            color: None,
            properties: None,
        })
    };
    let mut file = build_tilemap_file();
    file.external_files = vec![ExternalFile {
        id: 1,
        kind: 2,
        name: "ext".into(),
    }];
    file.palette = Some(PaletteChunk {
        first_color: 0,
        entries: vec![PaletteEntry {
            color: Color::rgba(1, 2, 3, 255),
            name: None,
        }],
    });
    file.user_data = note("sprite");
    file.tilesets[0].number_of_tiles = 3;
    file.tilesets[0].tile_pixels.extend(vec![0u8; 2 * 2 * 4]);
    file.tilesets[0].tile_user_data = vec![note("tile 0")];
    file.tags = vec![
        tag("a", 0, 0, AnimationDirection::Forward),
        tag("b", 0, 0, AnimationDirection::Forward),
    ];
    file.tags[1].user_data = note("tag b");
    file.layers[0].user_data = note("img");
    file.slices = vec![SliceChunk {
        name: "s".into(),
        keys: vec![SliceKey {
            frame: 0,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            nine_patch: None,
            pivot: None,
        }],
        user_data: note("slice"),
    }];
    file
}

#[test]
fn tilesets_precede_the_layers_that_reference_them() {
    // Aseprite resolves a tilemap layer's tileset while reading the
    // Layer chunk; a tileset written later is "not found" and the layer
    // (and its cels) are dropped. `aseprite-loader` collects all chunks
    // first, so only the chunk order shows this.
    let kinds = chunk_kinds(&write_to_vec(&first_frame_file()));
    let last_tileset = kinds.iter().rposition(|k| *k == "tileset");
    let first_layer = kinds.iter().position(|k| *k == "layer");
    assert!(
        last_tileset.expect("tileset") < first_layer.expect("layer"),
        "{kinds:?}"
    );
}

#[test]
fn sprite_user_data_precedes_every_object_chunk() {
    // Aseprite gives a User Data chunk to the sprite only while no layer,
    // cel, tag, slice or tileset chunk has been read yet; later it lands
    // on whatever object came last.
    let bytes = write_to_vec(&first_frame_file());
    let raw = parse_raw_file(&bytes).expect("raw parse");
    let sprite_ud = raw.frames[0]
        .chunks
        .iter()
        .position(|c| matches!(c, Chunk::UserData(u) if u.text == Some("sprite")))
        .expect("sprite user data written");
    let first_object = chunk_kinds(&bytes)
        .iter()
        .position(|k| matches!(*k, "layer" | "cel" | "tags" | "slice" | "tileset"))
        .expect("object chunks written");
    assert!(sprite_ud < first_object, "{:?}", chunk_kinds(&bytes));
}

#[test]
fn tileset_user_data_has_one_chunk_per_tile() {
    // Once a tileset has user data, Aseprite reads exactly one User Data
    // chunk per tile and warns about any other chunk in that run.
    let kinds = chunk_kinds(&write_to_vec(&first_frame_file()));
    let ts = kinds.iter().position(|k| *k == "tileset").expect("tileset");
    assert_eq!(&kinds[ts..ts + 5], &["tileset", "ud", "ud", "ud", "ud"]);
    assert_ne!(kinds.get(ts + 5), Some(&"ud"), "{kinds:?}");
}

#[test]
fn first_frame_chunk_order_matches_aseprite() {
    // Aseprite's encoder: external files, palette, sprite user data,
    // tilesets, tags, layers, slices, then the cels.
    assert_eq!(
        chunk_kinds(&write_to_vec(&first_frame_file())),
        vec![
            "external", "palette", "ud", "tileset", "ud", "ud", "ud", "ud", "tags", "ud", "ud",
            "layer", "ud", "layer", "slice", "ud", "cel",
        ]
    );
}

/// Write `file`, expect an error and check no byte reached the output.
fn rejected(file: &AseFile) -> aseprite_writer::WriteError {
    let mut buf = Vec::new();
    let err = write(file, &mut buf).expect_err("input the format cannot hold");
    assert!(buf.is_empty(), "nothing written on rejection: {err}");
    err
}

#[test]
fn write_rejects_unrepresentable_input_without_writing_bytes() {
    use aseprite_writer::WriteError;

    let mut file = empty_file();
    file.palette = Some(PaletteChunk {
        first_color: 0,
        entries: Vec::new(),
    });
    assert!(matches!(rejected(&file), WriteError::EmptyPalette));

    let mut file = empty_file();
    let mut tiles = rgba_layer("tiles");
    tiles.layer_type = LayerType::Tilemap;
    file.layers = vec![tiles];
    assert!(matches!(
        rejected(&file),
        WriteError::MissingTilesetIndex { name } if name == "tiles"
    ));

    let mut file = empty_file();
    file.frames = (0..6).map(|_| Frame::new(100)).collect();
    file.tags = vec![tag("bad", 5, 2, AnimationDirection::Forward)];
    assert!(matches!(
        rejected(&file),
        WriteError::InvalidTagRange { from: 5, to: 2, .. }
    ));

    let key = |frame: u32, nine_patch: Option<NinePatch>| SliceKey {
        frame,
        x: 0,
        y: 0,
        width: 4,
        height: 4,
        nine_patch,
        pivot: None,
    };
    let mut file = empty_file();
    file.frames = (0..4).map(|_| Frame::new(100)).collect();
    file.slices = vec![SliceChunk {
        name: "backwards".into(),
        keys: vec![key(3, None), key(1, None)],
        user_data: None,
    }];
    assert!(matches!(
        rejected(&file),
        WriteError::SliceKeysNotMonotonic {
            prev: 3,
            next: 1,
            ..
        }
    ));

    let center = NinePatch {
        x: 1,
        y: 1,
        width: 2,
        height: 2,
    };
    file.slices = vec![SliceChunk {
        name: "mixed".into(),
        keys: vec![key(0, None), key(1, Some(center))],
        user_data: None,
    }];
    assert!(matches!(
        rejected(&file),
        WriteError::SliceFlagsInconsistent { .. }
    ));
}
