//! Editor grid (docs/specs/pixel-grid.md, spec §7.1): the sprite's grid
//! cell size and offset, read and set through the undo bus. The grid is
//! drawn by the UI overlay, so setting it emits no dirty event; the UI
//! re-reads it when it paints.

use pincel_core::{Rect, SetGrid};
use wasm_bindgen::prelude::*;

use crate::Document;

#[wasm_bindgen]
impl Document {
    /// Grid x offset in sprite pixels (0 for a sprite without a stored
    /// grid).
    #[wasm_bindgen(getter, js_name = gridX)]
    pub fn grid_x(&self) -> i32 {
        self.sprite.metadata.grid_or_default().x
    }

    /// Grid y offset in sprite pixels (0 for a sprite without a stored
    /// grid).
    #[wasm_bindgen(getter, js_name = gridY)]
    pub fn grid_y(&self) -> i32 {
        self.sprite.metadata.grid_or_default().y
    }

    /// Grid cell width in sprite pixels (16 for a sprite without a stored
    /// grid; 0 when a file stored a zero-sized grid).
    #[wasm_bindgen(getter, js_name = gridWidth)]
    pub fn grid_width(&self) -> u32 {
        self.sprite.metadata.grid_or_default().width
    }

    /// Grid cell height in sprite pixels (16 for a sprite without a
    /// stored grid; 0 when a file stored a zero-sized grid).
    #[wasm_bindgen(getter, js_name = gridHeight)]
    pub fn grid_height(&self) -> u32 {
        self.sprite.metadata.grid_or_default().height
    }

    /// Set the grid's offset (`x`, `y`) and cell size (`width` ×
    /// `height`) as one undo entry; saved in the `.aseprite` header.
    /// Errors, leaving the grid unchanged, for a cell size outside
    /// 1–65535 or an offset outside −32768–32767.
    #[wasm_bindgen(js_name = setGrid)]
    pub fn set_grid(&mut self, x: i32, y: i32, width: u32, height: u32) -> Result<(), String> {
        self.bus.seal();
        self.bus
            .execute(
                SetGrid::new(Rect::new(x, y, width, height)).into(),
                &mut self.sprite,
                &mut self.cels,
            )
            .map_err(|e| format!("failed to set grid: {e}"))?;
        self.bus.seal();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(doc: &Document) -> (i32, i32, u32, u32) {
        (
            doc.grid_x(),
            doc.grid_y(),
            doc.grid_width(),
            doc.grid_height(),
        )
    }

    #[test]
    fn new_document_grid_is_sixteen_square_at_origin() {
        let doc = Document::new(8, 8).expect("doc");
        assert_eq!(grid(&doc), (0, 0, 16, 16));
    }

    #[test]
    fn set_grid_undoes_and_redoes_in_one_step() {
        let mut doc = Document::new(8, 8).expect("doc");
        doc.set_grid(2, 3, 8, 4).expect("set");
        assert_eq!(grid(&doc), (2, 3, 8, 4));
        assert!(doc.undo());
        assert_eq!(grid(&doc), (0, 0, 16, 16));
        assert!(doc.redo().expect("redo"));
        assert_eq!(grid(&doc), (2, 3, 8, 4));
    }

    #[test]
    fn set_grid_out_of_range_returns_err_and_keeps_grid() {
        let mut doc = Document::new(8, 8).expect("doc");
        doc.set_grid(1, 1, 4, 4).expect("set");
        for (x, y, w, h) in [
            (0, 0, 0, 4),
            (0, 0, 70_000, 4),
            (40_000, 0, 4, 4),
            (0, -40_000, 4, 4),
        ] {
            assert!(doc.set_grid(x, y, w, h).is_err(), "{x},{y} {w}x{h}");
            assert_eq!(grid(&doc), (1, 1, 4, 4));
        }
        assert_eq!(doc.undo_depth(), 1, "refused edits leave no undo entry");
    }

    #[test]
    fn set_grid_survives_save_and_open() {
        let mut doc = Document::new(8, 8).expect("doc");
        doc.set_grid(-4, 6, 24, 10).expect("set");
        let bytes = doc.save_aseprite().expect("save");
        let back = Document::open_aseprite(&bytes).expect("open");
        assert_eq!(grid(&back), (-4, 6, 24, 10));
    }
}
