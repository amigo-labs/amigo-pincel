//! Text surface (Fineliner parity): register a font once, then draw or
//! preview text on the active cel.
//!
//! Fonts are uploaded as raw TrueType / OpenType bytes via
//! [`register_font`] and referenced by id so a 400 KB face does not cross
//! the boundary on every keystroke of the text dialog. The registry is a
//! thread-local `Vec` — wasm is single-threaded and the ids live as long
//! as the module.

use std::cell::RefCell;

use pincel_core::{
    Cel, CelData, ColorMode, ComposeRequest, DrawText, FrameIndex, LayerId, PixelBuffer, Rgba,
    TextAlign, TextStyle, compose,
};
use wasm_bindgen::prelude::*;

use crate::events::Event;
use crate::{ComposeFrame, Document};

thread_local! {
    /// Registered font blobs, indexed by the id `register_font` hands out.
    static FONTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

/// Register a TrueType / OpenType font and return its id for
/// [`Document::draw_text`]. Re-registering byte-identical data returns the
/// existing id.
#[wasm_bindgen(js_name = registerFont)]
pub fn register_font(data: &[u8]) -> u32 {
    FONTS.with(|fonts| {
        let mut fonts = fonts.borrow_mut();
        if let Some(id) = fonts.iter().position(|f| f == data) {
            return id as u32;
        }
        fonts.push(data.to_vec());
        (fonts.len() - 1) as u32
    })
}

fn font_bytes(font_id: u32) -> Result<Vec<u8>, String> {
    FONTS
        .with(|fonts| fonts.borrow().get(font_id as usize).cloned())
        .ok_or_else(|| format!("unknown font id {font_id}"))
}

/// Text parameters shared by draw and preview.
struct TextArgs<'a> {
    font_id: u32,
    text: &'a str,
    x: i32,
    y: i32,
    style: TextStyle,
}

fn parse_style(
    size: f32,
    color: u32,
    bold: bool,
    italic: bool,
    anti_alias: bool,
    align: &str,
) -> Result<TextStyle, String> {
    let align =
        TextAlign::from_name(align).ok_or_else(|| format!("unknown text align `{align}`"))?;
    Ok(TextStyle {
        size,
        color: Rgba::from_u32(color),
        bold,
        italic,
        anti_alias,
        align,
    })
}

#[wasm_bindgen]
impl Document {
    /// Rasterise `text` onto the active layer's cel at the current frame.
    /// `(x, y)` is the top-left of the first line's em box (`x` is the
    /// alignment anchor for `center` / `right`). `size` is in pixels,
    /// `color` is `0xRRGGBBAA`, `align` is `left` / `center` / `right`.
    /// Returns `false` when no glyph landed on the canvas (nothing was
    /// committed). One undo entry; emits a `dirty-rect`.
    #[wasm_bindgen(js_name = drawText)]
    #[expect(
        clippy::too_many_arguments,
        reason = "flat positional wasm-bindgen signature; a struct would need serde"
    )]
    pub fn draw_text(
        &mut self,
        font_id: u32,
        text: &str,
        x: i32,
        y: i32,
        size: f32,
        color: u32,
        bold: bool,
        italic: bool,
        anti_alias: bool,
        align: &str,
    ) -> Result<bool, String> {
        let style = parse_style(size, color, bold, italic, anti_alias, align)?;
        let args = TextArgs {
            font_id,
            text,
            x,
            y,
            style,
        };
        let (layer, frame, cmd) = self.text_command(&args)?;
        self.ensure_paint_cel(layer, frame);
        self.bus.seal();
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to draw text: {e}"))?;
        self.bus.seal();
        let region = self.bus.last_dirty_region();
        if region.is_none() {
            // Nothing landed: drop the empty entry so undo stays meaningful.
            self.bus.undo(&mut self.sprite, &mut self.cels);
            return Ok(false);
        }
        if let Some(ev) = Event::from_dirty(region) {
            self.events.push(ev);
        }
        Ok(true)
    }

    /// Composite the current frame at `zoom` as it would look after
    /// [`Document::draw_text`] with the same arguments, without touching
    /// the document or the undo history.
    #[wasm_bindgen(js_name = previewText)]
    #[expect(
        clippy::too_many_arguments,
        reason = "flat positional wasm-bindgen signature; a struct would need serde"
    )]
    pub fn preview_text(
        &self,
        font_id: u32,
        text: &str,
        x: i32,
        y: i32,
        size: f32,
        color: u32,
        bold: bool,
        italic: bool,
        anti_alias: bool,
        align: &str,
        zoom: u32,
    ) -> Result<ComposeFrame, String> {
        let style = parse_style(size, color, bold, italic, anti_alias, align)?;
        let args = TextArgs {
            font_id,
            text,
            x,
            y,
            style,
        };
        let (layer, frame, mut cmd) = self.text_command(&args)?;
        let mut sprite = self.sprite.clone();
        let mut cels = self.cels.clone();
        if cels.get(layer, frame).is_none() {
            cels.insert(Cel::image(
                layer,
                frame,
                PixelBuffer::empty(sprite.width, sprite.height, ColorMode::Rgba),
            ));
        }
        pincel_core::Command::apply(&mut cmd, &mut sprite, &mut cels)
            .map_err(|e| format!("failed to preview text: {e}"))?;
        let mut request =
            ComposeRequest::full(self.current_frame, self.sprite.width, self.sprite.height);
        request.zoom = zoom;
        let mut pixels = Vec::new();
        let result = compose(&sprite, &cels, &request, &mut pixels)
            .map_err(|e| format!("failed to compose preview: {e}"))?;
        Ok(ComposeFrame {
            width: result.width,
            height: result.height,
            dirty_x: result.dirty_rect.x,
            dirty_y: result.dirty_rect.y,
            pixels,
        })
    }
}

