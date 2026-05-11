//! In-process event queue exposed via [`Document::drain_events`].
//!
//! The boundary contract is documented in `docs/specs/pincel.md` §9.3:
//! the UI calls `drainEvents()` once per RAF tick and routes the
//! returned events into Svelte stores. Today only `dirty-rect` events
//! are produced (paint commands and undo / redo); `layer-changed`,
//! `palette-changed`, and `undo-pushed` land alongside the commands
//! that need them.
//!
//! The queue is bounded with drop-oldest semantics so a UI that stops
//! draining (e.g. a backgrounded tab) cannot grow the buffer without
//! limit. The cap is intentionally generous — a 60 fps painter
//! averaging one paint per frame can stall for ~17 s before any event
//! is dropped.

use std::collections::VecDeque;

use wasm_bindgen::prelude::*;

/// Maximum number of buffered events. Older events are dropped when
/// the cap is exceeded.
pub(crate) const DEFAULT_EVENT_CAP: usize = 1024;

/// Discriminant for [`Event`]. New kinds are appended without
/// renumbering so the JS-side `kind` strings stay stable across
/// versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    /// A region of a single cel changed and should be re-rendered.
    /// `layer`, `frame`, `x`, `y`, `width`, `height` describe the
    /// region in sprite space.
    DirtyRect,
    /// The entire canvas should be re-rendered (e.g. an undo /redo
    /// reverted a command and the WASM layer cannot yet attribute
    /// the change to a single cel). `layer`, `frame`, `x`, `y`,
    /// `width`, `height` are unspecified — consumers must not key
    /// off them.
    DirtyCanvas,
    /// The active marquee selection changed. `x`, `y`, `width`,
    /// `height` carry the new selection rect in sprite space; on a
    /// clear, all four are `0`. `layer` and `frame` are unspecified.
    /// The UI repaints the marching-ants overlay in response.
    SelectionChanged,
}

impl EventKind {
    fn as_str(self) -> &'static str {
        match self {
            EventKind::DirtyRect => "dirty-rect",
            EventKind::DirtyCanvas => "dirty-canvas",
            EventKind::SelectionChanged => "selection-changed",
        }
    }
}

/// Notification emitted by [`Document`](crate::Document).
///
/// Every variant carries the same shape so the JS class is uniform;
/// fields that do not apply to a given `kind` are zeroed. The
/// concrete schema per kind:
///
/// * `dirty-rect` — `layer`, `frame`, `x`, `y`, `width`, `height`
///   describe the changed region. Coordinates are in sprite space.
/// * `dirty-canvas` — the whole canvas should be re-rendered. All
///   numeric fields are `0` and have no meaning; consumers must not
///   key off them. Emitted by undo / redo until per-command dirty
///   tracking lands in M12.
/// * `selection-changed` — the active marquee selection was replaced
///   or cleared. `x`, `y`, `width`, `height` carry the new rect in
///   sprite space; all four are `0` when the selection was cleared.
///   `layer` / `frame` are unspecified.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct Event {
    kind: EventKind,
    layer: u32,
    frame: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[wasm_bindgen]
