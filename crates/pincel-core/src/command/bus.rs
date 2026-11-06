//! Command bus: linear undo / redo stack. See `docs/specs/pincel.md` §6.2.

use std::collections::VecDeque;

use crate::document::{CelMap, Sprite};
use crate::geometry::Point;

use super::AnyCommand;
use super::dirty::DirtyRegion;
use super::error::CommandError;
use super::history::HistoryKind;

/// One undo / redo entry: the command and the kind of action that made it.
#[derive(Debug)]
struct Step {
    cmd: AnyCommand,
    kind: HistoryKind,
}

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
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    cap: usize,
    /// Set once the cap has dropped an entry from the bottom of the undo
    /// stack (`docs/specs/history-panel.md` AC-03).
    dropped: bool,
    /// Bumped whenever the list of steps changes (see [`Bus::revision`]).
    revision: u32,
    /// The [`DirtyRegion`] reported by the most recent successful
    /// `execute` / `undo` / `redo`. Stays as the last value across
    /// subsequent no-op calls so the caller can read it after the fact
    /// without changing the existing return-shape API.
    last_dirty: DirtyRegion,
    /// When set, the next `execute` skips the merge-with-top attempt (and
    /// clears the flag). See [`Bus::seal`].
    sealed: bool,
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
            dropped: false,
            revision: 0,
            last_dirty: DirtyRegion::None,
            sealed: false,
        }
    }

    /// Seal the current history top: the next [`Bus::execute`] will push a
    /// fresh entry instead of merging into it. Callers use this to delimit
    /// gestures — e.g. the UI seals on pointer-up so two pencil strokes
    /// stay two undo entries even though consecutive `SetPixel`s merge.
    /// `undo` / `redo` seal implicitly so a new command never coalesces
    /// into restored history.
    pub fn seal(&mut self) {
        self.sealed = true;
    }

    /// Apply `cmd` and push it onto the undo stack. The redo stack is cleared
    /// because executing a fresh command branches history. A paint command
    /// that wrote no pixel (a fill into its own color, a shape, text or
    /// gradient that missed the cel) leaves both stacks as they were: an
    /// undo step that changes nothing would mark the document dirty.
    ///
    /// The entry's history kind is the command's own [`AnyCommand::kind`];
    /// [`Bus::execute_as`] names it instead.
    pub fn execute(
        &mut self,
        cmd: AnyCommand,
        doc: &mut Sprite,
        cels: &mut CelMap,
    ) -> Result<(), CommandError> {
        let kind = cmd.kind();
        self.execute_as(kind, cmd, doc, cels)
    }

    /// [`Bus::execute`] with the history kind `kind` — for commands several
    /// actions share (an effect's whole-cel write, a crop's reframe, a
    /// cut's clear). When the command merges into the history top, the top
    /// keeps its kind.
    pub fn execute_as(
        &mut self,
        kind: HistoryKind,
        mut cmd: AnyCommand,
        doc: &mut Sprite,
        cels: &mut CelMap,
    ) -> Result<(), CommandError> {
        cmd.apply(doc, cels)?;
        self.last_dirty = cmd.dirty_region();
        if cmd.wrote_no_pixel() {
            return Ok(());
        }
        self.redo.clear();

        if self.cap == 0 {
            return Ok(());
        }

        if self.sealed {
            self.sealed = false;
        } else if let Some(top) = self.undo.back_mut()
            && top.cmd.merge(&cmd)
        {
            return Ok(());
        }

        self.undo.push_back(Step { cmd, kind });
        self.revision = self.revision.wrapping_add(1);
        if self.undo.len() > self.cap {
            self.undo.pop_front();
            self.dropped = true;
        }
        Ok(())
    }

    /// Extend the live stroke gesture to `to`: when the unsealed undo top is
    /// a [`PencilStroke`](super::PencilStroke), it paints the segment from
    /// its last sample to `to` (pen pressure `pressure` at `to`, `1.0` for
    /// mouse and touch) and stays one undo entry. Returns `Ok(false)`
    /// — and changes nothing — when there is no live stroke to extend (the
    /// bus is sealed, e.g. by [`Bus::seal`] or an undo, or the top is
    /// another command); the caller then starts a fresh stroke.
    pub fn continue_stroke(
        &mut self,
        to: Point,
        pressure: f32,
        cels: &mut CelMap,
    ) -> Result<bool, CommandError> {
        if self.sealed {
            return Ok(false);
        }
        let Some(Step {
            cmd: AnyCommand::PencilStroke(stroke),
            ..
        }) = self.undo.back_mut()
        else {
            return Ok(false);
        };
        self.last_dirty = stroke.extend_to(to, pressure, cels)?;
        self.redo.clear();
        Ok(true)
    }

    /// The command on top of the undo stack.
    #[cfg(test)]
    pub(crate) fn top(&self) -> Option<&AnyCommand> {
        self.undo.back().map(|s| &s.cmd)
    }

    /// Revert the most recent command. Returns `true` if a command was undone.
    pub fn undo(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> bool {
        let Some(mut step) = self.undo.pop_back() else {
            return false;
        };
        step.cmd.revert(doc, cels);
        self.last_dirty = step.cmd.dirty_region();
        self.redo.push(step);
        self.sealed = true;
        self.revision = self.revision.wrapping_add(1);
        true
    }

    /// Re-apply the most recently undone command. Returns `Ok(true)` if a
    /// command was redone. A command that fails stays on the redo stack.
    pub fn redo(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<bool, CommandError> {
        let Some(mut step) = self.redo.pop() else {
            return Ok(false);
        };
        self.sealed = true;
        if let Err(e) = step.cmd.apply(doc, cels) {
            self.redo.push(step);
            return Err(e);
        }
        self.last_dirty = step.cmd.dirty_region();
        self.undo.push_back(step);
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }

    /// Move through the history by `delta` steps (`-n` undoes `n`, `+n`
    /// redoes `n`) as that many [`Bus::undo`] / [`Bus::redo`] calls would,
    /// clamped to the steps there are. Returns how many steps it moved.
    /// When a redo fails the jump stops at the last state it reached, the
    /// failed step stays on the redo stack, and the error comes back with
    /// the steps moved before it (`docs/specs/history-panel.md` AC-05,
    /// AC-12). After a jump of more than one step
    /// [`Bus::last_dirty_region`] is the whole canvas.
    pub fn jump(
        &mut self,
        delta: i64,
        doc: &mut Sprite,
        cels: &mut CelMap,
    ) -> Result<usize, (usize, CommandError)> {
        let steps = delta.unsigned_abs();
        let mut moved = 0usize;
        while (moved as u64) < steps {
            let more = if delta < 0 {
                self.undo(doc, cels)
            } else {
                match self.redo(doc, cels) {
                    Ok(more) => more,
                    Err(e) => {
                        self.widen_dirty(moved);
                        return Err((moved, e));
                    }
                }
            };
            if !more {
                break;
            }
            moved += 1;
        }
        self.widen_dirty(moved);
        Ok(moved)
    }

    /// After a jump of more than one step the last dirty region is the
    /// whole canvas: the steps may have touched anything.
    fn widen_dirty(&mut self, moved: usize) {
        if moved > 1 {
            self.last_dirty = DirtyRegion::Canvas;
        }
    }

    /// The newest undo entry, if any — for inspection (memory accounting,
    /// tests); the bus keeps ownership.
    pub fn last(&self) -> Option<&AnyCommand> {
        self.undo.back().map(|s| &s.cmd)
    }

    /// The kinds of the undo steps, oldest first.
    pub fn undo_kinds(&self) -> impl Iterator<Item = HistoryKind> + '_ {
        self.undo.iter().map(|s| s.kind)
    }

    /// The kinds of the redo steps, in the order [`Bus::redo`] restores
    /// them (the next redo first).
    pub fn redo_kinds(&self) -> impl Iterator<Item = HistoryKind> + '_ {
        self.redo.iter().rev().map(|s| s.kind)
    }

    /// A counter that changes whenever the list of steps changes (a new
    /// step, an undo, a redo, a dropped step) and stays put while a stroke
    /// or another merge only grows the newest step, so a host re-reads
    /// [`Bus::undo_kinds`] / [`Bus::redo_kinds`] only when it moved.
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// `true` once the history cap has dropped a step: the oldest state
    /// left is no longer the document's start.
    pub fn dropped_steps(&self) -> bool {
        self.dropped
    }

    /// Number of commands available to undo.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Number of commands available to redo.
    pub fn redo_depth(&self) -> usize {
        self.redo.len()
    }

    /// [`DirtyRegion`] reported by the most recent successful
    /// `execute` / `undo` / `redo`. Returns [`DirtyRegion::None`] for a
    /// fresh bus that has never applied a command.
    pub fn last_dirty_region(&self) -> DirtyRegion {
        self.last_dirty
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{
        AddFrame, AddLayer, BrushShape, Ink, Orientation, PencilStroke, PixelBrush, ReframeCanvas,
        SetFrameDuration, SetLayerName, TransformCanvas, TransformCel,
    };
    use crate::document::{Frame, FrameIndex, Layer, LayerId, Rgba};
    use crate::geometry::Rect;

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    fn ids(kinds: impl Iterator<Item = HistoryKind>) -> Vec<String> {
        kinds.map(|k| k.id().into_owned()).collect()
    }

    /// Three steps: add frame, rename layer, set frame 1's duration.
    fn three_steps(bus: &mut Bus, s: &mut Sprite, c: &mut CelMap) {
        bus.execute(AddFrame::append(Frame::new(50)).into(), s, c)
            .expect("add frame");
        bus.execute(SetLayerName::new(LayerId::new(0), "ground").into(), s, c)
            .expect("rename");
        bus.seal();
        bus.execute(SetFrameDuration::new(FrameIndex::new(1), 70).into(), s, c)
            .expect("duration");
    }

    #[test]
    fn kinds_list_undo_oldest_first_and_redo_in_redo_order() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        three_steps(&mut bus, &mut s, &mut c);
        assert_eq!(
            ids(bus.undo_kinds()),
            ["add_frame", "rename_layer", "frame_duration"]
        );
        assert!(bus.undo(&mut s, &mut c));
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(ids(bus.undo_kinds()), ["add_frame"]);
        assert_eq!(ids(bus.redo_kinds()), ["rename_layer", "frame_duration"]);
    }

    #[test]
    fn execute_as_names_the_step_and_a_merge_keeps_the_top_kind() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        bus.execute_as(
            HistoryKind::CropToSelection,
            ReframeCanvas::new(Rect::new(0, 0, 2, 2)).into(),
            &mut s,
            &mut c,
        )
        .expect("crop");
        bus.execute_as(
            HistoryKind::Paste,
            SetFrameDuration::new(FrameIndex::new(0), 10).into(),
            &mut s,
            &mut c,
        )
        .expect("duration");
        // An unsealed duration change of the same frame merges into the
        // top: one entry, and it keeps its kind.
        bus.execute(
            SetFrameDuration::new(FrameIndex::new(0), 20).into(),
            &mut s,
            &mut c,
        )
        .expect("duration");
        assert_eq!(ids(bus.undo_kinds()), ["crop_to_selection", "paste"]);
    }

    #[test]
    fn revision_moves_with_the_step_list_but_not_with_a_merge() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        let r0 = bus.revision();
        bus.execute(
            SetFrameDuration::new(FrameIndex::new(0), 10).into(),
            &mut s,
            &mut c,
        )
        .expect("duration");
        let r1 = bus.revision();
        assert_ne!(r1, r0, "a new step");
        bus.execute(
            SetFrameDuration::new(FrameIndex::new(0), 20).into(),
            &mut s,
            &mut c,
        )
        .expect("duration");
        assert_eq!(bus.revision(), r1, "a merge keeps the list");
        assert!(bus.undo(&mut s, &mut c));
        let r2 = bus.revision();
        assert_ne!(r2, r1, "an undo");
        assert_eq!(bus.redo(&mut s, &mut c), Ok(true));
        assert_ne!(bus.revision(), r2, "a redo");
    }

    #[test]
    fn dropped_steps_is_set_once_the_cap_drops_the_oldest() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::with_capacity(2);
        for name in ["a", "b"] {
            bus.execute(
                SetLayerName::new(LayerId::new(0), name).into(),
                &mut s,
                &mut c,
            )
            .expect("rename");
        }
        assert!(!bus.dropped_steps());
        bus.execute(
            SetLayerName::new(LayerId::new(0), "c").into(),
            &mut s,
            &mut c,
        )
        .expect("rename");
        assert!(bus.dropped_steps());
        assert_eq!(bus.undo_depth(), 2);
    }

    #[test]
    fn jump_matches_the_same_number_of_undos_and_redos() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        let start = s.clone();
        three_steps(&mut bus, &mut s, &mut c);
        let end = s.clone();

        assert_eq!(bus.jump(-2, &mut s, &mut c), Ok(2));
        // Two plain undos on a copy of the document give the same state.
        let mut by_hand = Bus::new();
        let (mut s3, mut c3) = doc();
        three_steps(&mut by_hand, &mut s3, &mut c3);
        assert!(by_hand.undo(&mut s3, &mut c3));
        assert!(by_hand.undo(&mut s3, &mut c3));
        assert_eq!(s, s3);
        assert_eq!((bus.undo_depth(), bus.redo_depth()), (1, 2));

        assert_eq!(
            bus.jump(-9, &mut s, &mut c),
            Ok(1),
            "clamped to what is left"
        );
        assert_eq!(s, start);
        assert_eq!(bus.jump(9, &mut s, &mut c), Ok(3));
        assert_eq!(s, end);
        assert_eq!(bus.jump(0, &mut s, &mut c), Ok(0));
        assert_eq!(s, end);
    }

    #[test]
    fn jump_of_several_steps_reports_the_whole_canvas_dirty() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        three_steps(&mut bus, &mut s, &mut c);
        bus.jump(-2, &mut s, &mut c).expect("jump");
        assert_eq!(bus.last_dirty_region(), DirtyRegion::Canvas);
    }

    #[test]
    fn jump_stops_at_a_failing_redo_and_keeps_the_failed_step() {
        let (mut s, mut c) = doc();
        let mut bus = Bus::new();
        three_steps(&mut bus, &mut s, &mut c);
        assert_eq!(bus.jump(-2, &mut s, &mut c), Ok(2));
        // Frame 1 disappears behind the bus's back: redoing its duration fails.
        s.frames.truncate(1);
        let (moved, _) = bus.jump(2, &mut s, &mut c).expect_err("redo fails");
        assert_eq!(moved, 1, "the rename was redone");
        assert_eq!(
            s.layer(LayerId::new(0)).map(|l| l.name.as_str()),
            Some("ground")
        );
        assert_eq!(ids(bus.undo_kinds()), ["add_frame", "rename_layer"]);
        assert_eq!(ids(bus.redo_kinds()), ["frame_duration"], "no step lost");
    }

    #[test]
    fn default_kinds_name_the_tool_or_menu_command() {
        let layer = LayerId::new(0);
        let frame = FrameIndex::new(0);
        let brush = PixelBrush::new(1, BrushShape::Square);
        let stroke = |color: Rgba| {
            AnyCommand::from(PencilStroke::new(
                layer,
                frame,
                color,
                brush,
                false,
                crate::geometry::Point::new(0, 0),
                1.0,
            ))
        };
        let shading = AnyCommand::from(
            PencilStroke::new(
                layer,
                frame,
                Rgba::TRANSPARENT,
                brush,
                false,
                crate::geometry::Point::new(0, 0),
                1.0,
            )
            .with_ink(Ink::Shading {
                ramp: vec![Rgba::new(0, 0, 0, 255)],
                step: 1,
            }),
        );
        let cases: Vec<(AnyCommand, &str)> = vec![
            (stroke(Rgba::new(255, 0, 0, 255)), "pencil"),
            (stroke(Rgba::TRANSPARENT), "eraser"),
            (shading, "shading"),
            (
                TransformCel::new(layer, frame, Orientation::FlipVertical).into(),
                "flip_layer_vertical",
            ),
            (
                TransformCanvas::new(Orientation::Rotate90Ccw).into(),
                "rotate_canvas_90_ccw",
            ),
            (
                AddLayer::on_top(Layer::group(LayerId::new(5), "g")).into(),
                "add_group",
            ),
            (
                AddLayer::on_top(Layer::image(LayerId::new(6), "l")).into(),
                "add_layer",
            ),
        ];
        for (cmd, want) in cases {
            assert_eq!(cmd.kind().id(), want);
        }
    }
}
