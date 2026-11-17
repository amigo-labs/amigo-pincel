//! Grid suggestion for a sprite sheet (docs/specs/sprite-sheet-import.md
//! AC-09, AC-18).
//!
//! Sprites in a grid with spacing 0 and at least one clear column / row
//! between neighbours show up as runs of occupied columns and rows. The
//! cell size along an axis is the smallest distance between the starts of
//! two consecutive runs (two sprites in adjacent cells); the offset is the
//! smallest one at which every run lies inside one complete cell. An axis
//! with a single run (a one-row strip) takes the whole sheet as its cell
//! when the other axis has a period.

use super::{MAX_SHEET_CELL, Sheet, SheetGrid, clear};

/// Occupied `[start, end]` runs (inclusive) of an occupancy line.
fn runs(occupied: &[bool]) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &on) in occupied.iter().enumerate() {
        match (on, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s as u32, i as u32 - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s as u32, occupied.len() as u32 - 1));
    }
    out
}

/// `(cell, offset)` along one axis of length `len` with at least two runs.
fn period(runs: &[(u32, u32)], len: u32) -> Option<(u32, u32)> {
    let cell = runs.windows(2).map(|w| w[1].0 - w[0].0).min()?;
    if cell == 0 || cell > MAX_SHEET_CELL {
        return None;
    }
    (0..cell).find_map(|offset| {
        let fits = runs.iter().all(|&(s, e)| {
            if s < offset {
                return false;
            }
            let k = (s - offset) / cell;
            let end = u64::from(offset) + u64::from(k + 1) * u64::from(cell);
            u64::from(e) < end && end <= u64::from(len)
        });
        fits.then_some((cell, offset))
    })
}

/// The suggested grid for `sheet` after the background step.
pub(super) fn suggest(sheet: &Sheet, background: Option<[u8; 4]>) -> Option<SheetGrid> {
    let (w, h) = (sheet.width() as usize, sheet.height() as usize);
    let mut cols = vec![false; w];
    let mut rows = vec![false; h];
    for (y, line) in sheet.rgba().chunks_exact(w * 4).enumerate() {
        for (x, px) in line.as_chunks::<4>().0.iter().enumerate() {
            if !clear(px, background) {
                cols[x] = true;
                rows[y] = true;
            }
        }
    }
    // A fully clear sheet, or one without a clear column or row: no grid.
    if cols.iter().all(|&c| c) || rows.iter().all(|&r| r) {
        return None;
    }
    let (col_runs, row_runs) = (runs(&cols), runs(&rows));
    if col_runs.is_empty() || row_runs.is_empty() {
        return None;
    }
    let whole = |len: u32| (len <= MAX_SHEET_CELL).then_some((len, 0));
    let (x, y) = match (col_runs.len(), row_runs.len()) {
        (1, 1) => return None,
        (1, _) => (whole(sheet.width())?, period(&row_runs, sheet.height())?),
        (_, 1) => (period(&col_runs, sheet.width())?, whole(sheet.height())?),
        _ => (
            period(&col_runs, sheet.width())?,
            period(&row_runs, sheet.height())?,
        ),
    };
    Some(SheetGrid {
        cell_width: x.0,
        cell_height: y.0,
        offset_x: x.1,
        offset_y: y.1,
        spacing_x: 0,
        spacing_y: 0,
    })
}
