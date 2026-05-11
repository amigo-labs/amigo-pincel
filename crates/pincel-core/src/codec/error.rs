//! Error type for codec adapters.

use thiserror::Error;

use crate::error::DocumentError;

/// Errors produced by the Aseprite read / write adapters.
#[derive(Debug, Error)]
pub enum CodecError {
    /// The underlying parser rejected the byte stream.
    #[error("aseprite parse failed: {0}")]
    Parse(String),

    /// The file uses a color depth Pincel does not yet ingest. The current
    /// adapter handles RGBA only; indexed and grayscale are deferred.
    #[error("unsupported aseprite color mode (only RGBA is currently supported)")]
    UnsupportedColorMode,

    /// The file contains a layer kind we do not yet round-trip (tilemap layers
    /// land in M8, unknown layer types are forwarded as-is on write).
    #[error("unsupported aseprite layer kind: {kind}")]
    UnsupportedLayerKind {
        /// Layer kind code as encoded in the source file (`0x2004` chunk word).
        kind: u16,
    },

    /// The file uses a blend mode Pincel does not recognize.
    #[error("unsupported aseprite blend mode: {mode}")]
    UnsupportedBlendMode {
        /// Blend mode code as encoded in the source file.
        mode: u16,
    },

    /// The file contains a cel kind we do not yet ingest (compressed tilemap
    /// is M8; unknown cel types are forwarded as-is on write).
    #[error("unsupported aseprite cel kind: {kind}")]
    UnsupportedCelKind {
        /// Cel type code as encoded in the source file (Cel chunk type word).
        kind: u16,
    },

    /// The file references a layer index outside the layer table.
    #[error("cel references unknown layer index: {index}")]
    LayerIndexOutOfRange {
        /// 0-based layer index taken from the cel chunk.
        index: usize,
    },

    /// Pixel decode failed inside `aseprite-loader`.
    #[error("aseprite image decode failed: {0}")]
    Image(String),

    /// Building the [`crate::Sprite`] model rejected the adapter output.
    #[error(transparent)]
    Document(#[from] DocumentError),

    /// A field on the document does not fit into its on-disk slot
    /// (canvas dimensions / cel position / cel dimensions / linked frame
    /// index / tag frame range / layer count / layer depth).
    #[error("{what} value {value} does not fit on disk")]
    OutOfRange {
        /// Short human-readable description, e.g. `"canvas width"`.
        what: &'static str,
        /// Numeric value that exceeded the on-disk slot. Signed so that
        /// negative cel positions surface verbatim.
        value: i64,
    },

    /// A linked cel points at a frame index past `Sprite::frames`.
    #[error("linked cel target frame {index} is past sprite.frames")]
    LinkedFrameNotFound {
        /// Frame index referenced by the linked cel.
        index: u32,
    },

    /// A layer's `parent` references a [`crate::LayerId`] that isn't
    /// present in `Sprite::layers`.
    #[error("layer parent {id} not found in sprite.layers")]
    LayerParentNotFound {
        /// Numeric value of the missing parent's [`crate::LayerId`].
        id: u32,
    },

    /// The layer parent graph contains a cycle reachable from the named
    /// layer id.
    #[error("layer parent graph contains a cycle at layer id {id}")]
    LayerCycle {
        /// Numeric value of a [`crate::LayerId`] inside the cycle.
        id: u32,
    },

    /// A cel references a [`crate::LayerId`] that isn't present in
    /// `Sprite::layers`.
    #[error("cel references unknown layer id {id}")]
    CelLayerNotFound {
        /// Numeric value of the cel's [`crate::LayerId`].
        id: u32,
    },

    /// A cel's frame index is past `Sprite::frames`.
    #[error("cel frame index {index} is past sprite.frames")]
    CelFrameNotFound {
        /// Frame index that did not match any frame in `Sprite::frames`.
        index: u32,
    },

    /// `aseprite-writer` rejected the staged file. Pre-validation in the
    /// adapter catches the structural mistakes; this variant carries
    /// errors that the writer itself raises (e.g. zlib I/O failures).
    #[error(transparent)]
    Write(#[from] aseprite_writer::WriteError),

    /// A layer's parent is not a [`crate::LayerKind::Group`]. Aseprite
    /// only nests layers under groups, so the round-trip through
    /// [`super::aseprite_read`] would lose the parent link otherwise.
    #[error("layer {child} has parent {parent} which is not a group layer")]
    LayerParentNotGroup {
        /// Layer whose `parent` references a non-group.
        child: u32,
        /// Parent layer that should have been a group.
        parent: u32,
    },

    /// The layer order in `Sprite::layers` cannot be flattened into
    /// Aseprite's `child_level` encoding without changing the parent
    /// links. The reader reconstructs parents from a stack of group
    /// layers walked in order; if the parent appears after the child or
    /// is shadowed by a sibling group at the same depth, the round-trip
    /// would silently produce a different document.
    #[error(
        "layer {child} parent {expected:?} would be reconstructed as {reconstructed:?} after a write→read round-trip"
    )]
    LayerOrderingInconsistent {
        /// Layer whose reconstructed parent does not match its actual parent.
        child: u32,
        /// Numeric value of the layer's actual `parent`, if any.
        expected: Option<u32>,
        /// Numeric value of the parent the reader would reconstruct, if any.
        reconstructed: Option<u32>,
    },

    /// An image cel's [`crate::PixelBuffer::color_mode`] does not match
    /// the sprite's color mode. The on-disk header dictates the
    /// bytes-per-pixel for every cel, so a mismatched buffer would emit
    /// bytes the reader cannot interpret.
    #[error("image cel buffer for layer {layer} frame {frame} is not RGBA")]
    CelImageNotRgba {
        /// Numeric value of the cel's [`crate::LayerId`].
        layer: u32,
        /// Frame index of the cel.
        frame: u32,
    },

