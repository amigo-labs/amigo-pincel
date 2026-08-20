//! The error type raised by [`super::compose`]. See `docs/specs/pincel.md` §4.

use thiserror::Error;

use crate::document::{BlendMode, ColorMode, FrameIndex, LayerId, TilesetId};

use super::compose::MAX_ZOOM;

/// Errors raised by [`compose`](fn@super::compose).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RenderError {
    /// The sprite uses a color mode the renderer cannot yet handle.
    #[error("unsupported color mode: {mode:?}")]
    UnsupportedColorMode { mode: ColorMode },

    /// `zoom` was outside the supported range `1..=64`.
    #[error("invalid zoom: {zoom} (expected 1..={MAX_ZOOM})")]
    InvalidZoom { zoom: u32 },

    /// `viewport` was empty (zero width or height).
    #[error("empty viewport")]
    EmptyViewport,

    /// `frame` did not refer to a frame in the sprite.
    #[error("unknown frame index: {frame:?}")]
    UnknownFrame { frame: FrameIndex },

    /// A layer's content cannot be composed in this milestone.
    #[error("unsupported layer kind on layer {layer:?}")]
    UnsupportedLayerKind { layer: LayerId },

    /// A layer's blend mode is not yet implemented.
    #[error("unsupported blend mode {mode:?} on layer {layer:?}")]
    UnsupportedBlendMode { layer: LayerId, mode: BlendMode },

    /// A linked cel was encountered. Linked cels share data with another
    /// frame's cel; M3 does not follow links — the loader (M4) is the layer
    /// that resolves linkage.
    #[error("linked cel on layer {layer:?} frame {frame:?} is not yet supported")]
    LinkedCelUnsupported { layer: LayerId, frame: FrameIndex },

    /// A cel's pixel buffer uses a color mode that doesn't match the
    /// sprite's color mode.
    #[error(
        "cel buffer color mode {mode:?} on layer {layer:?} frame {frame:?} \
         doesn't match sprite color mode"
    )]
    CelColorModeMismatch {
        layer: LayerId,
        frame: FrameIndex,
        mode: ColorMode,
    },

    /// A cel's pixel buffer dimensions don't match its byte length.
    #[error("malformed cel buffer on layer {layer:?} frame {frame:?}")]
    MalformedCelBuffer { layer: LayerId, frame: FrameIndex },

    /// A cel's payload type is incompatible with its layer's kind (for
    /// example, tilemap data on an image layer). Indicates a corrupt
    /// document.
    #[error("cel type does not match layer kind on layer {layer:?} frame {frame:?}")]
    CelTypeMismatch { layer: LayerId, frame: FrameIndex },

    /// The request asked for an onion-skin overlay; M3 does not render
    /// onion skin yet.
    #[error("onion skin is not yet supported")]
    OnionSkinUnsupported,

    /// The request asked for one or more decoration overlays; M3 does not
    /// render overlays yet.
    #[error("overlays are not yet supported")]
    OverlaysUnsupported,

    /// A tilemap layer references a tileset id that doesn't exist on the
    /// sprite.
    #[error("tileset {tileset:?} for layer {layer:?} not found")]
    TilesetNotFound { layer: LayerId, tileset: TilesetId },

    /// A tilemap cel references a tile id that's past the end of the
    /// tileset's tile list. Indicates a corrupt document or a stale cel
    /// after tiles were removed.
    #[error("tile id {tile_id} on layer {layer:?} frame {frame:?} is out of range")]
    TileIdOutOfRange {
        layer: LayerId,
        frame: FrameIndex,
        tile_id: u32,
    },

    /// A tile image's dimensions don't match the tileset's declared
    /// `tile_size`.
    #[error("tile {tile_id} dimensions don't match tileset {tileset:?} on layer {layer:?}")]
    TileSizeMismatch {
        layer: LayerId,
        tileset: TilesetId,
        tile_id: u32,
    },

    /// A `TileRef::rotate_90` flag was set on a non-square tileset. Phase 1
    /// only supports 90° rotation on square tiles; non-square rotation is
    /// deferred to Phase 2.
    #[error(
        "rotate_90 on non-square tileset {tileset:?} (tile_size {tile_size:?}) \
         is not yet supported"
    )]
    NonSquareRotateUnsupported {
        layer: LayerId,
        tileset: TilesetId,
        tile_size: (u32, u32),
    },
}
