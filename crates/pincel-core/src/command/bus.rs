//! Command bus: linear undo / redo stack. See `docs/specs/pincel.md` §6.2.

use std::collections::VecDeque;

use crate::document::{CelMap, Sprite};

use super::AnyCommand;
use super::error::CommandError;

/// Default cap on the undo stack (matches `docs/specs/pincel.md` §6.2).
pub const DEFAULT_HISTORY_CAP: usize = 100;

/// Linear undo / redo history.
///
/// Each call to [`Bus::execute`] applies the command, drops the redo stack,
/// and either coalesces with the previous entry (via [`AnyCommand::merge`])
/// or pushes a new entry. When the undo stack exceeds the configured cap,
/// the oldest entry is dropped.
#[derive(Debug)]
pub struct Bus {
    undo: VecDeque<AnyCommand>,
    redo: Vec<AnyCommand>,
    cap: usize,
}

impl Bus {
    /// Create a bus with the default history cap.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HISTORY_CAP)
    }

    /// Create a bus with the given undo cap. A cap of `0` disables history.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            cap,
        }
    }

    /// Apply `cmd` and push it onto the undo stack. The redo stack is cleared
    /// because executing a fresh command branches history.
    pub fn execute(
        &mut self,
        mut cmd: AnyCommand,
        doc: &mut Sprite,
        cels: &mut CelMap,
    ) -> Result<(), CommandError> {
        cmd.apply(doc, cels)?;
        self.redo.clear();

        if self.cap == 0 {
            return Ok(());
        }

        if let Some(top) = self.undo.back_mut() {
            if top.merge(&cmd) {
                return Ok(());
            }
        }

        self.undo.push_back(cmd);
        if self.undo.len() > self.cap {
            self.undo.pop_front();
        }
        Ok(())
    }

    /// Revert the most recent command. Returns `true` if a command was undone.
    pub fn undo(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> bool {
        let Some(mut cmd) = self.undo.pop_back() else {
            return false;
        };
        cmd.revert(doc, cels);
        self.redo.push(cmd);
        true
    }

    /// Re-apply the most recently undone command. Returns `Ok(true)` if a
    /// command was redone.
    pub fn redo(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<bool, CommandError> {
        let Some(mut cmd) = self.redo.pop() else {
            return Ok(false);
        };
        cmd.apply(doc, cels)?;
        self.undo.push_back(cmd);
        Ok(true)
    }

    /// Number of commands available to undo.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of commands available to redo.
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}
