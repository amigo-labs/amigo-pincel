//! Copy / Cut / Copy Merged / Paste (spec §5.4 Paste, Decision Log
//! 2026-10-29). The UI writes the PNG to the system clipboard and keeps the
//! in-app record; this module only turns document pixels into PNG bytes
//! and PNG bytes into a pasted layer.

use pincel_core::{
    ClearRegion, CopySource, LayerId, LayerKind, PasteLayer, PastedImage, copy_pixels, encode_png,
    import_png,
};
use pincel_core::{FrameIndex, Layer};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

/// What Copy / Cut produced: the PNG and the canvas position it came from.
#[wasm_bindgen]
pub struct CopiedImage {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    png: Vec<u8>,
}

#[wasm_bindgen]
impl CopiedImage {
    /// Sprite-space `x` of the copied region (its paste-in-place origin).
    #[wasm_bindgen(getter)]
    pub fn x(&self) -> i32 {
        self.x
    }

    /// Sprite-space `y` of the copied region.
    #[wasm_bindgen(getter)]
    pub fn y(&self) -> i32 {
        self.y
    }

    /// Width of the copied region in pixels.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the copied region in pixels.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The copied pixels as RGBA8 PNG bytes.
    pub fn png(&self) -> Box<[u8]> {
        self.png.clone().into_boxed_slice()
    }
}

/// `"{w}x{h}:{hash}"` of decoded RGBA8 pixels, hashed premultiplied so a
/// clipboard that re-encodes the PNG (Chromium does) keeps the key.
pub(crate) fn rgba_key(width: u32, height: u32, rgba: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |b: u8| {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    };
    for px in rgba.as_chunks::<4>().0 {
        let a = u32::from(px[3]);
        for &c in &px[..3] {
            feed(((u32::from(c) * a + 127) / 255) as u8);
        }
        feed(px[3]);
    }
    format!("{width}x{height}:{hash:016x}")
}

#[wasm_bindgen]
impl Document {
    /// Copy (spec §5.4): the active layer at the current frame — with
    /// `merged`, the composite of all visible layers — cropped to the
    /// selection's bounding box (clipped to the canvas; the whole canvas
    /// without a selection), pixels outside a shaped selection
    /// transparent. A group or tilemap layer copies its rendered pixels.
    /// The document is unchanged.
    #[wasm_bindgen(js_name = copySelection)]
    pub fn copy_selection(&self, merged: bool) -> Result<CopiedImage, String> {
        let source = if merged {
            CopySource::Merged
        } else {
            CopySource::Layer(self.copy_layer()?)
        };
        self.copy_from(source)
    }

    /// Cut (spec §5.4): what [`Self::copy_selection`] returns for the
    /// active layer, then clears those pixels (the whole layer without a
    /// selection) in one undo step. Refused — document unchanged — for a
    /// group or tilemap layer and for a locked layer.
    #[wasm_bindgen(js_name = cutSelection)]
    pub fn cut_selection(&mut self) -> Result<CopiedImage, String> {
        let layer = self.copy_layer()?;
        if !matches!(
            self.sprite.layer(layer).map(|l| &l.kind),
            Some(LayerKind::Image)
        ) {
            return Err("cut needs an image layer".to_string());
        }
        if self.sprite.layer_locked(layer) {
            return Err(format!("layer {} is locked", layer.0));
        }
        let copied = self.copy_from(CopySource::Layer(layer))?;
        let rect = pincel_core::Rect::new(copied.x, copied.y, copied.width, copied.height);
        let mut cmd = ClearRegion::new(layer, self.current_frame, rect);
        if self.sprite.has_selection() {
            cmd = cmd.masked();
        }
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to cut: {e}"))?;
        self.bus.seal();
        if let Some(ev) = Event::from_dirty(self.bus.last_dirty_region()) {
            self.events.push(ev);
        }
        Ok(copied)
    }

