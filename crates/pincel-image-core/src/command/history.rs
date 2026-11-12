//! What made an undo step: the kind every [`UndoStack`](super::UndoStack)
//! entry carries, so the History panel can name it (`docs/specs/history-panel.md`
//! AC-15, `docs/specs/pincel.md` §15 2026-11-05 "History entries carry a
//! kind from the core").

use std::borrow::Cow;

use pincel_effects::EFFECT_NAMES;

macro_rules! history_kinds {
    ($($(#[$doc:meta])* $variant:ident => $id:literal,)*) => {
        /// The action that made one undo step. The set is fixed: the
        /// variants below plus one [`HistoryKind::Effect`] per name in
        /// [`pincel_effects::EFFECT_NAMES`]; [`HistoryKind::all`] lists it.
        /// [`HistoryKind::id`] is the stable snake_case id hosts translate.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum HistoryKind {
            $($(#[$doc])* $variant,)*
            /// An effect or adjustment, by its wire name from
            /// [`pincel_effects::EFFECT_NAMES`] (id `effect.<name>`).
            Effect(&'static str),
        }

        impl HistoryKind {
            /// Every kind except the effects, in declaration order.
            const FIXED: &'static [HistoryKind] = &[$(HistoryKind::$variant,)*];

            /// The id of a fixed kind; empty for an effect, whose id
            /// [`HistoryKind::id`] builds.
            fn fixed_id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)*
                    Self::Effect(_) => "",
                }
            }
        }
    };
}

history_kinds! {
    /// Brush stroke.
    Brush => "brush",
    /// Eraser stroke.
    Eraser => "eraser",
    /// Recolor stroke.
    Recolor => "recolor",
    /// Smudge stroke.
    Smudge => "smudge",
    /// Clone Stamp stroke.
    CloneStamp => "clone_stamp",
    /// Paint bucket.
    Fill => "fill",
    /// Gradient tool.
    Gradient => "gradient",
    /// Line tool.
    Line => "line",
    /// Rectangle tool.
    Rectangle => "rectangle",
    /// Rounded rectangle tool.
    RoundedRectangle => "rounded_rectangle",
    /// Ellipse tool.
    Ellipse => "ellipse",
    /// Polygon tool.
    Polygon => "polygon",
    /// Triangle shape (docs/specs/image-shapes-curves-shapes.md).
    Triangle => "triangle",
    /// Diamond shape.
    Diamond => "diamond",
    /// Star shape.
    Star => "star",
    /// Block Arrow shape.
    BlockArrow => "block_arrow",
    /// Heart shape.
    Heart => "heart",
    /// Callout (speech bubble) shape.
    Callout => "callout",
    /// Text tool.
    Text => "text",
    /// Pixels written without a tool.
    DrawPixels => "draw_pixels",
    /// Move tool on a whole layer.
    TranslateLayer => "translate_layer",
    /// Moving the selected pixels.
    MoveSelection => "move_selection",
    /// Moving the selection outline only.
    MoveSelectionMask => "move_selection_mask",
    /// Edit ▸ Erase Selection.
    DeleteSelection => "delete_selection",
    /// Edit ▸ Fill Selection.
    FillSelection => "fill_selection",
    /// Edit ▸ Paste.
    Paste => "paste",
    /// A selection set without a selection tool.
    Select => "select",
    /// Rectangle select.
    SelectRectangle => "select_rectangle",
    /// Ellipse select.
    SelectEllipse => "select_ellipse",
    /// Lasso select.
    SelectLasso => "select_lasso",
    /// Magic wand.
    SelectMagicWand => "select_magic_wand",
    /// Edit ▸ Select All.
    SelectAll => "select_all",
    /// Edit ▸ Deselect.
    Deselect => "deselect",
    /// Edit ▸ Invert Selection.
    InvertSelection => "invert_selection",
    /// Expand selection.
    ExpandSelection => "expand_selection",
    /// Contract selection.
    ContractSelection => "contract_selection",
    /// Feather selection.
    FeatherSelection => "feather_selection",
    /// New layer.
    AddLayer => "add_layer",
    /// New group.
    AddGroup => "add_group",
    /// Delete layer.
    DeleteLayer => "delete_layer",
    /// Duplicate layer.
    DuplicateLayer => "duplicate_layer",
    /// Rename layer.
    RenameLayer => "rename_layer",
    /// Show / hide a layer.
    LayerVisibility => "layer_visibility",
    /// Lock / unlock a layer.
    LayerLock => "layer_lock",
    /// Layer opacity.
    LayerOpacity => "layer_opacity",
    /// Layer blend mode.
    LayerBlendMode => "layer_blend_mode",
    /// Several layer properties at once (MCP `layer.set`).
    LayerProperties => "layer_properties",
    /// Moving a layer in the stack.
    MoveLayer => "move_layer",
    /// Merge down.
    MergeDown => "merge_down",
    /// Merge group.
    MergeGroup => "merge_group",
    /// Merge visible.
    MergeVisible => "merge_visible",
    /// Flatten image.
    FlattenImage => "flatten_image",
    /// Layer ▸ Flip Horizontal.
    FlipLayerHorizontal => "flip_layer_horizontal",
    /// Layer ▸ Flip Vertical.
    FlipLayerVertical => "flip_layer_vertical",
    /// Layer ▸ Rotate 90° clockwise.
    RotateLayer90Cw => "rotate_layer_90_cw",
    /// Layer ▸ Rotate 90° counter-clockwise.
    RotateLayer90Ccw => "rotate_layer_90_ccw",
    /// Layer ▸ Rotate 180°.
    RotateLayer180 => "rotate_layer_180",
    /// Image ▸ Flip Horizontal.
    FlipCanvasHorizontal => "flip_canvas_horizontal",
    /// Image ▸ Flip Vertical.
    FlipCanvasVertical => "flip_canvas_vertical",
    /// Image ▸ Rotate 90° clockwise.
    RotateCanvas90Cw => "rotate_canvas_90_cw",
    /// Image ▸ Rotate 90° counter-clockwise.
    RotateCanvas90Ccw => "rotate_canvas_90_ccw",
    /// Image ▸ Rotate 180°.
    RotateCanvas180 => "rotate_canvas_180",
    /// Image ▸ Resize Canvas.
    ResizeCanvas => "resize_canvas",
    /// Image ▸ Crop to Selection.
    CropToSelection => "crop_to_selection",
    /// Image ▸ Auto Crop.
    AutoCrop => "auto_crop",
    /// Image ▸ Scale Image.
    ScaleImage => "scale_image",
    /// Every change of one answer of the Assistant chat
    /// (docs/specs/ai-chat-panel.md AC-08).
    Assistant => "assistant",
}

impl HistoryKind {
    /// The stable id: snake_case for the fixed kinds, `effect.<name>` for
    /// an effect.
    pub fn id(self) -> Cow<'static, str> {
        match self {
            Self::Effect(name) => Cow::Owned(format!("effect.{name}")),
            fixed => Cow::Borrowed(fixed.fixed_id()),
        }
    }

    /// The kind for the effect wire name `name`, `None` for a name outside
    /// [`pincel_effects::EFFECT_NAMES`].
    pub fn effect(name: &str) -> Option<Self> {
        EFFECT_NAMES
            .iter()
            .find(|n| **n == name)
            .map(|n| Self::Effect(n))
    }

    /// Every kind this core reports: the fixed kinds, then one effect kind
    /// per effect name.
    pub fn all() -> impl Iterator<Item = Self> {
        Self::FIXED
            .iter()
            .copied()
            .chain(EFFECT_NAMES.iter().map(|n| Self::Effect(n)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_ids_are_unique_snake_case() {
        let ids: Vec<String> = HistoryKind::all().map(|k| k.id().into_owned()).collect();
        let unique: HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "duplicate kind id");
        for id in &ids {
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.'),
                "{id}"
            );
        }
    }

    use crate::command::spec::{CommandSpec, StrokeScratch, run_command};
    use crate::command::{
        AddLayer, CommandBus, FlipCanvas, LayerTransform, RenameLayer, SetPixels, SetSelection,
        TransformLayer,
    };
    use crate::document::Document;
    use crate::geometry::Rect;

    fn ids(kinds: impl Iterator<Item = HistoryKind>) -> Vec<String> {
        kinds.map(|k| k.id().into_owned()).collect()
    }

    fn bus() -> CommandBus {
        CommandBus::new(Document::new(8, 8).expect("doc"))
    }

    /// Add a layer, rename it, flip the canvas: three steps.
    fn three_steps(bus: &mut CommandBus) {
        bus.apply(Box::new(AddLayer::above(0))).expect("add");
        bus.apply(Box::new(RenameLayer::new(1, "Sky")))
            .expect("rename");
        bus.apply(Box::new(FlipCanvas::new(true))).expect("flip");
    }

    #[test]
    fn kinds_list_undo_oldest_first_and_redo_in_redo_order() {
        let mut b = bus();
        three_steps(&mut b);
        run_command(
            &mut b,
            CommandSpec::SelectAll,
            &mut StrokeScratch::default(),
            &[],
        )
        .expect("select all");
        b.apply(Box::new(SetSelection::clear())).expect("deselect");
        b.apply(Box::new(TransformLayer::new(
            1,
            LayerTransform::FlipVertical,
        )))
        .expect("flip layer");
        assert_eq!(
            ids(b.history.undo_kinds()),
            [
                "add_layer",
                "rename_layer",
                "flip_canvas_horizontal",
                "select_all",
                "deselect",
                "flip_layer_vertical"
            ]
        );
        b.undo().expect("undo");
        b.undo().expect("undo");
        assert_eq!(
            ids(b.history.redo_kinds()),
            ["deselect", "flip_layer_vertical"]
        );
    }

    #[test]
    fn a_pixel_edit_reports_the_kind_it_was_given() {
        let mut b = bus();
        let patch = crate::ImageBuffer::new_transparent(2, 2);
        b.apply(Box::new(
            SetPixels::new(0, Rect::new(0, 0, 2, 2), patch)
                .with_kind(HistoryKind::effect("invert").expect("effect")),
        ))
        .expect("edit");
        assert_eq!(ids(b.history.undo_kinds()), ["effect.invert"]);
    }

    #[test]
    fn dropped_steps_is_set_when_the_capacity_or_the_byte_cap_drops_a_step() {
        let mut b = bus();
        b.history = crate::command::UndoStack::with_capacity(10);
        for i in 0..10 {
            b.apply(Box::new(RenameLayer::new(0, format!("n{i}"))))
                .expect("rename");
        }
        assert!(!b.history.dropped_steps());
        b.apply(Box::new(RenameLayer::new(0, "last")))
            .expect("rename");
        assert!(b.history.dropped_steps());

        let mut c = CommandBus::new(Document::new(64, 64).expect("doc"));
        c.history.set_byte_cap(64 * 64 * 4);
        for v in [40u8, 80] {
            assert!(!c.history.dropped_steps());
            let full = crate::ImageBuffer::from_raw(64, 64, vec![v; 64 * 64 * 4]).expect("buf");
            c.apply(Box::new(SetPixels::new(0, Rect::new(0, 0, 64, 64), full)))
                .expect("edit");
        }
        assert!(
            c.history.dropped_steps(),
            "the byte cap dropped the first edit"
        );
    }

    #[test]
    fn jump_matches_the_same_number_of_undos_and_redos() {
        let mut jumped = bus();
        let start = jumped.document.clone();
        three_steps(&mut jumped);
        let end = jumped.document.clone();
        jumped.undo().expect("undo");
        jumped.undo().expect("undo");
        let two_undos = jumped.document.clone();
        jumped.redo().expect("redo");
        jumped.redo().expect("redo");
        assert!(jumped.document == end);
        assert_eq!(jumped.jump(-2), Ok(2));
        assert!(jumped.document == two_undos);
        assert_eq!(
            (jumped.history.undo_depth(), jumped.history.redo_depth()),
            (1, 2)
        );
        assert_eq!(jumped.jump(-9), Ok(1), "clamped to what is left");
        assert!(jumped.document == start);
        assert_eq!(jumped.jump(9), Ok(3));
        assert!(jumped.document == end);
        assert_eq!(jumped.jump(0), Ok(0));
    }

    #[test]
    fn jump_stops_at_a_failing_step_and_keeps_it() {
        let mut b = bus();
        three_steps(&mut b);
        assert_eq!(b.jump(-2), Ok(2));
        // Layer 1 disappears behind the history's back: redoing its rename fails.
        b.document.layers.truncate(1);
        b.history = {
            let mut h = std::mem::take(&mut b.history);
            // Undo the add without touching the document so only the rename
            // and the flip remain redoable.
            let add = h.pop_undo().expect("add step");
            drop(add);
            h
        };
        let (moved, _) = b.jump(2).expect_err("the rename fails");
        assert_eq!(moved, 0);
        assert_eq!(
            ids(b.history.redo_kinds()),
            ["rename_layer", "flip_canvas_horizontal"],
            "no step lost"
        );
    }

    #[test]
    fn effect_maps_known_names_and_rejects_others() {
        assert_eq!(
            HistoryKind::effect("gaussian_blur").map(|k| k.id().into_owned()),
            Some("effect.gaussian_blur".to_string())
        );
        assert_eq!(HistoryKind::effect("no_such_effect"), None);
    }
}
