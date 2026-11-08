//! Error kinds of `pincel-ocr` (docs/specs/ocr-recognizer.md AC-02, AC-07).

use thiserror::Error;

/// Why a model could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LoadError {
    /// The bytes are not a Tesseract `.traineddata` archive.
    #[error("not a .traineddata file")]
    NotTraineddata,
    /// A component, or a size or offset inside one, points past the end of
    /// the data.
    #[error("model data is truncated")]
    Truncated,
    /// The model has no LSTM network, or its network is malformed.
    #[error("model has no usable LSTM network")]
    NoLstm,
    /// The LSTM unicharset or recoder is missing or unreadable.
    #[error("LSTM unicharset or recoder missing or unreadable")]
    CharsetUnreadable,
    /// The LSTM unicharset or recoder does not match the network's output
    /// size.
    #[error("LSTM unicharset or recoder does not match the network")]
    CharsetMismatch,
    /// The network uses a layer type this crate does not implement.
    #[error("unsupported network layer type: {0}")]
    UnsupportedLayer(String),
    /// The network has float weights (a `tessdata_best` model); only the
    /// int8 `tessdata_fast` models are supported.
    #[error("float-weight models (tessdata_best) are not supported")]
    FloatWeights,
}

/// Why a line image could not be recognized.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecognizeError {
    /// The pixel buffer's length is not `width × height × 4`, or that
    /// product overflows `usize`.
    #[error("bad image: buffer length does not match width × height × 4")]
    BadImage,
}
