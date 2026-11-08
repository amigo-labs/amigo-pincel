//! Several commands as one undo step — e.g. Move (layer) on a group, which
//! moves every raster layer inside it (docs/specs/image-layer-groups.md AC-18).

use super::HistoryKind;
use super::{Command, Damage};
use crate::document::Document;
use crate::error::DocumentError;
use std::any::Any;

/// Applies its commands in order and reverts them in reverse; a failed
/// apply rolls back the ones that already ran.
pub(crate) struct Batch {
    cmds: Vec<Box<dyn Command>>,
    kind: HistoryKind,
}

impl Batch {
    pub(crate) fn new(cmds: Vec<Box<dyn Command>>, kind: HistoryKind) -> Self {
        Self { cmds, kind }
    }
}

impl Command for Batch {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        for i in 0..self.cmds.len() {
            if let Err(e) = self.cmds[i].apply(doc) {
                for done in self.cmds[..i].iter_mut().rev() {
                    done.revert(doc)?;
                }
                return Err(e);
            }
        }
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        for cmd in self.cmds.iter_mut().rev() {
            cmd.revert(doc)?;
        }
        Ok(())
    }

    fn kind(&self) -> HistoryKind {
        self.kind
    }

    fn undo_bytes(&self) -> usize {
        self.cmds.iter().map(|c| c.undo_bytes()).sum()
    }

    fn damage(&self) -> Damage {
        let mut damage = Damage::None;
        for cmd in &self.cmds {
            damage.add(cmd.damage());
        }
        damage
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
