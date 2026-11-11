//! Reading a whole page (docs/specs/ocr-page.md): find the text lines,
//! recognize each with the line recognizer and assemble the page text in
//! reading order.
//!
//! The steps: [`ink`] finds the ink (pixels on a strong local edge, so both
//! polarities and coloured backgrounds work alike) and its connected
//! components; [`lines`] estimates the skew and groups the components into
//! lines in deskewed coordinates; [`blocks`] groups the lines into text
//! blocks and orders them by the recursive cut; [`extract`] cuts each line
//! out of the page, straightened and with other lines' ink masked, for the
//! recognizer, and maps the word boxes back.

mod blocks;
mod extract;
mod ink;
mod lines;

use crate::{Model, PageError};

/// The largest width or height Image mode opens.
const MAX_SIDE: usize = 16_384;
/// The largest area Image mode opens.
const MAX_AREA: usize = 8192 * 8192;
/// Layout work between two progress reports, in pixels.
pub(crate) const REPORT_PIXELS: usize = 1_000_000;

/// An axis-aligned box in page-image pixels with inclusive edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageBox {
    /// Leftmost column.
    pub left: u32,
    /// Topmost row.
    pub top: u32,
    /// Rightmost column.
    pub right: u32,
    /// Bottommost row.
    pub bottom: u32,
}

impl PageBox {
    /// The smallest box that holds both boxes.
    pub fn union(&self, other: &PageBox) -> PageBox {
        PageBox {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    /// Whether `other` lies inside this box (edges included).
    pub fn contains(&self, other: &PageBox) -> bool {
        self.left <= other.left
            && self.top <= other.top
            && other.right <= self.right
            && other.bottom <= self.bottom
    }
}

/// A recognized word of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageWord {
    /// The word's text, without spaces.
    pub text: String,
    /// The word's box in page pixels.
    pub bbox: PageBox,
    /// Confidence from 0 to 100.
    pub confidence: u8,
}

/// A line of a page: words on one baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageLine {
    /// The words, left to right.
    pub words: Vec<PageWord>,
    /// The box enclosing all words.
    pub bbox: PageBox,
}

/// A text block: lines read one after the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBlock {
    /// The lines, top to bottom.
    pub lines: Vec<PageLine>,
    /// The box enclosing all lines.
    pub bbox: PageBox,
}

/// The result of reading a page: the text blocks in reading order and the
/// page text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PageResult {
    /// The text blocks in reading order.
    pub blocks: Vec<PageBlock>,
    /// Words joined by one space, lines by a line break and blocks by an
    /// empty line.
    pub text: String,
}

impl PageResult {
    /// Whether no text was found.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// A progress report of [`Model::read_page`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Layout analysis: `done` of `total` units of pixel work.
    Layout {
        /// Units done so far.
        done: u64,
        /// Units in all.
        total: u64,
    },
    /// Recognition: `done` of `total` lines recognized.
    Lines {
        /// Lines recognized so far.
        done: usize,
        /// Lines found by the layout analysis.
        total: usize,
    },
}

/// The caller's answer to a progress report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// Go on reading.
    Continue,
    /// Stop and return [`PageError::Cancelled`].
    Cancel,
}

/// The page image with its flattening background (docs/specs/ocr-page.md,
/// "Flattening").
pub(crate) struct Source<'a> {
    pub(crate) width: usize,
    pub(crate) height: usize,
    rgba: &'a [u8],
    background: u32,
}

impl<'a> Source<'a> {
    fn new(width: usize, height: usize, rgba: &'a [u8]) -> Self {
        let mut weighted = 0u64;
        let mut alpha_total = 0u64;
        for p in rgba.as_chunks::<4>().0 {
            let luma = 299 * u64::from(p[0]) + 587 * u64::from(p[1]) + 114 * u64::from(p[2]);
            weighted += luma * u64::from(p[3]);
            alpha_total += u64::from(p[3]);
        }
        // Over black when the alpha-weighted mean luminance is above 50 %
        // (the recognizer's rule, src/image/mod.rs `flatten`).
        let background = if weighted * 2 > alpha_total * 255 * 1000 {
            0
        } else {
            255
        };
        Self {
            width,
            height,
            rgba,
            background,
        }
    }

