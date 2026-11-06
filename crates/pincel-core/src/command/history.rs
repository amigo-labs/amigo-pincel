//! What made an undo step: the kind every [`Bus`](super::Bus) entry
//! carries, so the History panel can name it (`docs/specs/history-panel.md`
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
    /// Pencil stroke or click.
    Pencil => "pencil",
    /// Eraser stroke or click.
    Eraser => "eraser",
    /// Pencil stroke with the Shading ink.
    Shading => "shading",
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
    /// Paint bucket.
    Fill => "fill",
    /// Gradient tool.
    Gradient => "gradient",
    /// Text tool.
    Text => "text",
    /// Moving the selected pixels.
    MoveSelection => "move_selection",
    /// Edit ▸ Delete (clear the selection).
    DeleteSelection => "delete_selection",
    /// Edit ▸ Cut.
    Cut => "cut",
    /// Edit ▸ Paste.
    Paste => "paste",
    /// Pixels written without a tool (MCP `draw.pixels`, `draw.image`).
    DrawPixels => "draw_pixels",
    /// Painting a tile in the tileset.
    TilePixel => "tile_pixel",
    /// Placing a tile on a tilemap layer.
    PlaceTile => "place_tile",
    /// New layer.
    AddLayer => "add_layer",
    /// New group.
    AddGroup => "add_group",
    /// New tilemap layer.
    AddTilemapLayer => "add_tilemap_layer",
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
    /// Moving a layer in the stack.
    MoveLayer => "move_layer",
    /// Merge down.
    MergeDown => "merge_down",
    /// Flatten image.
    FlattenImage => "flatten_image",
    /// New frame.
    AddFrame => "add_frame",
    /// Duplicate frame (a new frame with copies of the cels).
    DuplicateFrame => "duplicate_frame",
    /// Delete frame.
    DeleteFrame => "delete_frame",
    /// Frame duration.
    FrameDuration => "frame_duration",
    /// New tag.
    AddTag => "add_tag",
    /// Delete tag.
    DeleteTag => "delete_tag",
    /// Tag frame range.
    TagRange => "tag_range",
    /// Rename tag.
    RenameTag => "rename_tag",
    /// Tag direction.
    TagDirection => "tag_direction",
    /// Palette entry color.
    PaletteColor => "palette_color",
    /// New palette entry.
    AddPaletteEntry => "add_palette_entry",
    /// Delete palette entry.
    DeletePaletteEntry => "delete_palette_entry",
    /// Move palette entry.
    MovePaletteEntry => "move_palette_entry",
    /// Load or replace the palette.
    LoadPalette => "load_palette",
    /// New tileset.
    AddTileset => "add_tileset",
    /// New tile.
    AddTile => "add_tile",
    /// New slice.
    AddSlice => "add_slice",
    /// Delete slice.
    DeleteSlice => "delete_slice",
    /// Slice bounds / pivot / nine-slice.
    SliceKey => "slice_key",
    /// Grid settings.
    Grid => "grid",
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
    /// Image ▸ Scale Image.
    ScaleImage => "scale_image",
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

    #[test]
    fn effect_maps_known_names_and_rejects_others() {
        assert_eq!(
            HistoryKind::effect("gaussian_blur").map(|k| k.id().into_owned()),
            Some("effect.gaussian_blur".to_string())
        );
        assert_eq!(HistoryKind::effect("no_such_effect"), None);
    }
}