    /// Paste (spec §5.4): decode PNG `bytes` onto a new image layer above
    /// the active one, on the current frame, top-left at sprite `(x, y)`.
    /// With `expand` the canvas first grows (anchored top-left) to cover
    /// the image there; otherwise the image is cropped to the canvas. One
    /// undo step. Returns the new layer's id, which becomes active.
    /// Refused for an indexed sprite (writing indexed cels is Phase 2).
    #[wasm_bindgen(js_name = pasteLayer)]
    pub fn paste_layer(
        &mut self,
        bytes: &[u8],
        x: i32,
        y: i32,
        expand: bool,
    ) -> Result<u32, String> {
        let name = format!("Layer {}", self.sprite.layers.len() + 1);
        let anchor = self.copy_layer().ok();
        self.paste_image(bytes, &name, x, y, expand, anchor)
            .map_err(|e| format!("failed to paste: {e}"))
    }

    /// Clipboard identity of encoded PNG `bytes`: size plus a hash of the
    /// decoded pixels, equal for two encodings of the same image. Tells
    /// Pincel's own copy apart on the system clipboard.
    #[wasm_bindgen(js_name = imageKey)]
    pub fn image_key(bytes: &[u8]) -> Result<String, String> {
        let img = import_png(bytes).map_err(|e| format!("failed to decode PNG: {e}"))?;
        Ok(rgba_key(img.width, img.height, &img.rgba))
    }
}

impl Document {
    /// The layer Copy reads: the selected layer whatever its kind, else
    /// the paint target.
    fn copy_layer(&self) -> Result<LayerId, String> {
        match self.active_layer {
            Some(id) if self.sprite.layer(id).is_some() => Ok(id),
            _ => self.paint_target_layer(),
        }
    }

    fn copy_from(&self, source: CopySource) -> Result<CopiedImage, String> {
        let c = copy_pixels(&self.sprite, &self.cels, self.current_frame, source)
            .map_err(|e| format!("failed to copy: {e}"))?;
        let png = encode_png(&c.rgba, c.rect.width, c.rect.height)
            .map_err(|e| format!("failed to copy: {e}"))?;
        Ok(CopiedImage {
            x: c.rect.x,
            y: c.rect.y,
            width: c.rect.width,
            height: c.rect.height,
            png,
        })
    }

    /// Shared body of Paste and Import PNG as layer: one [`PasteLayer`]
    /// step, the new layer active.
    pub(crate) fn paste_image(
        &mut self,
        bytes: &[u8],
        name: &str,
        x: i32,
        y: i32,
        expand: bool,
        above: Option<LayerId>,
    ) -> Result<u32, String> {
        let img = import_png(bytes).map_err(|e| e.to_string())?;
        let new_id = match self.sprite.layers.iter().map(|l| l.id.0).max() {
            None => 0,
            Some(u32::MAX) => return Err("layer id space exhausted".to_string()),
            Some(m) => m + 1,
        };
        let layer = Layer::image(LayerId::new(new_id), name);
        let image = PastedImage {
            width: img.width,
            height: img.height,
            rgba: img.rgba,
        };
        let frame: FrameIndex = self.current_frame;
        let cmd = PasteLayer::new(layer, frame, image)
            .at(x, y)
            .above(above)
            .expanding(expand);
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| e.to_string())?;
        self.bus.seal();
        self.active_layer = Some(LayerId::new(new_id));
        self.events.push(Event::dirty_canvas());
        Ok(new_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pincel_core::{
        Cel, CelData, ColorMode, PaletteEntry, PixelBuffer, Rgba, TileImage, TileRef, Tileset,
        TilesetId,
    };

    const RED: u32 = 0xFF0000FF;
    const BLUE: u32 = 0x0000FFFF;

    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        let img = import_png(png).expect("decode");
        (img.width, img.height, img.rgba)
    }

    fn rgba(c: u32) -> [u8; 4] {
        c.to_be_bytes()
    }