    /// The flattened colour of a pixel.
    #[inline]
    pub(crate) fn rgb(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 4;
        let p = &self.rgba[i..i + 4];
        let a = u32::from(p[3]);
        let mix = |c: u8| ((u32::from(c) * a + self.background * (255 - a) + 127) / 255) as u8;
        [mix(p[0]), mix(p[1]), mix(p[2])]
    }

    /// The Rec. 601 luma of a flattened pixel.
    #[inline]
    pub(crate) fn luma(&self, x: usize, y: usize) -> u8 {
        luma(self.rgb(x, y))
    }
}

/// Rec. 601 luma, rounded.
#[inline]
pub(crate) fn luma(c: [u8; 3]) -> u8 {
    ((299 * u32::from(c[0]) + 587 * u32::from(c[1]) + 114 * u32::from(c[2]) + 500) / 1000) as u8
}

/// Passes progress reports to the caller and turns "cancel" into an error.
pub(crate) struct Reporter<'p> {
    callback: &'p mut dyn FnMut(Progress) -> Control,
}

impl Reporter<'_> {
    pub(crate) fn report(&mut self, progress: Progress) -> Result<(), PageError> {
        match (self.callback)(progress) {
            Control::Continue => Ok(()),
            Control::Cancel => Err(PageError::Cancelled),
        }
    }
}

pub(crate) fn read_page(
    model: &Model,
    width: u32,
    height: u32,
    rgba: &[u8],
    progress: &mut dyn FnMut(Progress) -> Control,
) -> Result<PageResult, PageError> {
    let (w, h) = (width as usize, height as usize);
    let expected = w
        .checked_mul(h)
        .and_then(|n| n.checked_mul(4))
        .ok_or(PageError::BadImage)?;
    if rgba.len() != expected {
        return Err(PageError::BadImage);
    }
    let mut reporter = Reporter { callback: progress };
    if w == 0 || h == 0 {
        reporter.report(Progress::Lines { done: 0, total: 0 })?;
        return Ok(PageResult::default());
    }
    if w > MAX_SIDE || h > MAX_SIDE || w * h > MAX_AREA {
        return Err(PageError::TooLarge);
    }
    let source = Source::new(w, h, rgba);
    let ink = ink::find(&source, &mut reporter)?;
    let layout = lines::find(&ink, w, h);
    let order = blocks::order(&layout.lines);
    let total = layout.lines.len();
    reporter.report(Progress::Lines { done: 0, total })?;
    let mut done = 0;
    let mut result = PageResult::default();
    for block in order {
        let mut page_block: Option<PageBlock> = None;
        for index in block {
            let words = extract::recognize(model, &source, &ink, &layout, index);
            done += 1;
            reporter.report(Progress::Lines { done, total })?;
            let Some(bbox) = words.iter().map(|w| w.bbox).reduce(|a, b| a.union(&b)) else {
                continue;
            };
            let line = PageLine { words, bbox };
            match &mut page_block {
                Some(b) => {
                    b.bbox = b.bbox.union(&bbox);
                    b.lines.push(line);
                }
                None => {
                    page_block = Some(PageBlock {
                        lines: vec![line],
                        bbox,
                    })
                }
            }
        }
        result.blocks.extend(page_block);
    }
    result.text = page_text(&result.blocks);
    Ok(result)
}

/// The page text of blocks (docs/specs/ocr-page.md, "Page text").
pub(crate) fn page_text(blocks: &[PageBlock]) -> String {
    blocks
        .iter()
        .map(|b| {
            b.lines
                .iter()
                .map(|l| {
                    l.words
                        .iter()
                        .map(|w| w.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
