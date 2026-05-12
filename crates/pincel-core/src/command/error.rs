//! Error type returned by command execution.

use thiserror::Error;

use crate::document::{FrameIndex, LayerId};
use crate::geometry::Rect;

/// Errors raised when a command cannot be applied.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandError {
    /// No cel exists for the targeted `(layer, frame)`.
    #[error("missing cel for layer {layer:?} frame {frame:?}")]
    MissingCel { layer: LayerId, frame: FrameIndex },

    /// The cel for `(layer, frame)` is not an image cel (e.g. tilemap or linked).
    #[error("cel for layer {layer:?} frame {frame:?} is not an image cel")]
    NotAnImageCel { layer: LayerId, frame: FrameIndex },

    /// Pixel coordinates fall outside the cel's pixel buffer.
    #[error("pixel ({x}, {y}) is out of bounds for cel of size {width}x{height} at {position:?}")]
    PixelOutOfBounds {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        position: (i32, i32),
    },

    /// The cel buffer's color mode does not match the operation.
    #[error("unsupported color mode for this command")]
    UnsupportedColorMode,

    /// A layer with the same id already exists.
    #[error("layer id {0} already exists")]
    DuplicateLayerId(u32),

    /// A tileset with the same id already exists.
    #[error("tileset id {0} already exists")]
    DuplicateTilesetId(u32),

    /// The cel for `(layer, frame)` is not a tilemap cel (e.g. image or
    /// linked). Emitted by `PlaceTile`.
    #[error("cel for layer {layer:?} frame {frame:?} is not a tilemap cel")]
    NotATilemapCel { layer: LayerId, frame: FrameIndex },

    /// Grid coordinates fall outside the tilemap cel's grid.
    #[error("tile coord ({x}, {y}) is out of bounds for tilemap grid {grid_w}x{grid_h}")]
    TileCoordOutOfBounds {
        x: u32,
        y: u32,
        grid_w: u32,
        grid_h: u32,
    },

    /// A tilemap cel's `tiles` vector length doesn't equal
    /// `grid_w * grid_h`. Indicates a corrupt document; safer to refuse
    /// the edit than to write into an inconsistently-sized buffer.
    #[error(
        "tilemap cel on layer {layer:?} frame {frame:?} is malformed: \
         tiles_len={tiles_len} but grid is {grid_w}x{grid_h}"
    )]
    MalformedTilemapCel {
        layer: LayerId,
        frame: FrameIndex,
        grid_w: u32,
        grid_h: u32,
        tiles_len: usize,
    },

    /// A selection-scoped command was issued with no active selection
    /// on the sprite. Emitted by `MoveSelectionContent` when there is
    /// nothing to move; the caller (typically the Move tool drag in
    /// the UI) should fall back to a pan / no-op instead.
    #[error("no active selection")]
    NoSelection,

    /// The targeted tileset does not exist on the sprite. Emitted by
    /// [`crate::SetTilePixel`] and any future tileset-scoped command.
    #[error("unknown tileset id {0}")]
    UnknownTileset(u32),

    /// The targeted tile id is past the end of the tileset's stored
    /// tiles. Aseprite convention reserves tile id `0` as the implicit
    /// empty tile; if the caller needs to write to it the tileset
    /// must explicitly grow first.
    #[error("unknown tile id {tile_id} in tileset {tileset}")]
    UnknownTile { tileset: u32, tile_id: u32 },

    /// Pixel coordinates fall outside the targeted tile's pixel
    /// buffer. Emitted by [`crate::SetTilePixel`].
    #[error(
        "tile pixel ({x}, {y}) is out of bounds for tile {tile_id} of tileset {tileset} \
         (size {width}x{height})"
    )]
    TilePixelOutOfBounds {
        tileset: u32,
        tile_id: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },

    /// A slice with the same id already exists. Emitted by
    /// [`crate::AddSlice`].
    #[error("slice id {0} already exists")]
    DuplicateSliceId(u32),

    /// The targeted slice does not exist on the sprite. Emitted by
    /// [`crate::RemoveSlice`] and [`crate::SetSliceKey`].
    #[error("unknown slice id {0}")]
    UnknownSlice(u32),

    /// A slice was constructed with an empty `keys` vector, which the
    /// aseprite format would refuse to encode. Emitted by
    /// [`crate::AddSlice`].
    #[error("slice id {0} must carry at least one key")]
    EmptySliceKeys(u32),

    /// A slice key carries a zero-area bounding rect, which has no
    /// useful semantics. Emitted by [`crate::AddSlice`] and
    /// [`crate::SetSliceKey`].
    #[error("slice key for frame {frame:?} has an empty bounds rect {bounds:?}")]
    EmptySliceBounds { frame: FrameIndex, bounds: Rect },
}
