//! Pixel-exact mode: one layer per distinct colour, outlines along the
//! pixel edges (`docs/specs/vectorize-core.md` AC-12, AC-13, AC-20).
//! Potrace smooths pixel staircases even with every corner kept, so pixel
//! art gets its own tracer.

use std::collections::HashMap;

use crate::color::Rgba;
use crate::error::{TraceError, try_push, try_vec};

/// Most distinct colours pixel-exact mode accepts.
pub const MAX_EXACT_COLORS: usize = 256;

/// One traced colour: its pixel count, path data and number of steps.
pub(crate) struct ExactLayer {
    pub color: Rgba,
    pub area: u64,
    pub path_data: String,
    pub outlines: u64,
    pub steps: u64,
}

/// Unit step directions: right, down, left, up (screen coordinates).
const DIRS: [(i64, i64); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];

/// A boundary edge from vertex `(x, y)` one unit in `DIRS[dir]`; the filled
/// pixel lies on its right-hand side.
#[derive(Clone, Copy)]
struct Edge {
    x: i64,
    y: i64,
    dir: usize,
}

/// Traces every colour with alpha > 0 along the pixel edges, layers
/// ordered by area (largest first), ties by first appearance.
pub(crate) fn trace_exact(rgba: &[u8], w: usize, h: usize) -> Result<Vec<ExactLayer>, TraceError> {
    // Colour index per pixel, colours in order of first appearance.
    let mut index: HashMap<u32, u16> = HashMap::new();
    let mut colors: Vec<Rgba> = Vec::new();
    let mut seen_all = 0_usize;
    let mut idx = try_vec(w * h, u16::MAX)?;
    for (p, px) in rgba.as_chunks::<4>().0.iter().enumerate() {
        if px[3] == 0 {
            continue;
        }
        let key = u32::from_be_bytes([px[0], px[1], px[2], px[3]]);
        // The map keeps counting past the limit for the error message, so
        // it can grow with the image: reserve fallibly (AC-18).
        let slot = match index.get(&key) {
            Some(&slot) => slot,
            None => {
                index.try_reserve(1).map_err(|_| TraceError::OutOfMemory)?;
                let slot = index.len().min(usize::from(u16::MAX - 1)) as u16;
                index.insert(key, slot);
                seen_all += 1;
                slot
            }
        };
        if seen_all > MAX_EXACT_COLORS {
            continue;
        }
        if usize::from(slot) == colors.len() {
            colors.push(Rgba::new(px[0], px[1], px[2], px[3]));
        }
        idx[p] = slot;
    }
    if seen_all > MAX_EXACT_COLORS {
        return Err(TraceError::TooManyColors {
            count: seen_all,
            max: MAX_EXACT_COLORS,
        });
    }

    let (wi, hi) = (w as i64, h as i64);
    let at = |x: i64, y: i64| -> u16 {
        if x < 0 || y < 0 || x >= wi || y >= hi {
            u16::MAX
        } else {
            idx[(y * wi + x) as usize]
        }
    };
    // Count first, then reserve each colour's edge list exactly: the lists
    // grow with the image, so running out of memory is an error (AC-18).
    let mut area = vec![0_u64; colors.len()];
    let mut counts = vec![0_usize; colors.len()];
    for y in 0..hi {
        for x in 0..wi {
            let c = at(x, y);
            if c == u16::MAX {
                continue;
            }
            area[usize::from(c)] += 1;
            counts[usize::from(c)] += usize::from(at(x, y - 1) != c)
                + usize::from(at(x + 1, y) != c)
                + usize::from(at(x, y + 1) != c)
                + usize::from(at(x - 1, y) != c);
        }
    }
    let mut edges: Vec<Vec<Edge>> = Vec::with_capacity(colors.len());
    for &n in &counts {
        let mut list = Vec::new();
        list.try_reserve_exact(n)
            .map_err(|_| TraceError::OutOfMemory)?;
        edges.push(list);
    }
    for y in 0..hi {
        for x in 0..wi {
            let c = at(x, y);
            if c == u16::MAX {
                continue;
            }
            let list = &mut edges[usize::from(c)];
            if at(x, y - 1) != c {
                list.push(Edge { x, y, dir: 0 });
            }
            if at(x + 1, y) != c {
                list.push(Edge {
                    x: x + 1,
                    y,
                    dir: 1,
                });
            }
            if at(x, y + 1) != c {
                list.push(Edge {
                    x: x + 1,
                    y: y + 1,
                    dir: 2,
                });
            }
            if at(x - 1, y) != c {
                list.push(Edge {
                    x,
                    y: y + 1,
                    dir: 3,
                });
            }
        }
    }

    let mut order: Vec<usize> = (0..colors.len()).collect();
    order.sort_by(|&a, &b| area[b].cmp(&area[a]));
    let mut layers = Vec::with_capacity(order.len());
    for c in order {
        let (path_data, outlines, steps) = link(&edges[c], wi + 1)?;
        layers.push(ExactLayer {
            color: colors[c],
            area: area[c],
            path_data,
            outlines,
            steps,
        });
    }
    Ok(layers)
}

