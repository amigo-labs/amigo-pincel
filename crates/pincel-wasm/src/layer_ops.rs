//! Layer property surface (Fineliner parity, spec §3.2): opacity, blend
//! mode, duplicate, merge down, flatten.
//!
//! Every mutator routes through the undo bus and emits `dirty-canvas`
//! (each one changes the whole composite). Blend modes cross the boundary
//! as the stable snake_case names from [`BlendMode::name`].

use pincel_core::{
    BlendMode, DuplicateLayer, FlattenImage, LayerId, MergeDown, SetLayerBlendMode, SetLayerOpacity,
};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

/// Snake_case names of every blend mode, in file-format order.
#[wasm_bindgen(js_name = blendModeNames)]
pub fn blend_mode_names() -> Vec<String> {
    BlendMode::ALL.iter().map(|m| m.name().to_owned()).collect()
}

#[wasm_bindgen]
impl Document {
    /// Blend mode name of the named layer (`"normal"`, `"multiply"`, …),
    /// or `"normal"` when the layer is unknown.
    #[wasm_bindgen(js_name = layerBlendMode)]
    pub fn layer_blend_mode(&self, layer_id: u32) -> String {
        self.sprite
            .layer(LayerId::new(layer_id))
            .map(|l| l.blend_mode)
            .unwrap_or_default()
            .name()
            .to_owned()
    }

    /// Set the named layer's opacity (`0..=255`), routed through the undo
    /// bus. Consecutive calls for the same layer merge into one undo
    /// entry until [`Document::end_stroke`] seals it (slider drags). Emits
    /// `dirty-canvas`. Errors when `layer_id` is unknown.
    #[wasm_bindgen(js_name = setLayerOpacity)]
    pub fn set_layer_opacity(&mut self, layer_id: u32, opacity: u8) -> Result<(), String> {
        let cmd = SetLayerOpacity::new(LayerId::new(layer_id), opacity);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to set layer opacity: {e}"))?;
        self.push_last_dirty();
        Ok(())
    }

    /// Set the named layer's blend mode by name (see [`blend_mode_names`]),
    /// routed through the undo bus. Emits `dirty-canvas`. Errors on an
    /// unknown layer or mode name.
    #[wasm_bindgen(js_name = setLayerBlendMode)]
    pub fn set_layer_blend_mode(&mut self, layer_id: u32, mode: &str) -> Result<(), String> {
        let mode =
            BlendMode::from_name(mode).ok_or_else(|| format!("unknown blend mode `{mode}`"))?;
        let cmd = SetLayerBlendMode::new(LayerId::new(layer_id), mode);
        self.bus.seal();
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to set layer blend mode: {e}"))?;
        self.bus.seal();
        self.push_last_dirty();
        Ok(())
    }

    /// Duplicate the named image / tilemap layer (with its cels) directly
    /// above itself and return the copy's id. One undo entry; emits
    /// `dirty-canvas`. Errors for an unknown layer or a group.
    #[wasm_bindgen(js_name = duplicateLayer)]
    pub fn duplicate_layer(&mut self, layer_id: u32) -> Result<u32, String> {
        let cmd = DuplicateLayer::new(LayerId::new(layer_id));
        self.bus.seal();
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to duplicate layer: {e}"))?;
        self.bus.seal();
        self.events.push(Event::dirty_canvas());
        // The command chose the id; it is the highest id in the stack now.
        self.sprite
            .layers
            .iter()
            .map(|l| l.id.0)
            .max()
            .ok_or_else(|| "duplicate produced no layer".to_owned())
    }

    /// Merge the named image layer into the image layer directly below it
    /// (same parent) and remove it. One undo entry; emits `dirty-canvas`.
    /// If the merged layer was the active paint target, the target moves
    /// to the layer it was merged into. Errors when there is no mergeable
    /// layer below.
    #[wasm_bindgen(js_name = mergeDown)]
    pub fn merge_down(&mut self, layer_id: u32) -> Result<(), String> {
        let id = LayerId::new(layer_id);
        let below = self
            .sprite
            .layers
            .iter()
            .position(|l| l.id == id)
            .and_then(|i| i.checked_sub(1))
            .map(|i| self.sprite.layers[i].id);
        let cmd = MergeDown::new(id);
        self.bus.seal();
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to merge down: {e}"))?;
        self.bus.seal();
        if self.active_layer == Some(id) {
            self.active_layer = below;
        }
        self.events.push(Event::dirty_canvas());
        Ok(())
    }