    /// 4×4 document: layer 0 red at (1, 1) and (2, 2), blue at (3, 3).
    fn painted() -> Document {
        let mut doc = Document::new(4, 4).expect("doc");
        doc.apply_tool("pencil", 1, 1, RED).expect("paint");
        doc.apply_tool("pencil", 2, 2, RED).expect("paint");
        doc.apply_tool("pencil", 3, 3, BLUE).expect("paint");
        doc.end_stroke();
        doc
    }

    #[test]
    fn copy_selection_takes_the_selected_box_of_the_active_layer_unchanged_doc() {
        let doc = {
            let mut d = painted();
            d.set_selection(1, 1, 2, 2);
            d
        };
        let depth = doc.undo_depth();
        let c = doc.copy_selection(false).expect("copy");
        assert_eq!((c.x(), c.y(), c.width(), c.height()), (1, 1, 2, 2));
        let (w, h, px) = decode(&c.png());
        assert_eq!((w, h), (2, 2));
        assert_eq!(&px[0..4], &rgba(RED));
        assert_eq!(&px[4..8], &[0; 4]);
        assert_eq!(&px[12..16], &rgba(RED));
        assert_eq!(doc.undo_depth(), depth);
        assert_eq!(doc.pick_color(0, 1, 1).expect("pick"), RED);
    }

    #[test]
    fn copy_selection_shaped_selection_clears_outside_pixels() {
        let mut doc = painted();
        doc.select_ellipse(0, 0, 4, 4, "replace").expect("ellipse");
        let c = doc.copy_selection(false).expect("copy");
        let (w, h, px) = decode(&c.png());
        assert_eq!((w, h), (4, 4));
        // (3, 3) is outside the inscribed ellipse: blue on the layer, but
        // transparent in the copy; (1, 1) is inside.
        assert_eq!(&px[(3 * 4 + 3) * 4..(3 * 4 + 3) * 4 + 4], &[0; 4]);
        assert_eq!(&px[(4 + 1) * 4..(4 + 1) * 4 + 4], &rgba(RED));
    }

    #[test]
    fn copy_selection_without_selection_is_the_whole_layer() {
        let doc = painted();
        let c = doc.copy_selection(false).expect("copy");
        assert_eq!((c.x(), c.y(), c.width(), c.height()), (0, 0, 4, 4));
        let (_, _, px) = decode(&c.png());
        assert_eq!(&px[(3 * 4 + 3) * 4..(3 * 4 + 3) * 4 + 4], &rgba(BLUE));
    }

    #[test]
    fn copy_merged_takes_the_visible_composite_of_the_same_region() {
        let mut doc = painted();
        let top = doc.add_layer("top").expect("layer");
        doc.set_active_layer(top);
        doc.apply_tool("pencil", 1, 2, BLUE).expect("paint");
        doc.end_stroke();
        doc.set_selection(1, 1, 2, 2);
        let single = doc.copy_selection(false).expect("copy");
        let merged = doc.copy_selection(true).expect("copy merged");
        assert_eq!(
            (merged.x(), merged.y(), merged.width(), merged.height()),
            (single.x(), single.y(), single.width(), single.height())
        );
        let (_, _, px) = decode(&merged.png());
        assert_eq!(&px[0..4], &rgba(RED), "from the layer below");
        assert_eq!(&px[8..12], &rgba(BLUE), "from the active layer");
        let (_, _, own) = decode(&single.png());
        assert_eq!(&own[0..4], &[0; 4], "plain Copy stays on the active layer");
    }

    #[test]
    fn cut_selection_copies_then_clears_and_one_undo_restores() {
        let mut doc = painted();
        doc.set_selection(1, 1, 2, 2);
        let copied = doc.copy_selection(false).expect("copy").png();
        let depth = doc.undo_depth();
        let cut = doc.cut_selection().expect("cut");
        assert_eq!(decode(&cut.png()), decode(&copied));
        assert_eq!(doc.pick_color(0, 1, 1).expect("pick") & 0xFF, 0);
        assert_eq!(
            doc.pick_color(0, 3, 3).expect("pick"),
            BLUE,
            "outside stays"
        );
        assert_eq!(doc.undo_depth(), depth + 1);
        assert!(doc.undo());
        assert_eq!(doc.pick_color(0, 1, 1).expect("pick"), RED);
        assert_eq!(doc.pick_color(0, 2, 2).expect("pick"), RED);
    }

