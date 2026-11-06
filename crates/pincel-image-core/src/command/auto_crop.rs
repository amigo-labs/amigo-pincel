//! Image ▸ Auto Crop: trims a uniform border from the composite
//! (docs/specs/image-resize-autocrop.md AC-13 – AC-16).

use super::Command;
use super::snapshot::{DocSnapshot, restore, snapshot};
use crate::color::Color;
use crate::document::{CanvasSize, Document, ImageBuffer};
use crate::error::DocumentError;
use crate::geometry::Rect;
use crate::render::compose;
use std::any::Any;

/// Two pixels match when all four channels are equal or both are fully
/// transparent (AC-13).
fn matches(a: Color, b: Color) -> bool {
    a == b || (a.a == 0 && b.a == 0)
}

/// The smallest rectangle of `image` holding every pixel that differs from
/// its top-left pixel, or `None` when that rectangle is empty or the whole
/// image (AC-15).
fn content_rect(image: &ImageBuffer) -> Option<Rect> {
    let (w, h) = (image.width(), image.height());
    let border = image.get_pixel(0, 0)?;
    let differs = |x: u32, y: u32| image.get_pixel(x, y).is_some_and(|c| !matches(c, border));
    let row_uniform = |y: u32| (0..w).all(|x| !differs(x, y));
    let col_uniform = |x: u32, y0: u32, y1: u32| (y0..y1).all(|y| !differs(x, y));
    let top = (0..h).find(|&y| !row_uniform(y))?;
    let bottom = (top..h).rev().find(|&y| !row_uniform(y))? + 1;
    let left = (0..w).find(|&x| !col_uniform(x, top, bottom))?;
    let right = (left..w).rev().find(|&x| !col_uniform(x, top, bottom))? + 1;
    let rect = Rect::new(left as i32, top as i32, right - left, bottom - top);
    (rect.w < w || rect.h < h).then_some(rect)
}

/// The area Auto Crop keeps, in canvas pixels: the content inside a border
/// matching the composite's top-left pixel. `None` when there is nothing to
/// crop — no edge is uniform, or the whole composite is (AC-15).
pub fn auto_crop_rect(doc: &Document) -> Option<Rect> {
    content_rect(&compose(doc.layers()))
}

/// Crops every layer to [`auto_crop_rect`] in one undo step, clearing the
/// selection (AC-13, AC-14). A no-op when there is nothing to crop.
pub struct AutoCrop {
    before: Option<DocSnapshot>,
}

impl AutoCrop {
    /// Creates the command.
    pub fn new() -> Self {
        Self { before: None }
    }
}

impl Default for AutoCrop {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for AutoCrop {
    fn apply(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        let Some(rect) = auto_crop_rect(doc) else {
            return Ok(());
        };
        let new_canvas = CanvasSize::new(rect.w, rect.h)?;
        if self.before.is_none() {
            self.before = Some(snapshot(doc));
        }
        for layer in doc.layers.iter_mut().filter(|l| !l.is_group()) {
            layer.pixels = layer.pixels.copy_region(rect)?;
        }
        doc.canvas = new_canvas;
        doc.selection = None;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Document) -> Result<(), DocumentError> {
        match self.before.take() {
            Some(snap) => restore(doc, snap),
            None => Ok(()),
        }
    }

