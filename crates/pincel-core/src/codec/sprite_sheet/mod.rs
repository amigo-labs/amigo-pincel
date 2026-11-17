//! Sprite-sheet import (docs/specs/sprite-sheet-import.md): decode a PNG
//! sheet, cut it along a grid into frames (one per used cell, row-major)
//! and suggest a grid for it.
//!
//! The background step — every pixel exactly equal to a chosen RGBA colour
//! counts as fully transparent — applies to the emptiness test, to the cut
//! frames and to the grid suggestion alike.

mod suggest;
#[cfg(test)]
mod tests;

use thiserror::Error;

use super::png::{ImportError, import_png};
use crate::document::{ColorMode, FrameIndex, PixelBuffer, Tag};

/// Widest and tallest sheet the importer reads, in pixels per side.
pub const MAX_SHEET_SIDE: u32 = 16_384;
/// Largest sheet area the importer reads, in pixels (256 MiB decoded).
pub const MAX_SHEET_PIXELS: u64 = 67_108_864;
/// Largest cell width or height, the cap of File ▸ New.
pub const MAX_SHEET_CELL: u32 = 4096;
/// Most frames one import may create (Aseprite's 16-bit frame count).
pub const MAX_SHEET_FRAMES: u32 = 65_535;

/// Why a sheet could not be read or cut.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SheetError {
    /// The bytes are not a PNG the decoder can read.
    #[error("not a readable PNG: {0}")]
    Decode(String),
    /// The sheet passes [`MAX_SHEET_SIDE`] or [`MAX_SHEET_PIXELS`].
    #[error(
        "sheet of {width}x{height} px is larger than {MAX_SHEET_SIDE} px per side or {MAX_SHEET_PIXELS} pixels"
    )]
    TooLarge {
        /// Sheet width in pixels.
        width: u32,
        /// Sheet height in pixels.
        height: u32,
    },
    /// An allocation for the sheet or its frames failed.
    #[error("out of memory")]
    OutOfMemory,
}

/// Why a grid cannot be imported (spec AC-12). Field errors come first, in
/// field order; then the grid-wide ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GridError {
    /// Cell width outside `1..=MAX_SHEET_CELL`.
    #[error("cell width must be 1 to {MAX_SHEET_CELL}")]
    CellWidth,
    /// Cell height outside `1..=MAX_SHEET_CELL`.
    #[error("cell height must be 1 to {MAX_SHEET_CELL}")]
    CellHeight,
    /// Offset X past the sheet's last column.
    #[error("offset X must be less than the sheet width")]
    OffsetX,
    /// Offset Y past the sheet's last row.
    #[error("offset Y must be less than the sheet height")]
    OffsetY,
    /// Spacing X above the sheet width.
    #[error("spacing X must not exceed the sheet width")]
    SpacingX,
    /// Spacing Y above the sheet height.
    #[error("spacing Y must not exceed the sheet height")]
    SpacingY,
    /// No cell lies completely inside the sheet.
    #[error("the grid has no complete cell")]
    NoCell,
    /// Every complete cell is empty.
    #[error("every cell is empty")]
    AllEmpty,
    /// The import would create more than [`MAX_SHEET_FRAMES`] frames.
    #[error("more than {MAX_SHEET_FRAMES} frames")]
    TooManyFrames,
}

impl GridError {
    /// Stable kebab-case code for the UI to translate.
    pub fn code(self) -> &'static str {
        match self {
            Self::CellWidth => "cell-width",
            Self::CellHeight => "cell-height",
            Self::OffsetX => "offset-x",
            Self::OffsetY => "offset-y",
            Self::SpacingX => "spacing-x",
            Self::SpacingY => "spacing-y",
            Self::NoCell => "no-cell",
            Self::AllEmpty => "all-empty",
            Self::TooManyFrames => "too-many-frames",
        }
    }
}

