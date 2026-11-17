//! Sprite-sheet import surface (docs/specs/sprite-sheet-import.md): a
//! decoded PNG sheet the Import sprite sheet dialog previews, plans and
//! cuts into a new document.

use pincel_core::{
    Cel, CelMap, ColorMode, Frame, FrameIndex, GridError, Layer, Palette, Sheet, SheetError,
    SheetGrid, Sprite,
};
use wasm_bindgen::prelude::*;

use crate::{DEFAULT_LAYER_ID, Document};

/// A decoded PNG sprite sheet, held in Rust memory while the dialog is
/// open. Colours cross the boundary packed `0xRRGGBBAA`; `background`
/// arguments are `undefined` for "Make background transparent" off.
#[wasm_bindgen]
pub struct SpriteSheet {
    sheet: Sheet,
}

/// What a grid would import: frame and tag count, or why it cannot.
#[wasm_bindgen]
pub struct SheetSummary {
    frames: u32,
    tags: u32,
    error: Option<String>,
}

#[wasm_bindgen]
impl SheetSummary {
    /// Frames the import creates (0 with an error).
    #[wasm_bindgen(getter)]
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Tags the import creates (0 with an error or "Rows as tags" off).
    #[wasm_bindgen(getter)]
    pub fn tags(&self) -> u32 {
        self.tags
    }

    /// Why Import is disabled, as a stable code (`cell-width`,
    /// `cell-height`, `offset-x`, `offset-y`, `spacing-x`, `spacing-y`,
    /// `no-cell`, `all-empty`, `too-many-frames`), or `undefined`.
    #[wasm_bindgen(getter)]
    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }
}

/// Unpack `0xRRGGBBAA`.
fn rgba(color: u32) -> [u8; 4] {
    color.to_be_bytes()
}

/// The grid from its six fields `[cellW, cellH, offsetX, offsetY,
/// spacingX, spacingY]`; `None` for another length.
fn grid(fields: &[u32]) -> Option<SheetGrid> {
    let &[
        cell_width,
        cell_height,
        offset_x,
        offset_y,
        spacing_x,
        spacing_y,
    ] = fields
    else {
        return None;
    };
    Some(SheetGrid {
        cell_width,
        cell_height,
        offset_x,
        offset_y,
        spacing_x,
        spacing_y,
    })
}

/// Error code for a grid that is not six fields long.
const GRID_FIELDS: &str = "grid-fields";

#[wasm_bindgen]
impl SpriteSheet {
    /// Decode PNG bytes into a sheet. Errors are stable codes:
    /// `too-large` (past 16384 px per side or 67 108 864 pixels),
    /// `out-of-memory`, or `decode: <reason>`.
    pub fn decode(bytes: &[u8]) -> Result<SpriteSheet, String> {
        Sheet::decode(bytes)
            .map(|sheet| SpriteSheet { sheet })
            .map_err(|e| match e {
                SheetError::TooLarge { .. } => "too-large".to_owned(),
                SheetError::OutOfMemory => "out-of-memory".to_owned(),
                SheetError::Decode(reason) => format!("decode: {reason}"),
            })
    }

