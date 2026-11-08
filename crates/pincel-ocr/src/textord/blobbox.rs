//! `BLOBNBOX`, Tesseract's blob with its layout state
//! (`src/ccstruct/blobbox.cpp`).

use super::blob::CBlob;
use super::rect::TBox;

/// A blob on its way through the page layout.
#[derive(Debug, Clone)]
pub(crate) struct BlobBox {
    pub(crate) bbox: TBox,
    /// `enclosed_area`: the signed area of the outlines.
    pub(crate) area: i32,
    /// The outlines; `None` for the fake blobs `chop` makes.
    pub(crate) cblob: Option<CBlob>,
    /// `joined_to_prev`: merged into the blob before it.
    pub(crate) joined: bool,
    /// `baseline_position`.
    pub(crate) baseline_y: i32,
    /// `reduced_box`, once `reduced_box_for_blob` has set it.
    pub(crate) reduced: Option<TBox>,
}

impl BlobBox {
    /// `BLOBNBOX(C_BLOB*)`.
    pub(crate) fn new(cblob: CBlob) -> Self {
        let bbox = cblob.bounding_box();
        Self {
            bbox,
            area: cblob.area(),
            cblob: Some(cblob),
            joined: false,
            baseline_y: bbox.bottom,
            reduced: None,
        }
    }

    /// A blob that is only a box (`new BLOBNBOX` with `box` set).
    pub(crate) fn fake(bbox: TBox) -> Self {
        Self {
            bbox,
            area: 0,
            cblob: None,
            joined: false,
            baseline_y: bbox.bottom,
            reduced: None,
        }
    }

    /// `BLOBNBOX::EstimateBaselinePosition`.
    pub(crate) fn estimate_baseline_position(&mut self) {
        self.baseline_y = match &self.cblob {
            Some(c) => c.estimate_baseline_position(),
            None => self.bbox.bottom,
        };
    }
}

/// `box_next_pre_chopped`: the box of the blob at `*i`, then `*i` moves
/// to the next blob that is not joined to its predecessor (cyclically).
pub(crate) fn box_next_pre_chopped(blobs: &[BlobBox], i: &mut usize) -> TBox {
    let result = blobs[*i].bbox;
    loop {
        *i = (*i + 1) % blobs.len();
        if !blobs[*i].joined {
            break;
        }
    }
    result
}
