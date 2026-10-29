//! `CommandBatch` — several commands applied and reverted as one undo entry.
//!
//! Hosts that turn one high-level request into several commands (the agent
//! tools of `pincel-agent`: "set name, visibility and opacity of a layer",
//! "append a frame and clone its cels") wrap them in a batch so the bus
//! records a single entry. `apply` is atomic: when a member fails, the
//! members applied so far are reverted in reverse order and the error is
//! returned, leaving the document unchanged.

use crate::document::{CelMap, Sprite};

use super::dirty::DirtyRegion;
use super::error::CommandError;
use super::{AnyCommand, Command};

/// An ordered group of commands that the bus treats as one entry.
#[derive(Debug)]
pub struct CommandBatch {
    commands: Vec<AnyCommand>,
}

impl CommandBatch {
    /// Group `commands`, applied in order and reverted in reverse order.
    pub fn new(commands: Vec<AnyCommand>) -> Self {
        Self { commands }
    }

    /// Number of member commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// `true` when the batch holds no command.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Command for CommandBatch {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        for i in 0..self.commands.len() {
            if let Err(e) = self.commands[i].apply(doc, cels) {
                for applied in self.commands[..i].iter_mut().rev() {
                    applied.revert(doc, cels);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        for cmd in self.commands.iter_mut().rev() {
            cmd.revert(doc, cels);
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        match self.commands.as_slice() {
            [] => DirtyRegion::None,
            [only] => only.dirty_region(),
            _ => DirtyRegion::Canvas,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{AddFrame, SetFrameDuration};
    use crate::document::{Frame, FrameIndex};

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(4, 4)
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    #[test]
    fn apply_runs_members_in_order_and_revert_undoes_all() {
        let (mut sprite, mut cels) = doc();
        let mut batch = CommandBatch::new(vec![
            AddFrame::append(Frame::new(50)).into(),
            SetFrameDuration::new(FrameIndex::new(1), 70).into(),
        ]);
        batch.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(sprite.frames.len(), 2);
        assert_eq!(sprite.frames[1].duration_ms, 70);
        batch.revert(&mut sprite, &mut cels);
        assert_eq!(sprite.frames.len(), 1);
    }

    #[test]
    fn apply_failure_reverts_applied_members_and_returns_err() {
        let (mut sprite, mut cels) = doc();
        let before = sprite.clone();
        let mut batch = CommandBatch::new(vec![
            AddFrame::append(Frame::new(50)).into(),
            SetFrameDuration::new(FrameIndex::new(9), 70).into(),
        ]);
        assert!(batch.apply(&mut sprite, &mut cels).is_err());
        assert_eq!(sprite, before);
    }

    #[test]
    fn batch_is_one_bus_entry() {
        let (mut sprite, mut cels) = doc();
        let mut bus = crate::command::Bus::new();
        let batch = CommandBatch::new(vec![
            AddFrame::append(Frame::new(50)).into(),
            AddFrame::append(Frame::new(60)).into(),
        ]);
        bus.execute(batch.into(), &mut sprite, &mut cels)
            .expect("execute");
        assert_eq!(bus.undo_depth(), 1);
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(sprite.frames.len(), 1);
    }
}
