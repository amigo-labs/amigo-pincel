//! Tests for the sprite-sheet import (docs/specs/sprite-sheet-import.md).

use super::*;
use crate::codec::{AtlasOptions, export_atlas_png};
use crate::document::{Cel, CelMap, Frame, Layer, LayerId, Sprite, TagDirection};

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

/// A `w × h` sheet whose pixel at `(x, y)` is `f(x, y)`.
fn sheet_from(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Sheet {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&f(x, y));
        }
    }
    Sheet::from_rgba(w, h, rgba).expect("valid sheet")
}

/// A sheet of `cols × rows` cells of `cell × cell` px where `on(col, row)`
/// cells hold a one-pixel sprite `color(col, row)` at `(1, 1)` in the cell.
fn dotted(
    cell: u32,
    cols: u32,
    rows: u32,
    on: impl Fn(u32, u32) -> bool,
    color: impl Fn(u32, u32) -> [u8; 4],
) -> Sheet {
    sheet_from(cell * cols, cell * rows, |x, y| {
        let (c, r) = (x / cell, y / cell);
        if on(c, r) && x % cell == 1 && y % cell == 1 {
            color(c, r)
        } else {
            [0; 4]
        }
    })
}

#[test]
fn grid_columns_and_rows_count_only_complete_cells() {
    // AC-02: 10 px wide, offset 1, cells 3 wide, spacing 1 → cells at
    // x = 1, 5 (the one at 9 would end at 11 > 10).
    let grid = SheetGrid {
        cell_width: 3,
        cell_height: 2,
        offset_x: 1,
        offset_y: 0,
        spacing_x: 1,
        spacing_y: 2,
    };
    assert_eq!(grid.columns(10), 2);
    assert_eq!(grid.columns(12), 3);
    assert_eq!(grid.rows(6), 2, "rows at y = 0 and 4");
    assert_eq!(grid.rows(5), 1);
    assert_eq!(SheetGrid::new(4, 4).columns(3), 0);
}

#[test]
fn cut_offset_and_spacing_grid_frames_hold_their_cell_pixels() {
    // AC-02, AC-04: cells 2×2 at offset (1, 1), spacing (1, 1), on a 7×7
    // sheet → 2×2 complete cells. Pixel value encodes its coordinates.
    let sheet = sheet_from(7, 7, |x, y| [x as u8, y as u8, 7, 255]);
    let grid = SheetGrid {
        cell_width: 2,
        cell_height: 2,
        offset_x: 1,
        offset_y: 1,
        spacing_x: 1,
        spacing_y: 1,
    };
    let plan = sheet.plan(&grid, None).expect("plan");
    assert_eq!(plan.frame_count(), 4);
    let frames = sheet.cut(&grid, &plan, None).expect("cut");
    let origins = [(1, 1), (4, 1), (1, 4), (4, 4)];
    for (frame, (ox, oy)) in frames.iter().zip(origins) {
        assert_eq!((frame.width, frame.height), (2, 2));
        let expected: Vec<u8> = [(0, 0), (1, 0), (0, 1), (1, 1)]
            .iter()
            .flat_map(|&(dx, dy)| [(ox + dx) as u8, (oy + dy) as u8, 7, 255])
            .collect();
        assert_eq!(
            frame.data, expected,
            "frame at ({ox}, {oy}) in row-major order"
        );
    }
}

#[test]
fn plan_skips_trailing_empty_cells_and_empty_rows_keeps_inner_gaps() {
    // AC-05: row 0 = [x . x .], row 1 empty, row 2 = [x . . .].
    let on = |c: u32, r: u32| matches!((c, r), (0, 0) | (2, 0) | (0, 2));
    let sheet = dotted(4, 4, 3, on, |_, _| RED);
    let plan = sheet.plan(&SheetGrid::new(4, 4), None).expect("plan");
    assert_eq!(
        plan.rows,
        vec![
            PlannedRow { row: 0, used: 3 },
            PlannedRow { row: 2, used: 1 }
        ]
    );
    let frames = sheet.cut(&SheetGrid::new(4, 4), &plan, None).expect("cut");
    assert_eq!(frames.len(), 4);
    assert!(
        frames[1].data.iter().all(|&b| b == 0),
        "inner gap is an empty frame"
    );
    assert!(frames[2].data.as_chunks::<4>().0.contains(&RED));
}