impl Document {
    /// Build the `DrawText` for the active layer / frame. The caller seeds
    /// a missing cel (`draw_text` on the document, `preview_text` on its
    /// clone).
    fn text_command(&self, args: &TextArgs<'_>) -> Result<(LayerId, FrameIndex, DrawText), String> {
        let font = font_bytes(args.font_id)?;
        let layer = self.paint_target_layer()?;
        let frame = self.current_frame;
        if !matches!(
            self.cels.get(layer, frame).map(|c| &c.data),
            Some(CelData::Image(_))
        ) && self.cels.get(layer, frame).is_some()
        {
            return Err("the active cel is not an image cel".to_owned());
        }
        Ok((
            layer,
            frame,
            DrawText::new(layer, frame, (args.x, args.y), args.text, font, args.style),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../pincel-core/tests/fixtures/LiberationSans-Regular.ttf"
    ));

    fn opaque(doc: &Document) -> usize {
        let frame = doc.compose(0, 1).expect("compose");
        frame
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count()
    }

    #[test]
    fn register_font_dedupes() {
        let a = register_font(FONT);
        let b = register_font(FONT);
        assert_eq!(a, b);
    }

    #[test]
    fn draw_text_paints_and_undoes_and_preview_leaves_state() {
        let font = register_font(FONT);
        let mut doc = Document::new(64, 32).expect("dims");
        let preview = doc
            .preview_text(
                font, "Hi", 2, 2, 20.0, 0xFFFFFFFF, false, false, false, "left", 1,
            )
            .expect("preview");
        assert!(preview.pixels().as_chunks::<4>().0.iter().any(|p| p[3] > 0));
        assert_eq!(opaque(&doc), 0, "preview did not paint");
        assert_eq!(doc.undo_depth(), 0);
        assert!(
            doc.draw_text(font, "Hi", 2, 2, 20.0, 0xFFFFFFFF, true, true, true, "left")
                .expect("draw")
        );
        assert!(opaque(&doc) > 20);
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.undo());
        assert_eq!(opaque(&doc), 0);
    }

    #[test]
    fn draw_and_preview_seed_a_missing_cel_on_a_fresh_layer() {
        let font = register_font(FONT);
        let mut doc = Document::new(64, 32).expect("dims");
        let top = doc.add_layer("text").expect("add");
        doc.set_active_layer(top);
        let preview = doc
            .preview_text(
                font, "A", 0, 0, 20.0, 0xFF00FFFF, false, false, false, "left", 1,
            )
            .expect("preview");
        assert!(preview.pixels().as_chunks::<4>().0.iter().any(|p| p[3] > 0));
        assert!(
            doc.draw_text(
                font, "A", 0, 0, 20.0, 0xFF00FFFF, false, false, false, "left"
            )
            .expect("draw")
        );
        assert!(opaque(&doc) > 5);
    }

    #[test]
    fn draw_text_off_canvas_returns_false_and_leaves_no_undo_entry() {
        let font = register_font(FONT);
        let mut doc = Document::new(8, 8).expect("dims");
        assert!(
            !doc.draw_text(
                font, "x", 500, 500, 12.0, 0xFFFFFFFF, false, false, false, "left"
            )
            .expect("draw")
        );
        assert_eq!(doc.undo_depth(), 0);
    }

    #[test]
    fn draw_text_rejects_bad_font_and_align() {
        let mut doc = Document::new(8, 8).expect("dims");
        assert!(
            doc.draw_text(9999, "x", 0, 0, 12.0, 0, false, false, false, "left")
                .is_err()
        );
        let font = register_font(FONT);
        assert!(
            doc.draw_text(font, "x", 0, 0, 12.0, 0, false, false, false, "justify")
                .is_err()
        );
    }
}