    /// An image cel's [`crate::PixelBuffer`] has a `data` length that
    /// disagrees with `width * height * bytes_per_pixel`.
    #[error("image cel buffer for layer {layer} frame {frame} is malformed")]
    CelImageBufferMalformed {
        /// Numeric value of the cel's [`crate::LayerId`].
        layer: u32,
        /// Frame index of the cel.
        frame: u32,
    },

    /// A linked cel points at a `(layer, frame)` slot that has no cel.
    /// `aseprite-loader` rejects linked cels whose target is missing,
    /// so the produced file would fail to load.
    #[error(
        "linked cel for layer {layer} frame {from_frame} targets frame {target} which has no cel"
    )]
    LinkedCelTargetMissing {
        /// Numeric value of the cel's [`crate::LayerId`].
        layer: u32,
        /// Frame index that owns the linked cel.
        from_frame: u32,
        /// Frame index the linked cel points at.
        target: u32,
    },

    /// A linked cel points at a `(layer, frame)` slot whose cel is not
    /// itself an image cel (chained links and tilemap targets are not
    /// representable in the on-disk format).
    #[error(
        "linked cel for layer {layer} frame {from_frame} targets frame {target} which is not an image cel"
    )]
    LinkedCelTargetNotImage {
        /// Numeric value of the cel's [`crate::LayerId`].
        layer: u32,
        /// Frame index that owns the linked cel.
        from_frame: u32,
        /// Frame index the linked cel points at.
        target: u32,
    },

    /// A tilemap layer's `tileset_index` was missing from the layer chunk.
    /// Aseprite always emits this field for `LayerType::Tilemap`, so a
    /// missing value indicates a malformed or truncated file.
    #[error("tilemap layer {name:?} is missing its tileset index")]
    TilemapLayerMissingTilesetIndex {
        /// Layer name as encoded in the source file.
        name: String,
    },

    /// A tilemap cel uses a `bits_per_tile` value other than 32. The
    /// Aseprite v1.3 spec currently fixes this at 32 bits per tile;
    /// anything else is rejected so that the bitmask layout is well-defined.
    #[error("tilemap cel uses unsupported bits_per_tile {bits}")]
    TilemapBitsPerTileUnsupported {
        /// Bits-per-tile value reported by the cel chunk.
        bits: u16,
    },

    /// A tilemap cel's compressed payload did not decompress to the
    /// expected `width * height * bits_per_tile / 8` bytes.
    #[error("tilemap cel decode failed: {0}")]
    TilemapDecode(String),

    /// A tileset chunk uses a feature Pincel does not yet ingest. Phase 1
    /// supports inline tile data only; external-file tilesets are deferred.
    #[error("tileset {id} uses an unsupported feature: {what}")]
    TilesetUnsupported {
        /// Numeric tileset id from the chunk.
        id: u32,
        /// Short human-readable description of the rejected feature.
        what: &'static str,
    },

    /// A tileset chunk's compressed tile-image payload did not decompress
    /// to the expected `tile_w * tile_h * number_of_tiles * 4` bytes.
    #[error("tileset {id} tile-image decode failed: {message}")]
    TilesetDecode {
        /// Numeric tileset id from the chunk.
        id: u32,
        /// Underlying decode error message.
        message: String,
    },

    /// A `CelData::Tilemap` cel has `tiles.len()` that does not match
    /// `grid_w * grid_h`. The document model enforces this on
    /// construction, so seeing it here means a caller hand-built a
    /// malformed cel.
    #[error(
        "tilemap cel for layer {layer} frame {frame} has {actual} tiles for a {expected}-tile grid"
    )]
    CelTilemapTileCountMismatch {
        /// Numeric value of the cel's [`crate::LayerId`].
        layer: u32,
        /// Frame index of the cel.
        frame: u32,
        /// Expected `grid_w * grid_h`.
        expected: usize,
        /// Actual `tiles.len()`.
        actual: usize,
    },

    /// A `TileImage` inside a [`crate::Tileset`] has dimensions that
    /// don't match the tileset's `tile_size`, or uses a non-RGBA color
    /// mode. The on-disk tile-image block is one contiguous buffer of
    /// `tile_w * tile_h * number_of_tiles * 4` bytes, so per-tile
    /// dimension drift would produce a corrupt file.
    #[error(
        "tileset {tileset} tile {tile} dimensions {actual_w}x{actual_h} do not match tileset tile_size {expected_w}x{expected_h}"
    )]
    TilesetTileDimensionMismatch {
        /// Numeric tileset id.
        tileset: u32,
        /// 0-based tile index inside the tileset.
        tile: u32,
        /// Tileset's declared tile width.
        expected_w: u32,
        /// Tileset's declared tile height.
        expected_h: u32,
        /// Actual tile width.
        actual_w: u32,
        /// Actual tile height.
        actual_h: u32,
    },

    /// A [`crate::TileImage`] uses a [`crate::ColorMode`] other than
    /// RGBA. The writer is RGBA-only at the sprite level.
    #[error("tileset {tileset} tile {tile} is not RGBA")]
    TilesetTileNotRgba {
        /// Numeric tileset id.
        tileset: u32,
        /// 0-based tile index inside the tileset.
        tile: u32,
    },

    /// A [`crate::Tileset`] has a non-empty `external_file`. Phase 1
    /// does not yet round-trip external-file tilesets (the read path
    /// rejects them too).
    #[error("tileset {tileset} references an external file (not supported)")]
    UnsupportedTilesetExternalFile {
        /// Numeric tileset id.
        tileset: u32,
    },
}