impl Event {
    /// Discriminant string for the event. See [`Event`] for the
    /// catalog of values.
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.kind.as_str().to_string()
    }

    /// Layer id the event is attached to, or `0` when not applicable.
    #[wasm_bindgen(getter)]
    pub fn layer(&self) -> u32 {
        self.layer
    }

    /// Frame index the event is attached to, or `0` when not applicable.
    #[wasm_bindgen(getter)]
    pub fn frame(&self) -> u32 {
        self.frame
    }

    /// Sprite-space x of the dirty region, or `0` when not applicable.
    #[wasm_bindgen(getter)]
    pub fn x(&self) -> i32 {
        self.x
    }

    /// Sprite-space y of the dirty region, or `0` when not applicable.
    #[wasm_bindgen(getter)]
    pub fn y(&self) -> i32 {
        self.y
    }

    /// Width of the dirty region in pixels, or `0` when not applicable.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the dirty region in pixels, or `0` when not applicable.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Event {
    /// Build a `dirty-rect` event covering the given sprite-space rect.
    pub(crate) fn dirty_rect(
        layer: u32,
        frame: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            kind: EventKind::DirtyRect,
            layer,
            frame,
            x,
            y,
            width,
            height,
        }
    }

    /// Build a `dirty-canvas` event signalling a full re-render.
    pub(crate) fn dirty_canvas() -> Self {
        Self {
            kind: EventKind::DirtyCanvas,
            layer: 0,
            frame: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }

    /// Build a `selection-changed` event. `x`, `y`, `width`, `height`
    /// describe the new selection rect, or are all `0` when the
    /// selection was cleared.
    pub(crate) fn selection_changed(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            kind: EventKind::SelectionChanged,
            layer: 0,
            frame: 0,
            x,
            y,
            width,
            height,
        }
    }
}

/// Drop-oldest ring buffer that backs [`Document::drain_events`].
#[derive(Debug)]
pub(crate) struct EventQueue {
    events: VecDeque<Event>,
    cap: usize,
}

impl EventQueue {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_EVENT_CAP)
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            events: VecDeque::new(),
            cap,
        }
    }

    pub fn push(&mut self, event: Event) {
        if self.cap == 0 {
            return;
        }
        if self.events.len() == self.cap {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn drain(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_rect_kind_string_matches_spec() {
        let ev = Event::dirty_rect(0, 0, 0, 0, 1, 1);
        assert_eq!(ev.kind(), "dirty-rect");
    }

    #[test]
    fn dirty_canvas_kind_string_matches_spec() {
        let ev = Event::dirty_canvas();
        assert_eq!(ev.kind(), "dirty-canvas");
        assert_eq!(ev.layer(), 0);
        assert_eq!(ev.frame(), 0);
        assert_eq!(ev.width(), 0);
        assert_eq!(ev.height(), 0);
    }

    #[test]
    fn selection_changed_kind_string_and_fields() {
        let ev = Event::selection_changed(3, 4, 10, 6);
        assert_eq!(ev.kind(), "selection-changed");
        assert_eq!(ev.x(), 3);
        assert_eq!(ev.y(), 4);
        assert_eq!(ev.width(), 10);
        assert_eq!(ev.height(), 6);
        assert_eq!(ev.layer(), 0);
        assert_eq!(ev.frame(), 0);
    }

    #[test]
    fn selection_changed_with_cleared_rect_zeros_fields() {
        let ev = Event::selection_changed(0, 0, 0, 0);
        assert_eq!(ev.kind(), "selection-changed");
        assert_eq!(ev.width(), 0);
        assert_eq!(ev.height(), 0);
    }

    #[test]
    fn queue_drains_in_fifo_order() {
        let mut q = EventQueue::new();
        q.push(Event::dirty_rect(1, 0, 0, 0, 1, 1));
        q.push(Event::dirty_rect(2, 0, 0, 0, 1, 1));
        let drained = q.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].layer(), 1);
        assert_eq!(drained[1].layer(), 2);
        assert!(q.drain().is_empty());
    }

    #[test]
    fn queue_drops_oldest_when_capped() {
        let mut q = EventQueue::with_capacity(2);
        q.push(Event::dirty_rect(1, 0, 0, 0, 1, 1));
        q.push(Event::dirty_rect(2, 0, 0, 0, 1, 1));
        q.push(Event::dirty_rect(3, 0, 0, 0, 1, 1));
        assert_eq!(q.len(), 2);
        let drained = q.drain();
        assert_eq!(drained[0].layer(), 2);
        assert_eq!(drained[1].layer(), 3);
    }

    #[test]
    fn zero_capacity_disables_queue() {
        let mut q = EventQueue::with_capacity(0);
        q.push(Event::dirty_rect(1, 0, 0, 0, 1, 1));
        assert!(q.drain().is_empty());
    }
}