#[test]
fn row_tags_name_rows_by_grid_index_and_span_their_frames() {
    // AC-06: rows 0 and 2 used (row 1 empty) → "Row 1" 0..=2, "Row 3" 3..=3.
    let plan = SheetPlan {
        rows: vec![
            PlannedRow { row: 0, used: 3 },
            PlannedRow { row: 2, used: 1 },
        ],
    };
    let tags = plan.row_tags();
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "Row 1");
    assert_eq!(
        (tags[0].from, tags[0].to),
        (FrameIndex::new(0), FrameIndex::new(2))
    );
    assert_eq!(tags[1].name, "Row 3");
    assert_eq!(
        (tags[1].from, tags[1].to),
        (FrameIndex::new(3), FrameIndex::new(3))
    );
    assert!(tags.iter().all(|t| t.direction == TagDirection::Forward));
}

#[test]
fn background_exact_match_turns_transparent_before_cutting() {
    // AC-07: a white sheet with red dots in cells (0,0) and (1,0); cell
    // (2,0) holds an off-white pixel that must survive.
    let sheet = sheet_from(12, 4, |x, y| match (x, y) {
        (1, 1) | (5, 1) => RED,
        (9, 1) => [255, 255, 254, 255],
        _ => WHITE,
    });
    assert_eq!(sheet.top_left(), WHITE);
    let grid = SheetGrid::new(4, 4);
    let without = sheet.plan(&grid, None).expect("plan");
    assert_eq!(without.frame_count(), 3);
    let plan = sheet.plan(&grid, Some(WHITE)).expect("plan");
    assert_eq!(plan.frame_count(), 3, "the off-white cell is not empty");
    let frames = sheet.cut(&grid, &plan, Some(WHITE)).expect("cut");
    let px = |f: &PixelBuffer, x: usize, y: usize| f.data[(y * 4 + x) * 4..][..4].to_vec();
    assert_eq!(
        px(&frames[0], 0, 0),
        vec![0, 0, 0, 0],
        "white became transparent"
    );
    assert_eq!(px(&frames[0], 1, 1), RED.to_vec());
    assert_eq!(
        px(&frames[2], 1, 1),
        vec![255, 255, 254, 255],
        "exact match only"
    );
    // A semi-transparent white is not the opaque background colour.
    let soft = sheet_from(4, 4, |_, _| [255, 255, 255, 128]);
    assert!(soft.plan(&grid, Some(WHITE)).is_ok());
    assert_eq!(
        soft.plan(&grid, Some([255, 255, 255, 128])),
        Err(GridError::AllEmpty)
    );
}

#[test]
fn plan_reports_each_grid_error() {
    // AC-12.
    let sheet = dotted(4, 2, 2, |_, _| true, |_, _| RED);
    let base = SheetGrid::new(4, 4);
    let with = |f: fn(&mut SheetGrid)| {
        let mut g = base;
        f(&mut g);
        sheet.plan(&g, None)
    };
    assert_eq!(with(|g| g.cell_width = 0), Err(GridError::CellWidth));
    assert_eq!(with(|g| g.cell_width = 4097), Err(GridError::CellWidth));
    assert_eq!(with(|g| g.cell_height = 0), Err(GridError::CellHeight));
    assert_eq!(with(|g| g.offset_x = 8), Err(GridError::OffsetX));
    assert_eq!(with(|g| g.offset_y = 8), Err(GridError::OffsetY));
    assert_eq!(with(|g| g.spacing_x = 9), Err(GridError::SpacingX));
    assert_eq!(with(|g| g.spacing_y = 9), Err(GridError::SpacingY));
    assert_eq!(with(|g| g.cell_width = 9), Err(GridError::NoCell));
    assert_eq!(with(|g| g.offset_x = 7), Err(GridError::NoCell));
    assert!(
        with(|g| g.spacing_x = 8).is_ok(),
        "spacing up to the width is allowed"
    );
    let empty = sheet_from(8, 8, |_, _| [0; 4]);
    assert_eq!(empty.plan(&base, None), Err(GridError::AllEmpty));
    // 1×1 cells on an opaque 257×256 sheet: 65 792 frames.
    let big = sheet_from(257, 256, |_, _| RED);
    assert_eq!(
        big.plan(&SheetGrid::new(1, 1), None),
        Err(GridError::TooManyFrames)
    );
    let max = sheet_from(255, 257, |_, _| RED);
    assert_eq!(
        max.plan(&SheetGrid::new(1, 1), None)
            .map(|p| p.frame_count()),
        Ok(65_535)
    );
    assert_eq!(GridError::TooManyFrames.code(), "too-many-frames");
}

