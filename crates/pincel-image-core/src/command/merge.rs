//! Layer-flattening commands: `MergeDown`, `MergeVisible`, `FlattenImage`
//! (spec §5.2 / §7.3).
//!
//! Each bakes several layers into one. `MergeVisible`/`FlattenImage` produce a
//! plain raster layer (Normal blend, opacity 1.0) since their pixels encode
//! the full composite; `MergeDown` keeps the lower layer's blend mode and
//! opacity, which relate it to the layers *beneath* the pair. For undo,
//! `MergeDown` keeps the removed upper layer and the lower layer's changed
//! tiles (docs/specs/performance-image-undo-memory.md); the others replace
//! several layers at once and snapshot the prior layer stack and active
//! index, restoring them verbatim (spec §7.1).

use super::Command;
use super::HistoryKind;
use super::snapshot::{DocSnapshot, restore, snapshot, take_snapshot};
use super::tiles::LayerTiles;
use crate::color::{BlendMode, Color};
use crate::document::{Document, ImageBuffer, Layer, LayerKind};
use crate::error::DocumentError;
use crate::render::{compose, compose_over};
use std::any::Any;
use uuid::Uuid;

/// Merges the layer at `index` onto the layer directly below it.
///
/// The upper layer is baked (respecting its opacity and blend mode) onto the
/// lower layer's raw pixels; the merged layer keeps the lower layer's id,
/// name, blend mode, opacity, visibility and lock, since those relate it to
/// the layers beneath the pair. Where the upper layer is transparent the
/// document composite is unchanged. Errors if `index` is 0 (no layer below)
/// or out of range.
pub struct MergeDown {
    index: usize,
    before: Option<MergeDownUndo>,
}

/// What undoing a merge-down needs: the upper layer it removed, the lower
/// layer's changed tiles and the active index — not the whole document
/// (docs/specs/performance-image-undo-memory.md).
struct MergeDownUndo {
    upper: Layer,
    lower: LayerTiles,
    active: usize,
}

impl MergeDown {
    /// Merges the layer at `index` down onto `index - 1`.
    pub fn at(index: usize) -> Self {
        Self {
            index,
            before: None,
        }
    }
}