/// A grid over a sheet (spec AC-02): cells of `cell_width × cell_height`,
/// the first one's top-left at `(offset_x, offset_y)`, each next one
/// `cell_width + spacing_x` to the right and `cell_height + spacing_y`
/// below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SheetGrid {
    /// Cell width in pixels.
    pub cell_width: u32,
    /// Cell height in pixels.
    pub cell_height: u32,
    /// Distance from the sheet's left edge to the first cell.
    pub offset_x: u32,
    /// Distance from the sheet's top edge to the first cell.
    pub offset_y: u32,
    /// Horizontal gap between cells.
    pub spacing_x: u32,
    /// Vertical gap between cells.
    pub spacing_y: u32,
}

impl SheetGrid {
    /// A grid of `cell_width × cell_height` cells, offset 0, spacing 0.
    pub const fn new(cell_width: u32, cell_height: u32) -> Self {
        Self {
            cell_width,
            cell_height,
            offset_x: 0,
            offset_y: 0,
            spacing_x: 0,
            spacing_y: 0,
        }
    }

    /// Number of complete cells along one axis of length `len`.
    fn count(len: u32, offset: u32, cell: u32, spacing: u32) -> u32 {
        let (len, offset, cell, spacing) = (
            u64::from(len),
            u64::from(offset),
            u64::from(cell),
            u64::from(spacing),
        );
        if cell == 0 || offset + cell > len {
            return 0;
        }
        u32::try_from((len - offset - cell) / (cell + spacing) + 1).unwrap_or(u32::MAX)
    }

    /// Complete cells per grid row on a sheet `width` px wide.
    pub fn columns(&self, width: u32) -> u32 {
        Self::count(width, self.offset_x, self.cell_width, self.spacing_x)
    }

    /// Complete grid rows on a sheet `height` px tall.
    pub fn rows(&self, height: u32) -> u32 {
        Self::count(height, self.offset_y, self.cell_height, self.spacing_y)
    }

    /// Sheet coordinates of the top-left pixel of cell `(col, row)`.
    fn origin(&self, col: u32, row: u32) -> (usize, usize) {
        let x =
            u64::from(self.offset_x) + u64::from(col) * u64::from(self.cell_width + self.spacing_x);
        let y = u64::from(self.offset_y)
            + u64::from(row) * u64::from(self.cell_height + self.spacing_y);
        (x as usize, y as usize)
    }
}

/// One grid row that yields frames: its cells `0..used` are the used cells
/// (spec AC-05), so `used` counts frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedRow {
    /// 0-based row index in the grid.
    pub row: u32,
    /// Number of used cells, from the row's first cell on.
    pub used: u32,
}

/// What an import of a sheet along a grid creates: the rows that yield
/// frames, top to bottom. Rows without a used cell are left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPlan {
    /// Grid rows with at least one used cell, in grid order.
    pub rows: Vec<PlannedRow>,
}

impl SheetPlan {
    /// Number of frames the import creates.
    pub fn frame_count(&self) -> u32 {
        self.rows.iter().map(|r| r.used).sum()
    }

    /// One Forward tag per planned row, named `"Row k"` with `k` the
    /// row's 1-based grid index, spanning that row's frames (spec AC-06).
    pub fn row_tags(&self) -> Vec<Tag> {
        let mut from = 0u32;
        self.rows
            .iter()
            .map(|r| {
                let tag = Tag::new(
                    format!("Row {}", r.row + 1),
                    FrameIndex::new(from),
                    FrameIndex::new(from + r.used - 1),
                );
                from += r.used;
                tag
            })
            .collect()
    }
}

/// A decoded sprite sheet, straight-alpha RGBA8, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// A zeroed buffer of `len` bytes, or [`SheetError::OutOfMemory`].
fn zeroed(len: usize) -> Result<Vec<u8>, SheetError> {
    let mut v = Vec::new();
    v.try_reserve_exact(len)
        .map_err(|_| SheetError::OutOfMemory)?;
    v.resize(len, 0);
    Ok(v)
}

