//! Crate-level error types.

use thiserror::Error;

/// Errors produced by document and command operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DocumentError {
    /// A canvas dimension was outside the allowed `1..=32767` range (spec §3.2).
    #[error("invalid canvas size {width}x{height}: dimensions must be in 1..=32767")]
    InvalidCanvasSize {
        /// Requested width.
        width: u32,
        /// Requested height.
        height: u32,
    },

    /// Attempted to remove the last remaining raster layer (spec §3.2: always
    /// ≥1 layer; docs/specs/image-layer-groups.md AC-20).
    #[error("cannot remove the last remaining layer")]
    LastLayer,

    /// A group was to be moved into itself or one of its descendants.
    #[error("cannot move a group into itself")]
    LayerIntoOwnSubtree,

    /// A layer was to be moved into a layer that is not a group.
    #[error("layer {index} is not a group")]
    NotAGroup {
        /// The offending target index.
        index: usize,
    },

    /// The change would leave the layer list without a well-formed tree.
    #[error("the change would break the layer tree")]
    LayerTreeBroken,

    /// A pixel operation targeted a group, which has no pixels of its own.
    #[error("layer {index} is a group")]
    GroupLayer {
        /// The group's index.
        index: usize,
    },

    /// Merge Down needs two raster layers in the same group.
    #[error("merge down needs two raster layers in the same group")]
    MergeAcrossGroups,

    /// Attempted to exceed the 999-layer limit (spec §3.2).
    #[error("layer limit of {max} reached")]
    LayerLimit {
        /// The maximum number of layers.
        max: usize,
    },

    /// A layer index was out of bounds.
    #[error("layer index {index} out of bounds (len {len})")]
    LayerIndexOutOfBounds {
        /// The offending index.
        index: usize,
        /// Current number of layers.
        len: usize,
    },

    /// A pixel region did not fit the target buffer.
    #[error("region does not fit the target buffer")]
    RegionOutOfBounds,

    /// Bytes handed to `codec::snapshot::from_snapshot_bytes` are not a
    /// whole document snapshot.
    #[error("not a document snapshot")]
    InvalidSnapshot,
}