    #[test]
    fn cut_selection_without_selection_clears_the_whole_layer() {
        let mut doc = painted();
        let cut = doc.cut_selection().expect("cut");
        assert_eq!((cut.width(), cut.height()), (4, 4));
        assert_eq!(doc.pick_color(0, 3, 3).expect("pick") & 0xFF, 0);
        assert!(doc.undo());
        assert_eq!(doc.pick_color(0, 3, 3).expect("pick"), BLUE);
        assert_eq!(doc.pick_color(0, 1, 1).expect("pick"), RED);
    }

    #[test]
    fn copy_selection_on_a_group_takes_its_rendered_children() {
        let mut doc = painted();
        let group = doc.add_group("g").expect("group");
        doc.move_layer_to(0, group, "into").expect("into group");
        doc.set_active_layer(group);
        let c = doc.copy_selection(false).expect("copy");
        let (_, _, px) = decode(&c.png());
        assert_eq!(&px[(4 + 1) * 4..(4 + 1) * 4 + 4], &rgba(RED));
    }

    #[test]
    fn cut_selection_on_group_or_tilemap_is_refused_unchanged() {
        let mut doc = painted();
        let group = doc.add_group("g").expect("group");
        doc.set_active_layer(group);
        let depth = doc.undo_depth();
        assert!(doc.cut_selection().is_err());
        let ts = doc.add_tileset("t", 2, 2).expect("tileset");
        let map = doc.add_tilemap_layer("m", ts).expect("tilemap");
        doc.set_active_layer(map);
        let depth2 = doc.undo_depth();
        assert!(doc.cut_selection().is_err());
        assert_eq!(doc.undo_depth(), depth2);
        assert!(depth2 >= depth);
        assert_eq!(doc.pick_color(0, 1, 1).expect("pick"), RED);
    }

    #[test]
    fn copy_selection_on_a_tilemap_takes_its_rendered_tiles() {
        let mut doc = Document::new(2, 1).expect("doc");
        let mut ts = Tileset::new(TilesetId::new(0), "t", (1, 1));
        let mut tile = PixelBuffer::empty(1, 1, ColorMode::Rgba);
        tile.data.copy_from_slice(&rgba(BLUE));
        ts.tiles.push(TileImage {
            pixels: PixelBuffer::empty(1, 1, ColorMode::Rgba),
        });
        ts.tiles.push(TileImage { pixels: tile });
        doc.sprite.tilesets.push(ts);
        let map = LayerId::new(5);
        doc.sprite
            .layers
            .push(Layer::tilemap(map, "map", TilesetId::new(0)));
        doc.cels.insert(Cel {
            data: CelData::Tilemap {
                grid_w: 2,
                grid_h: 1,
                tiles: vec![TileRef::new(0), TileRef::new(1)],
            },
            ..Cel::image(
                map,
                FrameIndex::new(0),
                PixelBuffer::empty(1, 1, ColorMode::Rgba),
            )
        });
        doc.set_active_layer(5);
        let (_, _, px) = decode(&doc.copy_selection(false).expect("copy").png());
        assert_eq!(&px[0..4], &[0; 4]);
        assert_eq!(&px[4..8], &rgba(BLUE));
    }