impl Command for MergeDown {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let len = doc.layer_count();
        if self.index == 0 || self.index >= len {
            return Err(DocumentError::LayerIndexOutOfBounds {
                index: self.index,
                len,
            });
        }
        // Groups have no pixels to merge, and merging across a group
        // boundary would change what the group composites
        // (docs/specs/image-layer-groups.md AC-13).
        for i in [self.index - 1, self.index] {
            if doc.layers()[i].is_group() {
                return Err(DocumentError::GroupLayer { index: i });
            }
        }
        if doc.layers()[self.index - 1].parent != doc.layers()[self.index].parent {
            return Err(DocumentError::MergeAcrossGroups);
        }
        let active = doc.active_layer_index();
        // Bake the upper layer onto the lower one's raw pixels. The lower
        // layer's own blend mode/opacity must not be baked in — they apply
        // against the layers beneath and stay on the merged layer instead.
        // The pixels move into `base` and back, so no layer is copied.
        let lower = &mut doc.layers[self.index - 1];
        let pixels = std::mem::replace(&mut lower.pixels, ImageBuffer::new_transparent(0, 0));
        let base = Layer {
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            visible: true,
            pixels,
            ..lower.clone()
        };
        let upper = doc.layers.remove(self.index);
        let pair = [base, upper];
        let merged = compose(&pair);
        let [base, upper] = pair;
        let tiles = LayerTiles::diff(&base.pixels, &merged);
        doc.layers[self.index - 1].pixels = merged;
        self.before = Some(MergeDownUndo {
            upper,
            lower: tiles,
            active,
        });
        doc.set_active_layer(self.index - 1)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let undo = self.before.take().ok_or(DocumentError::RegionOutOfBounds)?;
        let MergeDownUndo {
            upper,
            mut lower,
            active,
        } = undo;
        let len = doc.layers.len();
        let target =
            doc.layers
                .get_mut(self.index - 1)
                .ok_or(DocumentError::LayerIndexOutOfBounds {
                    index: self.index - 1,
                    len,
                })?;
        lower.swap(&mut target.pixels);
        doc.insert_layer(self.index, upper)?;
        doc.set_active_layer(active)
    }

    fn undo_bytes(&self) -> usize {
        self.before
            .as_ref()
            .map_or(0, |u| u.upper.pixels.data().len() + u.lower.bytes())
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::MergeDown
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// [`compose`] of `layers`, or a transparent canvas when they hold no raster
/// layer (an empty group).
fn canvas_composite(doc: &Document, layers: &[Layer]) -> ImageBuffer {
    if layers.iter().all(|l| l.is_group()) {
        return ImageBuffer::new_transparent(doc.canvas.width(), doc.canvas.height());
    }
    compose(layers)
}

/// Replaces the group at `index` and its subtree with one raster layer at
/// the group's place (Layer ▸ Merge Group, docs/specs/image-layer-groups.md
/// AC-14).
///
/// The raster layer holds the isolated composite of the group's children
/// and keeps the group's id, name, parent, opacity, blend mode, visibility
/// and lock, so the document composite is unchanged. Errors with
/// `NotAGroup` on a raster layer.
pub struct MergeGroup {
    index: usize,
    before: Option<DocSnapshot>,
}

impl MergeGroup {
    /// Merges the group at `index`.
    pub fn at(index: usize) -> Self {
        Self {
            index,
            before: None,
        }
    }
}

impl Command for MergeGroup {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let group = doc
            .layers()
            .get(self.index)
            .ok_or(DocumentError::LayerIndexOutOfBounds {
                index: self.index,
                len: doc.layer_count(),
            })?;
        if !group.is_group() {
            return Err(DocumentError::NotAGroup { index: self.index });
        }
        let end = doc.subtree_end(self.index);
        let pixels = canvas_composite(doc, &doc.layers()[self.index + 1..end]);
        let merged = Layer {
            kind: LayerKind::Raster,
            pixels,
            ..group.clone()
        };
        self.before = Some(snapshot(doc));
        doc.layers.splice(self.index..end, [merged]);
        doc.set_active_layer(self.index)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let snap = self.before.take().ok_or(DocumentError::RegionOutOfBounds)?;
        restore(doc, snap)
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::MergeGroup
    }

    fn undo_bytes(&self) -> usize {
        self.before.as_ref().map_or(0, DocSnapshot::bytes)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Flattens all visible layers into a single layer, leaving hidden layers
/// untouched.
///
/// The merged layer is placed at the position of the lowest visible layer. A
/// no-op if no layer is visible.
pub struct MergeVisible {
    before: Option<DocSnapshot>,
    /// Id of the merged layer, fixed on first apply so redo recreates it
    /// exactly.
    id: Option<Uuid>,
}

impl MergeVisible {
    /// Creates the command.
    pub fn new() -> Self {
        Self {
            before: None,
            id: None,
        }
    }
}

impl Default for MergeVisible {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for MergeVisible {
    /// Merges every visible root-level layer — a group with its whole
    /// subtree — into one; hidden root-level layers and groups stay.
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        // The root-level runs `(start, end)` of whole subtrees, bottom first.
        let mut roots = Vec::new();
        let mut i = 0;
        while i < doc.layer_count() {
            let end = doc.subtree_end(i);
            roots.push((i, end));
            i = end;
        }
        let visible: Vec<(usize, usize)> = roots
            .iter()
            .copied()
            .filter(|&(s, _)| doc.layers()[s].visible)
            .collect();
        if visible.is_empty() {
            return Ok(()); // nothing visible to merge
        }
        self.before = Some(snapshot(doc));

        let visible_layers: Vec<Layer> = visible
            .iter()
            .flat_map(|&(s, e)| doc.layers()[s..e].iter().cloned())
            .collect();
        let mut merged = Layer::from_pixels("Merged", canvas_composite(doc, &visible_layers));
        merged.id = *self.id.get_or_insert(merged.id);

        // Keep the hidden runs in order and put the merged layer where the
        // lowest visible run was; everything below it is hidden.
        let lowest = visible[0].0;
        let mut kept = Vec::with_capacity(doc.layer_count());
        let mut merged = Some(merged);
        for &(s, e) in &roots {
            if doc.layers()[s].visible {
                kept.extend(merged.take());
            } else {
                kept.extend(doc.layers()[s..e].iter().cloned());
            }
        }
        doc.layers = kept;
        doc.set_active_layer(lowest)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        match self.before.take() {
            Some(snap) => restore(doc, snap),
            None => Ok(()), // apply was a no-op
        }
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::MergeVisible
    }

    fn undo_bytes(&self) -> usize {
        self.before.as_ref().map_or(0, DocSnapshot::bytes)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Flattens every layer onto an opaque white background, producing a single
/// fully opaque "Background" layer (spec §5.2).
pub struct FlattenImage {
    before: Option<DocSnapshot>,
    /// Id of the flattened layer, fixed on first apply so redo recreates
    /// it exactly.
    id: Option<Uuid>,
}

impl FlattenImage {
    /// Creates the command.
    pub fn new() -> Self {
        Self {
            before: None,
            id: None,
        }
    }
}

impl Default for FlattenImage {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for FlattenImage {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let flattened = compose_over(doc.layers(), Color::WHITE);
        let mut layer = Layer::from_pixels("Background", flattened);
        layer.id = *self.id.get_or_insert(layer.id);
        // The old layers move into the snapshot instead of being copied.
        self.before = Some(take_snapshot(doc, vec![layer]));
        doc.set_active_layer(0)
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let snap = self.before.take().ok_or(DocumentError::RegionOutOfBounds)?;
        restore(doc, snap)
    }

    fn kind(&self) -> HistoryKind {
        HistoryKind::FlattenImage
    }

    fn undo_bytes(&self) -> usize {
        self.before.as_ref().map_or(0, DocSnapshot::bytes)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(doc: &mut Document, index: usize, c: Color) {
        let buf = &mut doc.layers[index].pixels;
        for y in 0..buf.height() {
            for x in 0..buf.width() {
                buf.set_pixel(x, y, c);
            }
        }
    }

    fn composites_equal(a: &Document, b: &Document) -> bool {
        compose(a.layers()).data() == compose(b.layers()).data()
    }

    use crate::command::CommandBus;
    use crate::document::tree::tests::{names, tree_doc};

    /// bg, G (50 %, Multiply) { a, H (hidden) { b } , c }, top — each raster
    /// layer painted with its own partly transparent pattern.
    fn grouped_doc() -> Document {
        let mut doc = tree_doc(&[
            ("bg", None, false),
            ("G", None, true),
            ("a", Some("G"), false),
            ("H", Some("G"), true),
            ("b", Some("H"), false),
            ("c", Some("G"), false),
            ("top", None, false),
        ]);
        for (i, layer) in doc.layers.iter_mut().enumerate() {
            if layer.is_group() {
                continue;
            }
            let n = i as u8;
            for y in 0..4 {
                for x in 0..4 {
                    let a = if (x + y + u32::from(n)) % 3 == 0 {
                        0
                    } else {
                        90 + n * 20
                    };
                    layer.pixels.set_pixel(
                        x,
                        y,
                        Color::rgba(n * 30, x as u8 * 60, y as u8 * 50, a),
                    );
                }
            }
        }
        doc.layers[1].opacity = 0.5;
        doc.layers[1].blend_mode = BlendMode::Multiply;
        doc.layers[3].visible = false;
        doc.layers[5].blend_mode = BlendMode::Screen;
        doc
    }

    #[test]
    fn merge_down_with_a_group_or_across_parents_is_refused() {
        let doc = grouped_doc();
        let before = doc.layers().to_vec();
        let mut bus = CommandBus::new(doc);
        // a onto the group G, G onto bg, top onto c (another parent), the
        // group H onto a, and b onto the group H.
        for index in [2, 1, 6, 3, 4] {
            assert!(
                bus.apply(Box::new(MergeDown::at(index))).is_err(),
                "{index}"
            );
        }
        assert_eq!(bus.document.layers(), before.as_slice());
        // Two siblings in the same group still merge.
        bus.apply(Box::new(MergeDown::at(1)))
            .expect_err("group refused");
        let mut doc = tree_doc(&[
            ("G", None, true),
            ("a", Some("G"), false),
            ("b", Some("G"), false),
        ]);
        doc.set_active_layer(2).unwrap();
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(MergeDown::at(2))).unwrap();
        assert_eq!(names(&bus.document), ["G", "a"]);
        assert_eq!(bus.document.parent_index(1), Some(0));
    }

    #[test]
    fn merge_group_matches_composite() {
        let doc = grouped_doc();
        let before = doc.layers().to_vec();
        let composite = compose(doc.layers());
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(MergeGroup::at(1))).unwrap();
        let doc = &bus.document;
        assert_eq!(names(doc), ["bg", "G", "top"]);
        assert!(!doc.layers()[1].is_group());
        assert_eq!(doc.active_layer_index(), 1);
        assert_eq!(compose(doc.layers()), composite);
        assert!(bus.undo().unwrap());
        assert_eq!(bus.document.layers(), before.as_slice());
    }

    #[test]
    fn merge_group_of_a_nested_group_keeps_its_place_and_undo_restores_it() {
        let doc = grouped_doc();
        let before = doc.layers().to_vec();
        let composite = compose(doc.layers());
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(MergeGroup::at(3))).unwrap();
        let doc = &bus.document;
        assert_eq!(names(doc), ["bg", "G", "a", "H", "c", "top"]);
        assert_eq!(doc.parent_index(3), Some(1), "still inside G");
        assert!(doc.is_well_formed());
        assert_eq!(compose(doc.layers()), composite);
        assert!(bus.undo().unwrap());
        assert_eq!(bus.document.layers(), before.as_slice());
    }

    #[test]
    fn merge_group_on_a_raster_layer_is_refused() {
        let mut bus = CommandBus::new(grouped_doc());
        let err = bus.apply(Box::new(MergeGroup::at(0))).unwrap_err();
        assert_eq!(err, DocumentError::NotAGroup { index: 0 });
    }

    #[test]
    fn merge_visible_with_groups_preserves_composite() {
        let mut doc = grouped_doc();
        doc.layers[6].visible = false; // a hidden root layer stays
        let composite = compose(doc.layers());
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(MergeVisible::new())).unwrap();
        assert_eq!(compose(bus.document.layers()), composite);
        assert_eq!(names(&bus.document), ["Merged", "top"]);
        assert!(bus.document.is_well_formed());
    }

    #[test]
    fn flatten_image_with_groups_preserves_composite() {
        let doc = grouped_doc();
        let expected = compose_over(doc.layers(), Color::WHITE);
        let mut bus = CommandBus::new(doc);
        bus.apply(Box::new(FlattenImage::new())).unwrap();
        assert_eq!(bus.document.layer_count(), 1);
        assert_eq!(bus.document.layers()[0].pixels, expected);
        assert_eq!(compose(bus.document.layers()), expected);
    }

    #[test]
    fn merge_down_combines_two_layers_into_one() {
        let mut doc = Document::new(2, 2).unwrap();
        solid(&mut doc, 0, Color::rgba(0, 0, 255, 255)); // blue bottom
        doc.add_layer("Top").unwrap();
        solid(&mut doc, 1, Color::rgba(255, 0, 0, 255)); // opaque red top

        let mut cmd = MergeDown::at(1);
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layer_count(), 1);
        // Opaque red over blue → red.
        assert_eq!(
            doc.layers()[0].pixels.get_pixel(0, 0),
            Some(Color::rgba(255, 0, 0, 255))
        );
        assert_eq!(doc.active_layer_index(), 0);
    }

    #[test]
    fn merge_down_round_trip_restores_stack() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.add_layer("Top").unwrap();
        let before = compose(doc.layers()).into_raw();
        let mut cmd = MergeDown::at(1);
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(compose(doc.layers()).into_raw(), before);
    }

    #[test]
    fn merge_down_preserves_lower_layer_blend_mode_and_opacity() {
        // [white bottom (Normal), red (Multiply), sparse top]: after merging the
        // top down, the red layer's Multiply relationship to the bottom layer
        // must survive — the merged layer inherits the lower layer's properties.
        let mut doc = Document::new(2, 2).unwrap();
        solid(&mut doc, 0, Color::rgba(200, 200, 200, 255));
        doc.add_layer("Red").unwrap();
        solid(&mut doc, 1, Color::rgba(255, 0, 0, 255));
        doc.layers[1].blend_mode = BlendMode::Multiply;
        doc.layers[1].opacity = 0.8;
        doc.layers[1].locked = true;
        doc.add_layer("Top").unwrap();
        // Top paints a single pixel; everywhere else it is transparent.
        doc.layers[2]
            .pixels
            .set_pixel(0, 0, Color::rgba(0, 255, 0, 255));

        let before = compose(doc.layers()).into_raw();
        let mut cmd = MergeDown::at(2);
        cmd.apply(&mut doc).unwrap();

        assert_eq!(doc.layer_count(), 2);
        let merged = &doc.layers()[1];
        assert_eq!(merged.blend_mode, BlendMode::Multiply);
        assert_eq!(merged.opacity, 0.8);
        assert!(merged.locked);
        // Where the top layer was transparent the composite is unchanged.
        let after = compose(doc.layers()).into_raw();
        assert_eq!(after[4..], before[4..], "multiply region must be preserved");
    }

    #[test]
    fn merge_down_rejects_bottom_layer() {
        let mut doc = Document::new(2, 2).unwrap();
        assert!(MergeDown::at(0).apply(&mut doc).is_err());
    }

    #[test]
    fn merge_visible_preserves_composite_and_keeps_hidden_layers() {
        let mut doc = Document::new(2, 2).unwrap();
        solid(&mut doc, 0, Color::rgba(0, 0, 255, 255));
        doc.add_layer("Hidden").unwrap();
        solid(&mut doc, 1, Color::rgba(0, 255, 0, 255));
        doc.layers[1].visible = false; // hidden middle layer
        doc.add_layer("Top").unwrap();
        solid(&mut doc, 2, Color::rgba(255, 0, 0, 128)); // 50% red top

        let expected = doc.clone();
        let mut cmd = MergeVisible::new();
        cmd.apply(&mut doc).unwrap();
        // Two visible layers merged into one; the hidden layer survives.
        assert_eq!(doc.layer_count(), 2);
        assert!(doc.layers().iter().any(|l| !l.visible));
        assert!(composites_equal(&doc, &expected));
    }

    #[test]
    fn merge_visible_no_op_when_all_hidden() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.layers[0].visible = false;
        let mut cmd = MergeVisible::new();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layer_count(), 1);
    }

    #[test]
    fn flatten_produces_single_opaque_layer() {
        let mut doc = Document::new(2, 2).unwrap();
        solid(&mut doc, 0, Color::rgba(255, 0, 0, 128)); // 50% red over white
        let mut cmd = FlattenImage::new();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layer_count(), 1);
        // Every pixel is fully opaque after flattening onto white.
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(doc.layers()[0].pixels.get_pixel(x, y).unwrap().a, 255);
            }
        }
    }

    #[test]
    fn flatten_round_trip_restores_stack() {
        let mut doc = Document::new(2, 2).unwrap();
        doc.add_layer("Top").unwrap();
        solid(&mut doc, 1, Color::rgba(0, 255, 0, 200));
        let before = compose(doc.layers()).into_raw();
        let mut cmd = FlattenImage::new();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(compose(doc.layers()).into_raw(), before);
    }

    #[test]
    fn flatten_composite_matches_flattened_layer() {
        // A flattened doc's composite equals its single opaque layer's pixels.
        let mut doc = Document::new(2, 2).unwrap();
        solid(&mut doc, 0, Color::rgba(128, 64, 32, 180));
        let mut cmd = FlattenImage::new();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(compose(doc.layers()).data(), doc.layers()[0].pixels.data());
    }
}