    /// Flatten every visible layer into one image layer (hidden layers are
    /// dropped) and return the new layer's id, which also becomes the
    /// active paint target. One undo entry; emits `dirty-canvas`.
    #[wasm_bindgen(js_name = flattenImage)]
    pub fn flatten_image(&mut self) -> Result<u32, String> {
        let cmd = FlattenImage::new();
        self.bus.seal();
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to flatten image: {e}"))?;
        self.bus.seal();
        let id = self
            .sprite
            .layers
            .first()
            .map(|l| l.id)
            .ok_or_else(|| "flatten produced no layer".to_owned())?;
        self.active_layer = Some(id);
        self.events.push(Event::dirty_canvas());
        Ok(id.0)
    }
}

impl Document {
    /// Relay the bus's last dirty region as an event, if any.
    fn push_last_dirty(&mut self) {
        if let Some(ev) = Event::from_dirty(self.bus.last_dirty_region()) {
            self.events.push(ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_mode_names_lists_all_modes() {
        let names = blend_mode_names();
        assert_eq!(names.len(), BlendMode::ALL.len());
        assert_eq!(names[0], "normal");
        assert_eq!(names[1], "multiply");
    }

    #[test]
    fn set_layer_opacity_round_trips_and_merges_slider_ticks() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.set_layer_opacity(0, 200).expect("set");
        doc.set_layer_opacity(0, 100).expect("set");
        assert_eq!(doc.layer_opacity(0), 100);
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.undo());
        assert_eq!(doc.layer_opacity(0), 255);
        assert!(doc.set_layer_opacity(9, 1).is_err());
    }

    #[test]
    fn set_layer_blend_mode_by_name_and_reject_unknown() {
        let mut doc = Document::new(4, 4).expect("dims");
        assert_eq!(doc.layer_blend_mode(0), "normal");
        doc.set_layer_blend_mode(0, "multiply").expect("set");
        assert_eq!(doc.layer_blend_mode(0), "multiply");
        assert!(doc.set_layer_blend_mode(0, "plasma").is_err());
        assert!(doc.undo());
        assert_eq!(doc.layer_blend_mode(0), "normal");
    }

    #[test]
    fn duplicate_layer_returns_new_id_and_copies_pixels() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 1, 1, 0xFF00FFFF).expect("paint");
        doc.end_stroke();
        let copy = doc.duplicate_layer(0).expect("dup");
        assert_eq!(copy, 1);
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.layer_name(copy), "Layer 1 copy");
        // Hide the source: the copy alone must still show the pixel.
        doc.set_layer_visible(0, false).expect("hide");
        assert_eq!(doc.pick_color(0, 1, 1).expect("pick"), 0xFF00FFFF);
    }

    #[test]
    fn merge_down_combines_layers_and_moves_active_target() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0xFF0000FF).expect("paint");
        doc.end_stroke();
        let top = doc.add_layer("top").expect("add");
        doc.set_active_layer(top);
        doc.apply_tool("pencil", 1, 0, 0x0000FFFF).expect("paint");
        doc.end_stroke();
        doc.merge_down(top).expect("merge");
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.paint_target_layer_id(), 0);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0xFF0000FF);
        assert_eq!(doc.pick_color(0, 1, 0).expect("pick"), 0x0000FFFF);
        assert!(doc.undo());
        assert_eq!(doc.layer_count(), 2);
        assert!(doc.merge_down(0).is_err(), "bottom layer has no target");
    }

    #[test]
    fn flatten_image_leaves_one_layer_with_the_composite() {
        let mut doc = Document::new(4, 4).expect("dims");
        doc.apply_tool("pencil", 0, 0, 0xFF0000FF).expect("paint");
        doc.end_stroke();
        let top = doc.add_layer("top").expect("add");
        doc.set_active_layer(top);
        doc.apply_tool("pencil", 0, 0, 0x00FF00FF).expect("paint");
        doc.end_stroke();
        let id = doc.flatten_image().expect("flatten");
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.paint_target_layer_id(), id);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0x00FF00FF);
        assert!(doc.undo());
        assert_eq!(doc.layer_count(), 2);
    }
}
