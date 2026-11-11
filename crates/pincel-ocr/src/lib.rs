//! Text recognition for Pincel: a Rust port of Tesseract's LSTM line
//! recognizer (`docs/specs/ocr-recognizer.md`, Decision Log 2026-11-08 in
//! `docs/specs/pincel.md` §15).
//!
//! A [`Model`] is loaded from the bytes of a `tessdata_fast` `.traineddata`
//! file; it turns an RGBA image of **one text line** into a
//! [`RecognizedLine`]: the words with their boxes and confidences.
//! [`Model::read_page`] reads a whole page (`docs/specs/ocr-page.md`): it
//! finds the text lines, blocks and columns, recognizes each line and
//! returns a [`PageResult`] with the page text in reading order. The
//! crate is pure — no I/O, no threads, no platform APIs — and a loaded model
//! can be shared between threads.
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let bytes = include_bytes!("../models/eng.traineddata");
//! let model = pincel_ocr::Model::from_bytes(bytes)?;
//! assert_eq!(model.input_height(), 36);
//! let white = [255u8; 4 * 20 * 20];
//! let line = model.recognize_line(20, 20, &white)?;
//! assert!(line.words.is_empty());
//! # Ok(())
//! # }
//! ```

mod beam;
mod error;
mod functions;
mod functions_table;
mod image;
mod network;
mod page;
mod reader;
mod recoder;
mod recognize;
mod tessdata;
mod textord;
mod unicharset;

pub use error::{LoadError, PageError, RecognizeError};
pub use page::{Control, PageBlock, PageBox, PageLine, PageResult, PageWord, Progress};

use network::{Layer, LstmComponent};
use recoder::Recoder;
use tessdata::{Component, Tessdata};
use unicharset::Unicharset;

/// `TF_INT_MODE` in the recognizer's training flags.
const TF_INT_MODE: i32 = 1;

/// A loaded Tesseract model.
#[derive(Debug, Clone)]
pub struct Model {
    network: Layer,
    unicharset: Unicharset,
    recoder: Recoder,
    null_char: usize,
    sample_iteration: i32,
    input_height: usize,
    x_scale: usize,
    version: String,
}

/// A word of a recognized line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    /// The word's text, without spaces.
    pub text: String,
    /// The word's box in line-image pixels.
    pub bbox: WordBox,
    /// Confidence from 0 to 100.
    pub confidence: u8,
}

/// An axis-aligned box in image pixels: the left and top edges and the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordBox {
    /// Left edge.
    pub left: u32,
    /// Top edge.
    pub top: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// The recognizer's result for one line image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecognizedLine {
    /// The words, ordered by the left edge of their box.
    pub words: Vec<Word>,
    /// The words' text joined by single spaces.
    pub text: String,
}

impl Model {
    /// Loads a model from the bytes of a `tessdata_fast` `.traineddata`
    /// file.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, LoadError> {
        let archive = Tessdata::parse(bytes)?;
        let lstm = archive.get(Component::Lstm).ok_or(LoadError::NoLstm)?;
        let component = LstmComponent::read(lstm)?;
        if component.training_flags & TF_INT_MODE == 0 {
            return Err(LoadError::FloatWeights);
        }
        let network = component.network;
        let input_height = network.input_height().ok_or(LoadError::NoLstm)?;
        let outputs = network.validate(1)?;
        if outputs == 0 {
            return Err(LoadError::NoLstm);
        }
        let null_char = usize::try_from(component.null_char)
            .ok()
            .filter(|&n| n < outputs)
            .ok_or(LoadError::CharsetMismatch)?;
        let unicharset = archive
            .get(Component::LstmUnicharset)
            .ok_or(LoadError::CharsetUnreadable)
            .and_then(Unicharset::parse)?;
        let recoder = archive
            .get(Component::LstmRecoder)
            .ok_or(LoadError::CharsetUnreadable)
            .and_then(Recoder::parse)?;
        if recoder.code_range() > outputs || recoder.len() != unicharset.len() {
            return Err(LoadError::CharsetMismatch);
        }
        let version = archive
            .get(Component::Version)
            .map(|v| String::from_utf8_lossy(v).into_owned())
            .unwrap_or_default();
        let x_scale = network.x_scale_factor().max(1);
        Ok(Self {
            network,
            unicharset,
            recoder,
            null_char,
            sample_iteration: component.sample_iteration,
            input_height,
            x_scale,
            version,
        })
    }

    /// Number of entries in the model's LSTM character set, including
    /// Tesseract's special entries (space, `Joined`, …).
    pub fn character_set_size(&self) -> usize {
        self.unicharset.len()
    }

    /// The height in pixels the network scales each line to.
    pub fn input_height(&self) -> u32 {
        self.input_height as u32
    }

    /// The model's version string, for diagnostics.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Recognizes a single line of text in an RGBA image of
    /// `width × height` pixels (`width * height * 4` bytes, row-major).
    pub fn recognize_line(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<RecognizedLine, RecognizeError> {
        recognize::recognize_line(self, width, height, rgba)
    }

    /// Reads a whole page: an RGBA image of `width × height` pixels
    /// (`width * height * 4` bytes, row-major) with any number of lines,
    /// blocks and columns (docs/specs/ocr-page.md). Returns the text
    /// blocks in reading order and the page text.
    ///
    /// `progress` is called during layout analysis at least once per
    /// million pixels, then with "0 of n lines" and after each line;
    /// answering [`Control::Cancel`] stops the reading with
    /// [`PageError::Cancelled`].
    pub fn read_page(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        progress: &mut dyn FnMut(Progress) -> Control,
    ) -> Result<PageResult, PageError> {
        page::read_page(self, width, height, rgba, progress)
    }
}
