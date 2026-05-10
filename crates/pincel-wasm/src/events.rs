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

/// Discriminant for [`Event`]. Today only one variant ships; new
/// kinds are appended without renumbering so the JS-side `kind`
/// strings stay stable across versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    /// A region of a cel changed and should be re-rendered.
    DirtyRect,
}

impl EventKind {
    fn as_str(self) -> &'static str {
        match self {
            EventKind::DirtyRect => "dirty-rect",
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