/// Links boundary edges into closed outlines and writes them as path data.
/// Every buffer here grows with the outline, so each is reserved fallibly
/// (AC-18).
fn link(edges: &[Edge], stride: i64) -> Result<(String, u64, u64), TraceError> {
    let key = |x: i64, y: i64| y * stride + x;
    let mut from: HashMap<i64, [usize; 2]> = HashMap::new();
    from.try_reserve(edges.len())
        .map_err(|_| TraceError::OutOfMemory)?;
    for (i, e) in edges.iter().enumerate() {
        from.entry(key(e.x, e.y))
            .and_modify(|slot| slot[1] = i)
            .or_insert([i, usize::MAX]);
    }
    let mut used = try_vec(edges.len(), false)?;
    let mut out = String::new();
    let (mut outlines, mut steps) = (0_u64, 0_u64);
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let mut dirs: Vec<usize> = Vec::new();
        let mut verts: Vec<(i64, i64)> = Vec::new();
        let mut e = start;
        let (sx, sy) = (edges[start].x, edges[start].y);
        loop {
            used[e] = true;
            let edge = edges[e];
            try_push(&mut verts, (edge.x, edge.y))?;
            try_push(&mut dirs, edge.dir)?;
            let (nx, ny) = (edge.x + DIRS[edge.dir].0, edge.y + DIRS[edge.dir].1);
            if (nx, ny) == (sx, sy) {
                break;
            }
            let [a, b] = from[&key(nx, ny)];
            let free = |i: usize| i != usize::MAX && !used[i];
            // At a vertex where two pixels touch diagonally, turn right:
            // the outline keeps hugging the same pixel.
            let right = (edge.dir + 1) % 4;
            e = match (free(a), free(b)) {
                (true, true) => {
                    if edges[b].dir == right {
                        b
                    } else {
                        a
                    }
                }
                (true, false) => a,
                (false, true) => b,
                (false, false) => break,
            };
        }
        // Start at a corner, then one step per run of equal directions;
        // the last run is closed by `z`.
        let n = dirs.len();
        let Some(first) = (0..n).find(|&i| dirs[i] != dirs[(i + n - 1) % n]) else {
            continue;
        };
        let (mx, my) = verts[first];
        out.try_reserve(STEP_ROOM)
            .map_err(|_| TraceError::OutOfMemory)?;
        out.push_str(&format!("M{mx} {my}"));
        let mut runs: Vec<(usize, i64)> = Vec::new();
        for k in 0..n {
            let d = dirs[(first + k) % n];
            match runs.last_mut() {
                Some((rd, len)) if *rd == d => *len += 1,
                _ => try_push(&mut runs, (d, 1))?,
            }
        }
        for &(d, len) in &runs[..runs.len() - 1] {
            let (dx, dy) = (DIRS[d].0 * len, DIRS[d].1 * len);
            let sep = if dy < 0 { "" } else { " " };
            out.try_reserve(STEP_ROOM)
                .map_err(|_| TraceError::OutOfMemory)?;
            out.push_str(&format!("l{dx}{sep}{dy}"));
        }
        out.try_reserve(1).map_err(|_| TraceError::OutOfMemory)?;
        out.push('z');
        outlines += 1;
        steps += runs.len() as u64;
    }
    Ok((out, outlines, steps))
}

/// Bytes reserved before writing one move or step: a letter and two
/// integers of at most 20 characters each.
const STEP_ROOM: usize = 48;