/// `true` when a sheet of `width × height` is within the import limits.
fn fits(width: u32, height: u32) -> bool {
    width <= MAX_SHEET_SIDE
        && height <= MAX_SHEET_SIDE
        && u64::from(width) * u64::from(height) <= MAX_SHEET_PIXELS
}

/// The width and height from a PNG's IHDR chunk, read without decoding.
fn png_header_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let be =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Some((be(16), be(20)))
}

/// `true` when `px` is fully transparent after the background step.
#[inline]
fn clear(px: &[u8], background: Option<[u8; 4]>) -> bool {
    px[3] == 0 || background.is_some_and(|bg| px == bg)
}

impl Sheet {
    /// Decode PNG bytes (any colour type and bit depth, of an APNG the
    /// default image) into a sheet.
    ///
    /// # Errors
    ///
    /// [`SheetError::TooLarge`] past the size limits (checked on the
    /// header, before decoding), [`SheetError::OutOfMemory`] when the
    /// decoded image does not fit in memory, [`SheetError::Decode`] for
    /// anything that is not a readable PNG.
    pub fn decode(bytes: &[u8]) -> Result<Self, SheetError> {
        if let Some((width, height)) = png_header_size(bytes)
            && !fits(width, height)
        {
            return Err(SheetError::TooLarge { width, height });
        }
        let img = import_png(bytes).map_err(|e| match e {
            ImportError::OutOfMemory => SheetError::OutOfMemory,
            other => SheetError::Decode(other.to_string()),
        })?;
        Self::from_rgba(img.width, img.height, img.rgba)
    }

    /// A sheet from raw RGBA8 pixels.
    ///
    /// # Errors
    ///
    /// [`SheetError::TooLarge`] past the size limits,
    /// [`SheetError::Decode`] when `rgba` does not hold `width × height`
    /// pixels or a side is zero.
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, SheetError> {
        if !fits(width, height) {
            return Err(SheetError::TooLarge { width, height });
        }
        if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
            return Err(SheetError::Decode(format!(
                "{} bytes for {width}x{height} pixels",
                rgba.len()
            )));
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    /// Sheet width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Sheet height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The decoded pixels, RGBA8 row-major.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// The RGBA of the pixel at `(x, y)` (inside the sheet).
    fn pixel(&self, x: usize, y: usize) -> &[u8] {
        let at = (y * self.width as usize + x) * 4;
        &self.rgba[at..at + 4]
    }

    /// The sheet's top-left pixel, the background colour the dialog
    /// starts with (spec AC-07).
    pub fn top_left(&self) -> [u8; 4] {
        [self.rgba[0], self.rgba[1], self.rgba[2], self.rgba[3]]
    }

    /// The sheet resampled to `width × height` with nearest-neighbour, as
    /// RGBA8 (the dialog's preview). Empty for a zero side.
    ///
    /// # Errors
    ///
    /// [`SheetError::OutOfMemory`] when the buffer cannot be allocated.
    pub fn preview(&self, width: u32, height: u32) -> Result<Vec<u8>, SheetError> {
        let mut out = zeroed(width as usize * height as usize * 4)?;
        if width == 0 || height == 0 {
            return Ok(out);
        }
        let (sw, sh) = (u64::from(self.width), u64::from(self.height));
        for (y, row) in out.chunks_exact_mut(width as usize * 4).enumerate() {
            let sy = (y as u64 * sh / u64::from(height)) as usize;
            for (x, px) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let sx = (x as u64 * sw / u64::from(width)) as usize;
                px.copy_from_slice(self.pixel(sx, sy));
            }
        }
        Ok(out)
    }

