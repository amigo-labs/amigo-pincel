//! Commands and the undo / redo bus. See `docs/specs/pincel.md` §6.

mod add_frame;
mod add_layer;
mod bus;
mod draw_line;
mod draw_rectangle;
mod error;
mod set_pixel;

pub use add_frame::AddFrame;
pub use add_layer::AddLayer;
pub use bus::{Bus, DEFAULT_HISTORY_CAP};
pub use draw_line::DrawLine;
pub use draw_rectangle::DrawRectangle;
pub use error::CommandError;
pub use set_pixel::SetPixel;

use crate::document::{CelMap, Sprite};

/// A reversible mutation on a [`Sprite`] and its [`CelMap`].
///
/// Implementations are expected to record any state needed to undo their
/// effect during [`Command::apply`], so [`Command::revert`] can restore the
/// document precisely. See `docs/specs/pincel.md` §6.1.
pub trait Command {
    /// Apply the command, mutating the document and cel map in place.
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError>;

    /// Reverse a previously-applied command. Must be a no-op for a command
    /// that has not been applied (or that already failed).
    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap);

    /// If this command can absorb `next` (e.g. consecutive pencil strokes
    /// inside one press-drag-release), update `self` to subsume both effects
    /// and return `true`. Default: no merging.
    fn merge(&mut self, _next: &Self) -> bool
    where
        Self: Sized,
    {
        false
    }
}

/// Bus-level command variant. Each variant wraps a concrete [`Command`]
/// implementation; the enum dispatches `apply` / `revert` / `merge` in the
/// bus without trait-object overhead.
#[derive(Debug)]
pub enum AnyCommand {
    SetPixel(SetPixel),
    DrawLine(DrawLine),
    DrawRectangle(DrawRectangle),
    AddLayer(AddLayer),
    AddFrame(AddFrame),
}

impl AnyCommand {
    pub(crate) fn apply(
        &mut self,
        doc: &mut Sprite,
        cels: &mut CelMap,
    ) -> Result<(), CommandError> {
        match self {
            Self::SetPixel(c) => c.apply(doc, cels),
            Self::DrawLine(c) => c.apply(doc, cels),
            Self::DrawRectangle(c) => c.apply(doc, cels),
            Self::AddLayer(c) => c.apply(doc, cels),
            Self::AddFrame(c) => c.apply(doc, cels),
        }
    }

    pub(crate) fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        match self {
            Self::SetPixel(c) => c.revert(doc, cels),
            Self::DrawLine(c) => c.revert(doc, cels),
            Self::DrawRectangle(c) => c.revert(doc, cels),
            Self::AddLayer(c) => c.revert(doc, cels),
            Self::AddFrame(c) => c.revert(doc, cels),
        }
    }

    pub(crate) fn merge(&mut self, next: &Self) -> bool {
        match (self, next) {
            (Self::SetPixel(a), Self::SetPixel(b)) => a.merge(b),
            (Self::DrawLine(a), Self::DrawLine(b)) => a.merge(b),
            (Self::DrawRectangle(a), Self::DrawRectangle(b)) => a.merge(b),
            (Self::AddLayer(a), Self::AddLayer(b)) => a.merge(b),
            (Self::AddFrame(a), Self::AddFrame(b)) => a.merge(b),
            _ => false,
        }
    }
}

impl From<SetPixel> for AnyCommand {
    fn from(c: SetPixel) -> Self {
        Self::SetPixel(c)
    }
}

impl From<DrawLine> for AnyCommand {
    fn from(c: DrawLine) -> Self {
        Self::DrawLine(c)
    }
}

impl From<DrawRectangle> for AnyCommand {
    fn from(c: DrawRectangle) -> Self {
        Self::DrawRectangle(c)
    }
}

impl From<AddLayer> for AnyCommand {
    fn from(c: AddLayer) -> Self {
        Self::AddLayer(c)
    }
}

impl From<AddFrame> for AnyCommand {
    fn from(c: AddFrame) -> Self {
        Self::AddFrame(c)
    }
}
