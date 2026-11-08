//! Tesseract's page layout (`textord`) for a single text line
//! (`--psm 7`): where the words of a line image are, what the LSTM is
//! shown and which blobs make up each word.
//!
//! Each file ports one area of Tesseract 5.5.4's `src/textord` and
//! `src/ccstruct` with the default parameters; the functions name the C++
//! function they port. Coordinates follow Tesseract: `x` grows to the
//! right, `y` grows upwards from the bottom of the image.
//!
//! The stages ([`page_layout`]): crack-following outlines nested into
//! blobs and the noise filter (`find_components`, [`tordmain`]); one row
//! of all blobs (`make_single_row`, [`makerow`]); a straight baseline
//! ([`baseline`]); underlines, blob association and the spline baseline
//! with the x-height ([`makerow`], [`oldbasel`], [`xheight`],
//! [`underlin`]); words ([`tospace`]) and noise cleanup ([`tordmain`]).
//! [`pageres`] then forms the groups the LSTM reads and hands the blobs
//! back to the recognized words.
//!
//! Not ported, because no line of the golden corpus needs them: the
//! fixed-pitch analysis (`compute_fixed_pitch`, `fixed_pitch_words`), so
//! every row is treated as proportional; and the rotation of a wide blob
//! by the skew before `test_underline` (exact for unskewed lines).

pub(crate) mod baseline;
pub(crate) mod blob;
pub(crate) mod blobbox;
pub(crate) mod cxx;
pub(crate) mod detlinefit;
pub(crate) mod fpchop;
pub(crate) mod makerow;
pub(crate) mod oldbasel;
pub(crate) mod outline;
pub(crate) mod pageres;
pub(crate) mod quspline;
pub(crate) mod rect;
pub(crate) mod stats;
pub(crate) mod threshold;
pub(crate) mod tordmain;
pub(crate) mod tospace;
pub(crate) mod underlin;
pub(crate) mod xheight;

pub(crate) use pageres::{clip_to_range, lstm_box, recog_words, replace_current_word};
pub(crate) use rect::TBox;
pub(crate) use threshold::binarize;
pub(crate) use tospace::Row;

/// `BaselineDetect::ComputeBaselineSplinesAndXheights` for the page's
/// block: `PrepareForSplineFitting` (underlines, `pre_associate_blobs`),
/// the spline baselines and the x-heights (`FitBaselineSplines`).
pub(crate) fn compute_baseline_splines(block: &mut tordmain::ToBlock, skew: f64, page: TBox) {
    let gradient = skew.tan() as f32;
    makerow::separate_underlines(block);
    makerow::pre_associate_blobs(block);
    oldbasel::make_spline_rows(block, gradient, page);
    xheight::compute_block_xheight(block, gradient);
    underlin::restore_underlined_blobs(block);
}

/// Tesseract's page layout for `--psm 7` (`Textord::TextordPage`): the
/// rows of words of a binary line image (`black`, row-major from the
/// top), after noise cleanup.
pub(crate) fn page_layout(black: &[bool], width: usize, height: usize) -> Vec<Row> {
    let page = TBox::new(0, 0, width as i32, height as i32);
    let mut block = tordmain::find_components(black, width, height);
    makerow::make_single_row(&mut block, page);
    let skew = baseline::compute_straight_baselines(&mut block);
    compute_baseline_splines(&mut block, skew, page);
    let rows = tospace::make_words(&mut block);
    tordmain::cleanup_rows(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two "words" of three letter-sized bars each on a white line.
    fn two_words() -> (Vec<bool>, usize, usize) {
        let (w, h) = (90usize, 30usize);
        let mut black = vec![false; w * h];
        for start in [5usize, 50] {
            for k in 0..3 {
                let x0 = start + k * 9;
                for y in 8..22 {
                    for x in x0..x0 + 6 {
                        black[y * w + x] = true;
                    }
                }
            }
        }
        (black, w, h)
    }

    #[test]
    fn page_layout_splits_words_at_wide_gap() {
        let (black, w, h) = two_words();
        let rows = page_layout(&black, w, h);
        assert_eq!(rows.len(), 1);
        let words = &rows[0].words;
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].blobs.len(), 3);
        assert_eq!(words[0].bounding_box(), TBox::new(5, 8, 29, 22));
        assert_eq!(words[1].bounding_box(), TBox::new(50, 8, 74, 22));
    }

    #[test]
    fn page_layout_of_blank_image_has_one_page_word() {
        let rows = page_layout(&[false; 20 * 10], 20, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].words.len(), 1);
    }

    #[test]
    fn replace_current_word_gives_blobs_to_words_by_middle() {
        let (black, w, h) = two_words();
        let rows = page_layout(&black, w, h);
        let groups = recog_words(&rows[0]);
        assert_eq!(groups.len(), 1);
        // Two recognized words, one character cell per bar.
        let cell = |l: i32, r: i32| TBox::new(l, 0, r, 30);
        let chars = vec![
            vec![cell(5, 12), cell(14, 21), cell(23, 29)],
            vec![cell(50, 57), cell(59, 66), cell(68, 74)],
        ];
        let boxes = replace_current_word(&groups[0], &chars);
        assert_eq!(
            boxes,
            vec![TBox::new(5, 8, 29, 22), TBox::new(50, 8, 74, 22)]
        );
    }
}