    /// Sheet width in pixels.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.sheet.width()
    }

    /// Sheet height in pixels.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.sheet.height()
    }

    /// The top-left pixel, `0xRRGGBBAA` (the initial background colour).
    #[wasm_bindgen(js_name = topLeft)]
    pub fn top_left(&self) -> u32 {
        u32::from_be_bytes(self.sheet.top_left())
    }

    /// The sheet resampled to `width × height` with nearest-neighbour,
    /// RGBA8, for the dialog's preview canvas.
    pub fn preview(&self, width: u32, height: u32) -> Result<Box<[u8]>, String> {
        self.sheet
            .preview(width, height)
            .map(Vec::into_boxed_slice)
            .map_err(|e| e.to_string())
    }

    /// The suggested grid `[cellW, cellH, offsetX, offsetY, spacingX,
    /// spacingY]` for the sheet after the background step, or `undefined`.
    pub fn suggest(&self, background: Option<u32>) -> Option<Box<[u32]>> {
        let g = self.sheet.suggest_grid(background.map(rgba))?;
        Some(Box::new([
            g.cell_width,
            g.cell_height,
            g.offset_x,
            g.offset_y,
            g.spacing_x,
            g.spacing_y,
        ]))
    }

    /// What importing along `grid` (`[cellW, cellH, offsetX, offsetY,
    /// spacingX, spacingY]`) creates: the dialog's summary.
    pub fn summary(
        &self,
        grid: &[u32],
        background: Option<u32>,
        rows_as_tags: bool,
    ) -> SheetSummary {
        let Some(g) = self::grid(grid) else {
            return SheetSummary {
                frames: 0,
                tags: 0,
                error: Some(GRID_FIELDS.to_owned()),
            };
        };
        match self.sheet.plan(&g, background.map(rgba)) {
            Ok(plan) => SheetSummary {
                frames: plan.frame_count(),
                tags: if rows_as_tags {
                    plan.rows.len() as u32
                } else {
                    0
                },
                error: None,
            },
            Err(e) => SheetSummary {
                frames: 0,
                tags: 0,
                error: Some(GridError::code(e).to_owned()),
            },
        }
    }

    /// Cut the sheet along `grid` (as in [`SpriteSheet::summary`]) into a
    /// new document: canvas = cell size, one image
    /// layer "Layer 1", one `duration_ms` frame per used cell in row-major
    /// order, one "Row k" tag per row when `rows_as_tags`, RGBA with the
    /// palette of File ▸ New. Errors: a grid error code (see
    /// [`SheetSummary::error`]), `duration` for a zero duration, or
    /// `out-of-memory`.
    pub fn build(
        &self,
        grid: &[u32],
        background: Option<u32>,
        duration_ms: u16,
        rows_as_tags: bool,
    ) -> Result<Document, String> {
        if duration_ms == 0 {
            return Err("duration".to_owned());
        }
        let g = self::grid(grid).ok_or_else(|| GRID_FIELDS.to_owned())?;
        let bg = background.map(rgba);
        let plan = self
            .sheet
            .plan(&g, bg)
            .map_err(|e| GridError::code(e).to_owned())?;
        let frames = self
            .sheet
            .cut(&g, &plan, bg)
            .map_err(|_| "out-of-memory".to_owned())?;
        let mut builder = Sprite::builder(g.cell_width, g.cell_height)
            .color_mode(ColorMode::Rgba)
            .add_layer(Layer::image(DEFAULT_LAYER_ID, "Layer 1"))
            .palette(Palette::db32());
        for _ in 0..frames.len() {
            builder = builder.add_frame(Frame::new(duration_ms));
        }
        if rows_as_tags {
            for tag in plan.row_tags() {
                builder = builder.add_tag(tag);
            }
        }
        let sprite = builder
            .build()
            .map_err(|e| format!("failed to build sprite: {e}"))?;
        let mut cels = CelMap::new();
        for (i, buf) in frames.into_iter().enumerate() {
            cels.insert(Cel::image(DEFAULT_LAYER_ID, FrameIndex::new(i as u32), buf));
        }
        Ok(Document::from_parts(sprite, cels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: u32 = 0xFF0000FF;
    const WHITE: u32 = 0xFFFFFFFF;

    /// PNG bytes of a `w × h` sheet whose pixel `(x, y)` is `f(x, y)`.
    fn png(w: u32, h: u32, f: impl Fn(u32, u32) -> u32) -> Vec<u8> {
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&f(x, y).to_be_bytes());
            }
        }
        pincel_core::encode_png(&rgba, w, h).expect("encode")
    }

    /// A white 12×8 sheet of 4×4 cells: row 0 holds dots in cells 0 and
    /// 1, row 1 a dot in cell 0 only; the dot in cell (c, r) has red
    /// `10 + c + 10 r` at (1, 1) in its cell.
    fn white_sheet() -> SpriteSheet {
        let bytes = png(12, 8, |x, y| match (x, y) {
            (1, 1) => 0x0A0000FF,
            (5, 1) => 0x0B0000FF,
            (1, 5) => 0x140000FF,
            _ => WHITE,
        });
        SpriteSheet::decode(&bytes).expect("decode")
    }

    #[test]
    fn build_creates_one_frame_per_used_cell_with_its_pixels() {
        // AC-04, AC-05, AC-07, AC-08, AC-15.
        let sheet = white_sheet();
        assert_eq!((sheet.width(), sheet.height()), (12, 8));
        assert_eq!(sheet.top_left(), WHITE);
        let doc = sheet
            .build(&[4, 4, 0, 0, 0, 0], Some(WHITE), 250, true)
            .expect("build");
        assert_eq!((doc.width(), doc.height()), (4, 4));
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.layer_name(0), "Layer 1");
        assert_eq!(doc.frame_count(), 3);
        for (frame, red) in [(0, 0x0A), (1, 0x0B), (2, 0x14)] {
            assert_eq!(
                doc.pick_color(frame, 1, 1).expect("pick"),
                (red << 24) | 0xFF
            );
            assert_eq!(
                doc.pick_color(frame, 0, 0).expect("pick"),
                0,
                "background cleared"
            );
            assert_eq!(doc.frame_duration(frame).expect("duration"), 250);
        }
        assert_eq!(doc.sprite.color_mode, ColorMode::Rgba);
        assert_eq!(doc.sprite.palette, Palette::db32());
        assert_eq!(doc.undo_depth(), 0);
    }

    #[test]
    fn build_names_one_forward_tag_per_row_or_none() {
        // AC-06.
        let sheet = white_sheet();
        let doc = sheet
            .build(&[4, 4, 0, 0, 0, 0], Some(WHITE), 100, true)
            .expect("build");
        assert_eq!(doc.tag_count(), 2);
        assert_eq!(doc.tag_name(0).expect("tag"), "Row 1");
        assert_eq!(
            (doc.tag_from(0).expect("from"), doc.tag_to(0).expect("to")),
            (0, 1)
        );
        assert_eq!(doc.tag_name(1).expect("tag"), "Row 2");
        assert_eq!(
            (doc.tag_from(1).expect("from"), doc.tag_to(1).expect("to")),
            (2, 2)
        );
        assert_eq!(doc.tag_direction(0).expect("dir"), "forward");
        let untagged = sheet
            .build(&[4, 4, 0, 0, 0, 0], Some(WHITE), 100, false)
            .expect("build");
        assert_eq!(untagged.tag_count(), 0);
    }

    #[test]
    fn summary_counts_frames_and_tags_or_names_the_error() {
        // AC-03, AC-12.
        let sheet = white_sheet();
        let s = sheet.summary(&[4, 4, 0, 0, 0, 0], Some(WHITE), true);
        assert_eq!((s.frames(), s.tags(), s.error()), (3, 2, None));
        let s = sheet.summary(&[4, 4, 0, 0, 0, 0], Some(WHITE), false);
        assert_eq!((s.frames(), s.tags()), (3, 0));
        let s = sheet.summary(&[4, 4, 0, 0, 0, 0], None, true);
        assert_eq!(
            s.frames(),
            6,
            "without the background step every cell is used"
        );
        let s = sheet.summary(&[13, 4, 0, 0, 0, 0], None, true);
        assert_eq!(s.error().as_deref(), Some("no-cell"));
        let s = sheet.summary(&[0, 4, 0, 0, 0, 0], None, true);
        assert_eq!(s.error().as_deref(), Some("cell-width"));
        let s = sheet.summary(&[4, 4], None, true);
        assert_eq!(s.error().as_deref(), Some("grid-fields"));
        assert_eq!(
            sheet
                .build(&[13, 4, 0, 0, 0, 0], None, 100, true)
                .err()
                .as_deref(),
            Some("no-cell")
        );
        assert_eq!(
            sheet
                .build(&[4, 4, 0, 0, 0, 0], None, 0, true)
                .err()
                .as_deref(),
            Some("duration")
        );
    }

    #[test]
    fn suggest_reports_the_grid_for_the_background_setting() {
        // AC-09, AC-11.
        let sheet = white_sheet();
        assert_eq!(sheet.suggest(None), None);
        let g = sheet.suggest(Some(WHITE)).expect("suggestion");
        assert_eq!(&*g, &[4, 4, 0, 0, 0, 0]);
    }

    #[test]
    fn decode_maps_errors_to_codes_and_preview_resamples() {
        // AC-13, AC-01.
        assert!(
            SpriteSheet::decode(&[1, 2, 3])
                .err()
                .expect("err")
                .starts_with("decode:")
        );
        let mut big = png(1, 1, |_, _| RED);
        // Patch the IHDR width to 16385 (CRC is not checked before the limit).
        big[16..20].copy_from_slice(&16_385u32.to_be_bytes());
        assert_eq!(
            SpriteSheet::decode(&big).err().as_deref(),
            Some("too-large")
        );
        let sheet = SpriteSheet::decode(&png(2, 1, |x, _| if x == 0 { RED } else { WHITE }))
            .expect("decode");
        let px = sheet.preview(4, 1).expect("preview");
        assert_eq!(
            &px[..],
            &[[255, 0, 0, 255], [255, 0, 0, 255], [255; 4], [255; 4]].concat()[..]
        );
    }
}