    #[test]
    fn copy_selection_from_indexed_sprite_writes_palette_colors() {
        let mut doc = Document::new(2, 1).expect("doc");
        doc.sprite.color_mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        doc.sprite.palette.colors = vec![
            PaletteEntry::new(Rgba::new(0, 0, 0, 255)),
            PaletteEntry::new(Rgba::new(12, 34, 56, 255)),
        ];
        let mut buf = PixelBuffer::empty(
            2,
            1,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        buf.data = vec![1, 0];
        doc.cels
            .insert(Cel::image(crate::DEFAULT_LAYER_ID, FrameIndex::new(0), buf));
        let (_, _, px) = decode(&doc.copy_selection(false).expect("copy").png());
        assert_eq!(&px[0..4], &[12, 34, 56, 255]);
        assert_eq!(&px[4..8], &[0; 4]);
    }

    #[test]
    fn paste_layer_lands_above_the_active_layer_at_the_position_in_one_undo_step() {
        let mut doc = painted();
        let top = doc.add_layer("top").expect("layer");
        doc.set_active_layer(0);
        doc.set_selection(1, 1, 2, 2);
        let png = doc.copy_selection(false).expect("copy").png();
        let depth = doc.undo_depth();
        let id = doc.paste_layer(&png, 2, 1, false).expect("paste");
        assert_eq!(doc.layer_count(), 3);
        assert_eq!(
            doc.layer_id_at(1).expect("id"),
            id,
            "above the active layer"
        );
        assert_eq!(doc.layer_id_at(2).expect("id"), top);
        assert_eq!(doc.paint_target_layer_id(), id);
        assert_eq!(doc.undo_depth(), depth + 1);
        // Copied (1, 1) lands at (2, 1).
        doc.set_layer_visible(0, false).expect("hide");
        assert_eq!(doc.pick_color(0, 2, 1).expect("pick"), RED);
        assert_eq!(doc.pick_color(0, 1, 1).expect("pick") & 0xFF, 0);
        doc.set_layer_visible(0, true).expect("show");
        assert!(doc.undo(), "show");
        assert!(doc.undo(), "hide");
        assert!(doc.undo(), "paste");
        assert_eq!(doc.layer_count(), 2);
    }

    #[test]
    fn paste_layer_expand_grows_the_canvas_and_one_undo_shrinks_it() {
        let mut doc = Document::new(4, 4).expect("doc");
        let big = Document::new(6, 5)
            .expect("doc")
            .export_png(0)
            .expect("png");
        doc.paste_layer(&big, 1, 2, true).expect("paste");
        assert_eq!((doc.width(), doc.height()), (7, 7));
        assert!(doc.undo());
        assert_eq!((doc.width(), doc.height()), (4, 4));
        assert_eq!(doc.layer_count(), 1);
        doc.paste_layer(&big, 1, 2, false).expect("paste");
        assert_eq!((doc.width(), doc.height()), (4, 4));
    }

    #[test]
    fn paste_layer_into_indexed_sprite_is_refused_unchanged() {
        let mut doc = Document::new(2, 2).expect("doc");
        doc.sprite.color_mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        let png = Document::new(1, 1)
            .expect("doc")
            .export_png(0)
            .expect("png");
        assert!(doc.paste_layer(&png, 0, 0, false).is_err());
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.undo_depth(), 0);
    }

    #[test]
    fn image_key_matches_for_the_same_pixels_and_differs_otherwise() {
        let doc = painted();
        let a = doc.copy_selection(false).expect("copy").png();
        let b = doc.export_png(0).expect("png");
        assert_eq!(Document::image_key(&a), Document::image_key(&b));
        let other = Document::new(4, 4)
            .expect("doc")
            .export_png(0)
            .expect("png");
        assert_ne!(Document::image_key(&a), Document::image_key(&other));
        assert!(Document::image_key(&[1, 2, 3]).is_err());
    }

    #[test]
    fn rgba_key_ignores_color_under_zero_alpha() {
        assert_eq!(
            rgba_key(1, 1, &[200, 100, 50, 0]),
            rgba_key(1, 1, &[0, 0, 0, 0])
        );
        assert_ne!(
            rgba_key(1, 1, &[200, 100, 50, 255]),
            rgba_key(1, 1, &[0, 0, 0, 255])
        );
    }
}