    fn label(&self) -> &str {
        "Auto Crop"
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
    use crate::color::BlendMode;
    use crate::command::CommandBus;
    use crate::command::spec::{CommandSpec, StrokeScratch, run_command};
    use crate::document::Layer;
    use crate::selection::SelectionMask;

    const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    const RED: Color = Color {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };

    /// A `w × h` document whose only layer is filled with `bg`.
    fn doc_filled(w: u32, h: u32, bg: Color) -> Document {
        let mut doc = Document::new(w, h).expect("doc");
        let px = &mut doc.layers[0].pixels;
        for y in 0..h {
            for x in 0..w {
                px.set_pixel(x, y, bg);
            }
        }
        doc
    }

    fn run(bus: &mut CommandBus, json: &str) {
        let spec: CommandSpec = serde_json::from_str(json).expect("parses");
        run_command(bus, spec, &mut StrokeScratch::default(), &[]).expect("runs");
    }

    #[test]
    fn auto_crop_rect_trims_a_uniform_border() {
        let mut doc = doc_filled(10, 8, WHITE);
        doc.layers[0].pixels.set_pixel(3, 2, RED);
        doc.layers[0].pixels.set_pixel(6, 5, RED);
        assert_eq!(auto_crop_rect(&doc), Some(Rect::new(3, 2, 4, 4)));
    }

    #[test]
    fn auto_crop_rect_trims_only_the_free_edges() {
        // Content touching the right edge keeps that edge.
        let mut doc = doc_filled(6, 6, WHITE);
        doc.layers[0].pixels.set_pixel(5, 3, RED);
        assert_eq!(auto_crop_rect(&doc), Some(Rect::new(5, 3, 1, 1)));
    }

    #[test]
    fn auto_crop_rect_matches_transparent_pixels_of_any_rgb() {
        let mut doc = Document::new(5, 5).expect("doc");
        doc.layers[0]
            .pixels
            .set_pixel(4, 0, Color::rgba(10, 20, 30, 0));
        doc.layers[0].pixels.set_pixel(2, 2, RED);
        assert_eq!(auto_crop_rect(&doc), Some(Rect::new(2, 2, 1, 1)));
    }

    #[test]
    fn auto_crop_rect_uniform_or_framed_is_none() {
        assert_eq!(auto_crop_rect(&Document::new(4, 4).expect("doc")), None);
        // A top-left pixel differing from every edge neighbour: no edge is
        // uniform.
        let mut doc = doc_filled(4, 4, WHITE);
        doc.layers[0].pixels.set_pixel(0, 0, RED);
        assert_eq!(auto_crop_rect(&doc), None);
    }

    #[test]
    fn auto_crop_rect_reads_the_visible_composite() {
        // Content only on a hidden layer: the composite is uniform.
        let mut doc = doc_filled(6, 6, WHITE);
        let mut hidden = Layer::new_transparent("hidden", 6, 6);
        hidden.pixels.set_pixel(2, 2, RED);
        hidden.visible = false;
        doc.layers.push(hidden);
        assert_eq!(auto_crop_rect(&doc), None);
        // A visible layer at opacity 0 adds nothing either.
        doc.layers[1].visible = true;
        doc.layers[1].opacity = 0.0;
        assert_eq!(auto_crop_rect(&doc), None);
        doc.layers[1].opacity = 1.0;
        assert_eq!(auto_crop_rect(&doc), Some(Rect::new(2, 2, 1, 1)));
    }

    #[test]
    fn auto_crop_rect_hides_the_content_of_a_hidden_group() {
        // A visible layer inside a hidden group: the canvas shows nothing.
        let mut doc = doc_filled(6, 6, WHITE);
        let group = Layer::new_group("group");
        let mut child = Layer::new_transparent("child", 6, 6);
        child.pixels.set_pixel(2, 2, RED);
        child.parent = Some(group.id);
        doc.layers.push(group);
        doc.layers.push(child);
        doc.layers[1].visible = false;
        assert!(doc.layers[2].visible);
        assert_eq!(auto_crop_rect(&doc), None);
        doc.layers[1].visible = true;
        assert_eq!(auto_crop_rect(&doc), Some(Rect::new(2, 2, 1, 1)));
    }

    #[test]
    fn auto_crop_rect_follows_the_blend_mode() {
        // Black in Screen mode over white composites to white: no content.
        let mut doc = doc_filled(6, 6, WHITE);
        let mut top = Layer::new_transparent("top", 6, 6);
        top.pixels.set_pixel(2, 3, Color::rgba(0, 0, 0, 255));
        top.blend_mode = BlendMode::Screen;
        doc.layers.push(top);
        assert_eq!(auto_crop_rect(&doc), None);
        doc.layers[1].blend_mode = BlendMode::Normal;
        assert_eq!(auto_crop_rect(&doc), Some(Rect::new(2, 3, 1, 1)));
    }

    #[test]
    fn auto_crop_crops_every_layer_and_undo_restores() {
        let mut doc = doc_filled(8, 8, WHITE);
        doc.layers[0].pixels.set_pixel(2, 3, RED);
        doc.layers[0].pixels.set_pixel(4, 4, RED);
        let mut hidden = Layer::new_transparent("hidden", 8, 8);
        hidden.pixels.set_pixel(3, 3, Color::rgba(0, 0, 255, 255));
        hidden.pixels.set_pixel(7, 7, Color::rgba(0, 0, 255, 255));
        hidden.visible = false;
        doc.layers.push(hidden);
        let mut mask = SelectionMask::new_empty(8, 8);
        mask.set(1, 1, 255);
        doc.selection = Some(mask);
        let before_layers = doc.layers().to_vec();
        let before_sel = doc.selection.clone();
        let mut bus = CommandBus::new(doc);
        run(&mut bus, r#"{"type":"auto_crop"}"#);
        let d = &bus.document;
        assert_eq!((d.canvas.width(), d.canvas.height()), (3, 2));
        assert!(d.selection.is_none());
        assert_eq!(d.layers[0].pixels.get_pixel(0, 0), Some(RED));
        // The hidden layer is cropped in the same place.
        assert_eq!(
            d.layers[1].pixels.get_pixel(1, 0),
            Some(Color::rgba(0, 0, 255, 255))
        );
        assert_eq!(d.layers[1].pixels.width(), 3);
        assert_eq!(bus.history.undo_depth(), 1);
        assert!(bus.undo().expect("undo"));
        assert_eq!(bus.document.layers(), &before_layers[..]);
        assert_eq!(bus.document.selection, before_sel);
        assert_eq!(
            (bus.document.canvas.width(), bus.document.canvas.height()),
            (8, 8)
        );
    }

    #[test]
    fn auto_crop_with_nothing_to_crop_adds_no_undo_step() {
        let mut bus = CommandBus::new(doc_filled(4, 4, WHITE));
        run(&mut bus, r#"{"type":"auto_crop"}"#);
        assert_eq!(bus.history.undo_depth(), 0);
        assert_eq!(
            (bus.document.canvas.width(), bus.document.canvas.height()),
            (4, 4)
        );
    }

    #[test]
    fn scale_image_spec_parses_supersampling() {
        let mut doc = doc_filled(2, 1, Color::TRANSPARENT);
        doc.layers[0].pixels.set_pixel(0, 0, RED);
        let mut bus = CommandBus::new(doc);
        run(
            &mut bus,
            r#"{"type":"scale_image","width":1,"height":1,"interpolation":"supersampling"}"#,
        );
        let c = bus.document.layers[0].pixels.get_pixel(0, 0).expect("px");
        assert_eq!([c.r, c.g, c.b], [255, 0, 0]);
        assert!(c.a == 127 || c.a == 128);
    }

    #[test]
    fn scale_image_supersampling_is_one_undo_step() {
        let mut doc = doc_filled(6, 4, WHITE);
        doc.layers[0].pixels.set_pixel(1, 1, RED);
        doc.layers.push(Layer::new_transparent("top", 6, 4));
        doc.selection = Some(SelectionMask::new_full(6, 4));
        let before_layers = doc.layers().to_vec();
        let mut bus = CommandBus::new(doc);
        run(
            &mut bus,
            r#"{"type":"scale_image","width":3,"height":2,"interpolation":"supersampling"}"#,
        );
        assert_eq!(bus.history.undo_depth(), 1);
        assert!(bus.document.selection.is_none());
        assert!(bus.document.layers.iter().all(|l| l.pixels.width() == 3));
        assert!(bus.undo().expect("undo"));
        assert_eq!(bus.document.layers(), &before_layers[..]);
        assert_eq!(bus.document.selection, Some(SelectionMask::new_full(6, 4)));
        assert_eq!(bus.document.canvas.width(), 6);
    }
}
