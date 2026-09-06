//! Styled shape surface (Fineliner parity): rectangle, rounded
//! rectangle, ellipse and regular polygon with stroke width and
//! outline / fill / fill+outline modes. `applyRectangle` / `applyEllipse`
//! stay as the 1-px spec §5.2 tools; this is the general form.

use pincel_core::{DrawShape, Rgba, ShapeKind, ShapeMode, ShapeStyle};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

#[wasm_bindgen]
impl Document {
    /// Draw a styled shape spanning the inclusive sprite-space corners
    /// `(x0, y0)`–`(x1, y1)` on the active layer / current frame.
    ///
    /// - `kind`: `rectangle`, `rounded_rectangle`, `ellipse`, `polygon`.
    /// - `mode`: `outline`, `fill`, `fill_outline`.
    /// - `stroke_width`: pixels, at least 1.
    /// - `stroke_color` / `fill_color`: `0xRRGGBBAA`.
    /// - `param`: corner radius for `rounded_rectangle`, number of sides
    ///   for `polygon` (3–64); ignored otherwise.
    ///
    /// One undo entry; emits a `dirty-rect` for the shape bounds.
    #[wasm_bindgen(js_name = applyShape)]
    #[expect(
        clippy::too_many_arguments,
        reason = "flat positional wasm-bindgen signature, mirrors applyRectangle/applyEllipse"
    )]
    pub fn apply_shape(
        &mut self,
        kind: &str,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        mode: &str,
        stroke_width: u32,
        stroke_color: u32,
        fill_color: u32,
        param: f64,
    ) -> Result<(), String> {
        let kind = match kind {
            "rectangle" => ShapeKind::Rectangle,
            "rounded_rectangle" => ShapeKind::RoundedRectangle {
                radius: if param.is_finite() {
                    param.max(0.0)
                } else {
                    0.0
                },
            },
            "ellipse" => ShapeKind::Ellipse,
            "polygon" => ShapeKind::Polygon {
                sides: if param.is_finite() {
                    param.round().clamp(3.0, 64.0) as u32
                } else {
                    3
                },
                rotation_deg: 0.0,
            },
            other => return Err(format!("unknown shape kind `{other}`")),
        };
        let mode =
            ShapeMode::from_name(mode).ok_or_else(|| format!("unknown shape mode `{mode}`"))?;
        let layer = self.paint_target_layer()?;
        let frame = self.current_frame;
        self.ensure_paint_cel(layer, frame);
        let style = ShapeStyle {
            mode,
            stroke_width,
            stroke: Rgba::from_u32(stroke_color),
            fill: Rgba::from_u32(fill_color),
        };
        let cmd = DrawShape::new(layer, frame, (x0, y0), (x1, y1), kind, style);
        let bounds = cmd.bounds();
        self.bus.seal();
        self.bus
            .execute(cmd.into(), &mut self.sprite, &mut self.cels)
            .map_err(|e| format!("failed to draw shape: {e}"))?;
        self.bus.seal();
        self.events.push(Event::dirty_rect(
            layer.0,
            frame.0,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(doc: &Document, x: i32, y: i32) -> u32 {
        doc.pick_color(0, x, y).expect("pick")
    }

    #[test]
    fn apply_shape_fill_outline_rectangle() {
        let mut doc = Document::new(6, 6).expect("dims");
        doc.apply_shape(
            "rectangle",
            0,
            0,
            5,
            5,
            "fill_outline",
            2,
            0xFF0000FF,
            0x0000FFFF,
            0.0,
        )
        .expect("shape");
        assert_eq!(px(&doc, 0, 0), 0xFF0000FF);
        assert_eq!(px(&doc, 1, 1), 0xFF0000FF);
        assert_eq!(px(&doc, 2, 2), 0x0000FFFF);
        assert_eq!(doc.undo_depth(), 1);
        assert!(doc.undo());
        assert_eq!(px(&doc, 0, 0) & 0xFF, 0);
    }

    #[test]
    fn apply_shape_polygon_and_rounded_rectangle_and_errors() {
        let mut doc = Document::new(8, 8).expect("dims");
        doc.apply_shape("polygon", 0, 0, 7, 7, "fill", 1, 0, 0x00FF00FF, 6.0)
            .expect("hexagon");
        assert_eq!(px(&doc, 3, 3), 0x00FF00FF);
        assert_eq!(px(&doc, 0, 0) & 0xFF, 0);
        doc.apply_shape(
            "rounded_rectangle",
            0,
            0,
            7,
            7,
            "outline",
            1,
            0xFFFFFFFF,
            0,
            3.0,
        )
        .expect("rounded");
        assert_eq!(px(&doc, 3, 0), 0xFFFFFFFF);
        assert!(
            doc.apply_shape("star", 0, 0, 1, 1, "fill", 1, 0, 0, 0.0)
                .is_err()
        );
        assert!(
            doc.apply_shape("ellipse", 0, 0, 1, 1, "dashed", 1, 0, 0, 0.0)
                .is_err()
        );
        let ev = doc.drain_events();
        assert!(
            ev.iter()
                .any(|e| e.kind() == "dirty-rect" && e.width() == 8)
        );
    }
}
