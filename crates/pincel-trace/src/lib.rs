//! Raster-to-vector tracing for Pincel: the owner's Vectorizer core
//! (`daniel-rck/Vectorizer`), ported to Rust (`docs/specs/vectorize-core.md`,
//! Decision Log 2026-11-01 in `docs/specs/pincel.md` §15).
//!
//! An RGBA8 image goes in, an SVG comes out: Potrace in a colour mode (one
//! layer per reduced colour) or a monochrome mode, or a pixel-exact mode for
//! pixel art. The crate is pure — no threads, no I/O, no platform APIs —
//! like `pincel-effects`, so both editor modes, the desktop shell, the agent
//! tools and a CLI share it.
//!
//! ```
//! use pincel_trace::{TraceOptions, TraceSession};
//!
//! let rgba = [0, 0, 0, 255].repeat(4 * 4);
//! let mut session = TraceSession::new(&rgba, 4, 4)?;
//! let result = session.trace(&TraceOptions::default())?;
//! assert_eq!(result.layers.len(), 1);
//! # Ok::<(), pincel_trace::TraceError>(())
//! ```

mod color;
mod error;
mod potrace;
mod quality;
mod quantize;
mod session;

pub use color::{Rgb, Rgba};
pub use error::TraceError;
pub use potrace::{PotraceParams, TurnPolicy};
pub use session::{
    Layer, MAX_COLORS, MAX_SIDE, Mode, ResultKind, Threshold, TraceOptions, TraceResult,
    TraceSession, TraceStats,
};