    /// `true` when every pixel of cell `(col, row)` is clear.
    fn cell_empty(
        &self,
        grid: &SheetGrid,
        col: u32,
        row: u32,
        background: Option<[u8; 4]>,
    ) -> bool {
        let (x0, y0) = grid.origin(col, row);
        let (w, h) = (grid.cell_width as usize, grid.cell_height as usize);
        (y0..y0 + h).all(|y| {
            let at = (y * self.width as usize + x0) * 4;
            self.rgba[at..at + w * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .all(|px| clear(px, background))
        })
    }

    /// Check `grid` against the sheet and work out the frames it yields
    /// (spec AC-02, AC-05, AC-12).
    ///
    /// # Errors
    ///
    /// The first [`GridError`] that applies.
    pub fn plan(
        &self,
        grid: &SheetGrid,
        background: Option<[u8; 4]>,
    ) -> Result<SheetPlan, GridError> {
        if !(1..=MAX_SHEET_CELL).contains(&grid.cell_width) {
            return Err(GridError::CellWidth);
        }
        if !(1..=MAX_SHEET_CELL).contains(&grid.cell_height) {
            return Err(GridError::CellHeight);
        }
        if grid.offset_x >= self.width {
            return Err(GridError::OffsetX);
        }
        if grid.offset_y >= self.height {
            return Err(GridError::OffsetY);
        }
        if grid.spacing_x > self.width {
            return Err(GridError::SpacingX);
        }
        if grid.spacing_y > self.height {
            return Err(GridError::SpacingY);
        }
        let (cols, rows) = (grid.columns(self.width), grid.rows(self.height));
        if cols == 0 || rows == 0 {
            return Err(GridError::NoCell);
        }
        let mut planned = Vec::new();
        let mut frames = 0u32;
        for row in 0..rows {
            let Some(last) = (0..cols)
                .rev()
                .find(|&col| !self.cell_empty(grid, col, row, background))
            else {
                continue;
            };
            frames = frames.saturating_add(last + 1);
            if frames > MAX_SHEET_FRAMES {
                return Err(GridError::TooManyFrames);
            }
            planned.push(PlannedRow {
                row,
                used: last + 1,
            });
        }
        if planned.is_empty() {
            return Err(GridError::AllEmpty);
        }
        Ok(SheetPlan { rows: planned })
    }

    /// Cut the used cells of `plan` into frames, row-major, each holding
    /// exactly its cell's pixels after the background step (spec AC-04,
    /// AC-07).
    ///
    /// # Errors
    ///
    /// [`SheetError::OutOfMemory`] when the frames do not fit in memory.
    pub fn cut(
        &self,
        grid: &SheetGrid,
        plan: &SheetPlan,
        background: Option<[u8; 4]>,
    ) -> Result<Vec<PixelBuffer>, SheetError> {
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(plan.frame_count() as usize)
            .map_err(|_| SheetError::OutOfMemory)?;
        let (w, h) = (grid.cell_width as usize, grid.cell_height as usize);
        for planned in &plan.rows {
            for col in 0..planned.used {
                let (x0, y0) = grid.origin(col, planned.row);
                let mut data = zeroed(w * h * 4)?;
                for (y, dst) in data.chunks_exact_mut(w * 4).enumerate() {
                    let at = ((y0 + y) * self.width as usize + x0) * 4;
                    dst.copy_from_slice(&self.rgba[at..at + w * 4]);
                    for px in dst.as_chunks_mut::<4>().0 {
                        if clear(px, background) {
                            *px = [0; 4];
                        }
                    }
                }
                frames.push(PixelBuffer {
                    width: grid.cell_width,
                    height: grid.cell_height,
                    color_mode: ColorMode::Rgba,
                    data,
                });
            }
        }
        Ok(frames)
    }

    /// The grid suggestion for the sheet after the background step, or
    /// `None` when none is found (spec AC-09, AC-18).
    pub fn suggest_grid(&self, background: Option<[u8; 4]>) -> Option<SheetGrid> {
        suggest::suggest(self, background)
    }
}