/// A minimal PNG signature + IHDR claiming `w × h` (no image data).
fn png_header(w: u32, h: u32) -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b
}

#[test]
fn decode_rejects_garbage_oversized_sheets_and_reports_out_of_memory() {
    // AC-13.
    assert!(matches!(
        Sheet::decode(&[1, 2, 3]),
        Err(SheetError::Decode(_))
    ));
    assert!(matches!(
        Sheet::decode(&png_header(4, 4)),
        Err(SheetError::Decode(_))
    ));
    assert_eq!(
        Sheet::decode(&png_header(16_385, 1)),
        Err(SheetError::TooLarge {
            width: 16_385,
            height: 1
        })
    );
    assert_eq!(
        Sheet::decode(&png_header(8193, 8193)),
        Err(SheetError::TooLarge {
            width: 8193,
            height: 8193
        })
    );
    assert!(
        SheetError::TooLarge {
            width: 1,
            height: 1
        }
        .to_string()
        .contains("16384")
    );
    assert_eq!(zeroed(usize::MAX), Err(SheetError::OutOfMemory));
}

#[test]
fn decode_reads_a_png_sheet() {
    let rgba: Vec<u8> = [RED, BLUE, WHITE, [0; 4]].concat();
    let png = crate::codec::encode_png(&rgba, 2, 2).expect("encode");
    let sheet = Sheet::decode(&png).expect("decode");
    assert_eq!((sheet.width(), sheet.height()), (2, 2));
    assert_eq!(sheet.rgba(), rgba.as_slice());
}

#[test]
fn preview_samples_nearest_neighbour() {
    let sheet = sheet_from(2, 1, |x, _| if x == 0 { RED } else { BLUE });
    assert_eq!(
        sheet.preview(4, 2).expect("preview"),
        [RED, RED, BLUE, BLUE, RED, RED, BLUE, BLUE].concat()
    );
    assert_eq!(sheet.preview(1, 1).expect("preview"), RED.to_vec());
}

/// A deterministic pseudo-random sequence (no dependency).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: u32) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % u64::from(below)) as u32
    }
}

#[test]
fn suggest_grid_finds_the_grid_of_aligned_sprites() {
    // AC-09: 300 random sheets of P × Q cells at offset (X, Y), spacing 0,
    // each sprite the same box inside its cell with a clear margin, some
    // cells empty, at least one horizontal and one vertical neighbour pair.
    let mut rng = Lcg(7);
    for case in 0..300 {
        let (p, q) = (3 + rng.next(14), 3 + rng.next(14));
        let (ox, oy) = (rng.next(6), rng.next(6));
        // Sprite box inside the cell, leaving at least one clear column /
        // row between neighbours.
        let (bx, by) = (rng.next(p - 1), rng.next(q - 1));
        let bw = 1 + rng.next(p - 1 - bx);
        let bh = 1 + rng.next(q - 1 - by);
        let (cols, rows) = (2 + rng.next(4), 2 + rng.next(4));
        let mut used = vec![vec![false; cols as usize]; rows as usize];
        for row in used.iter_mut() {
            for cell in row.iter_mut() {
                *cell = rng.next(3) > 0;
            }
        }
        used[0][0] = true;
        used[0][1] = true;
        used[1][0] = true;
        let tail = (rng.next(5), rng.next(5));
        let (w, h) = (ox + cols * p + tail.0, oy + rows * q + tail.1);
        let sheet = sheet_from(w, h, |x, y| {
            if x < ox || y < oy {
                return [0; 4];
            }
            let (c, r) = ((x - ox) / p, (y - oy) / q);
            let (ix, iy) = ((x - ox) % p, (y - oy) % q);
            let inside = c < cols
                && r < rows
                && used[r as usize][c as usize]
                && (bx..bx + bw).contains(&ix)
                && (by..by + bh).contains(&iy);
            if inside {
                [(c * 40) as u8, (r * 40) as u8, 9, 255]
            } else {
                [0; 4]
            }
        });
        let g = sheet
            .suggest_grid(None)
            .unwrap_or_else(|| panic!("case {case}: no suggestion for P {p} Q {q} X {ox} Y {oy}"));
        assert_eq!((g.cell_width, g.cell_height), (p, q), "case {case}");
        assert_eq!((g.spacing_x, g.spacing_y), (0, 0), "case {case}");
        // Every sprite lies inside one complete cell of the suggested grid.
        let plan = sheet.plan(&g, None).expect("plan");
        let sprites = used.iter().flatten().filter(|&&u| u).count();
        let frames = sheet.cut(&g, &plan, None).expect("cut");
        let full = frames
            .iter()
            .filter(|f| {
                f.data
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|px| px[3] > 0)
                    .count()
                    == (bw * bh) as usize
            })
            .count();
        assert_eq!(
            full, sprites,
            "case {case}: every sprite whole in one frame"
        );
    }
}

