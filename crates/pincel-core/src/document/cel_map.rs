//! Per-`(layer, frame)` cel storage. See `docs/specs/pincel.md` §3.2.
//!
//! Cels are kept in a separate map rather than on `Sprite` so commands can
//! borrow the document and the cel store independently.

use std::collections::BTreeMap;

use super::cel::Cel;
use super::frame::FrameIndex;
use super::layer::LayerId;

/// Composite key identifying a cel.
pub type CelKey = (LayerId, FrameIndex);

/// Owning storage for cels, keyed by `(layer, frame)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CelMap {
    cels: BTreeMap<CelKey, Cel>,
}

impl CelMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of cels stored.
    pub fn len(&self) -> usize {
        self.cels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cels.is_empty()
    }

    /// Insert (or replace) a cel. Returns the previous cel if one existed at
    /// the same `(layer, frame)`.
    pub fn insert(&mut self, cel: Cel) -> Option<Cel> {
        self.cels.insert((cel.layer, cel.frame), cel)
    }

    /// Borrow the cel at `(layer, frame)`, if present.
    pub fn get(&self, layer: LayerId, frame: FrameIndex) -> Option<&Cel> {
        self.cels.get(&(layer, frame))
    }

    /// Mutably borrow the cel at `(layer, frame)`, if present.
    pub fn get_mut(&mut self, layer: LayerId, frame: FrameIndex) -> Option<&mut Cel> {
        self.cels.get_mut(&(layer, frame))
    }

    /// Remove and return the cel at `(layer, frame)`, if present.
    pub fn remove(&mut self, layer: LayerId, frame: FrameIndex) -> Option<Cel> {
        self.cels.remove(&(layer, frame))
    }

    /// Iterate cels in `(layer, frame)` key order.
    pub fn iter(&self) -> impl Iterator<Item = (&CelKey, &Cel)> {
        self.cels.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{ColorMode, PixelBuffer};

    fn sample_cel(layer: u32, frame: u32) -> Cel {
        Cel::image(
            LayerId::new(layer),
            FrameIndex::new(frame),
            PixelBuffer::empty(2, 2, ColorMode::Rgba),
        )
    }

    #[test]
    fn insert_and_get_round_trip() {
        let mut map = CelMap::new();
        let cel = sample_cel(1, 0);
        assert!(map.insert(cel.clone()).is_none());
        assert_eq!(map.get(LayerId::new(1), FrameIndex::new(0)), Some(&cel));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn insert_replaces_existing_cel() {
        let mut map = CelMap::new();
        map.insert(sample_cel(1, 0));
        let mut replacement = sample_cel(1, 0);
        replacement.opacity = 128;
        let prior = map.insert(replacement.clone()).expect("prior cel returned");
        assert_eq!(prior.opacity, 255);
        assert_eq!(
            map.get(LayerId::new(1), FrameIndex::new(0)),
            Some(&replacement)
        );
    }

    #[test]
    fn remove_returns_stored_cel() {
        let mut map = CelMap::new();
        map.insert(sample_cel(2, 1));
        let removed = map.remove(LayerId::new(2), FrameIndex::new(1));
        assert!(removed.is_some());
        assert!(map.is_empty());
    }
}