#[test]
fn suggest_grid_works_after_the_background_step() {
    // AC-09 with an opaque background removed first (AC-07).
    let sheet = sheet_from(
        16,
        16,
        |x, y| if x % 8 < 3 && y % 8 < 3 { RED } else { WHITE },
    );
    assert_eq!(
        sheet.suggest_grid(None),
        None,
        "no clear column without the background step"
    );
    let g = sheet.suggest_grid(Some(WHITE)).expect("suggestion");
    assert_eq!(
        (g.cell_width, g.cell_height, g.offset_x, g.offset_y),
        (8, 8, 0, 0)
    );
}

#[test]
fn suggest_grid_gives_none_for_blank_single_and_gapless_sheets() {
    // AC-18.
    assert_eq!(
        sheet_from(8, 8, |_, _| [0; 4]).suggest_grid(None),
        None,
        "fully transparent"
    );
    let single = sheet_from(8, 8, |x, y| {
        if (2..5).contains(&x) && (2..5).contains(&y) {
            RED
        } else {
            [0; 4]
        }
    });
    assert_eq!(single.suggest_grid(None), None, "single sprite");
    let no_clear_col = sheet_from(8, 8, |_, y| if y % 4 == 1 { RED } else { [0; 4] });
    assert_eq!(
        no_clear_col.suggest_grid(None),
        None,
        "no transparent column"
    );
    let no_clear_row = sheet_from(8, 8, |x, _| if x % 4 == 1 { RED } else { [0; 4] });
    assert_eq!(no_clear_row.suggest_grid(None), None, "no transparent row");
}

#[test]
fn exported_sheet_reimports_to_the_same_frames() {
    // AC-16: 5 frames of 3×2 exported in 3 columns (last row has a gap),
    // imported with cell = canvas, offset 0, spacing 0, no background.
    let (w, h, n) = (3u32, 2u32, 5u32);
    let mut builder = Sprite::builder(w, h).add_layer(Layer::image(LayerId::new(0), "art"));
    for _ in 0..n {
        builder = builder.add_frame(Frame::new(100));
    }
    let sprite = builder.build().expect("sprite");
    let mut cels = CelMap::new();
    let mut expected = Vec::new();
    for i in 0..n {
        let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
        for (j, px) in buf.data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            // Every frame has opaque pixels, and pixel (i % 6) is clear.
            if j as u32 != i % 6 {
                *px = [(i * 50) as u8, j as u8 * 30, 200, 255];
            }
        }
        expected.push(buf.data.clone());
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(i), buf));
    }
    let atlas = export_atlas_png(&sprite, &cels, &AtlasOptions::new(3)).expect("export");
    let sheet = Sheet::decode(&atlas.png).expect("decode");
    let grid = SheetGrid::new(w, h);
    let plan = sheet.plan(&grid, None).expect("plan");
    let frames = sheet.cut(&grid, &plan, None).expect("cut");
    let got: Vec<Vec<u8>> = frames.into_iter().map(|f| f.data).collect();
    assert_eq!(got, expected);
}
