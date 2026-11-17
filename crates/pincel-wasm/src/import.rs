//! Raster import surface (Fineliner parity): open a PNG as a new document
//! or drop one onto the current document as a new layer; open an animated
//! GIF, APNG or WebP as a document with one frame per animation frame
//! (docs/specs/animated-image-import.md).

use pincel_core::{
    ANIMATION_MAX_PIXELS, ANIMATION_MAX_SIDE, AnimatedFormat, AnimationLimit,
    AnimationProbe as CoreProbe, Cel, ColorMode, Frame, FrameIndex, PixelBuffer, import_animation,
    import_png, probe_animation,
};
use wasm_bindgen::prelude::*;

use crate::Document;

/// What the open dialog shows about an animated file: see
/// [`probe_animation_js`].
#[wasm_bindgen]
pub struct AnimationProbe {
    probe: CoreProbe,
}

#[wasm_bindgen]
impl AnimationProbe {
    /// `"gif"`, `"apng"` or `"webp"`.
    #[wasm_bindgen(getter)]
    pub fn format(&self) -> String {
        match self.probe.format {
            AnimatedFormat::Gif => "gif",
            AnimatedFormat::Apng => "apng",
            AnimatedFormat::WebP => "webp",
        }
        .to_owned()
    }

    /// Canvas width in pixels.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.probe.width
    }

    /// Canvas height in pixels.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.probe.height
    }

    /// Frames the file announces (at least 2).
    #[wasm_bindgen(getter, js_name = frameCount)]
    pub fn frame_count(&self) -> u32 {
        self.probe.frame_count
    }

    /// `frames × width × height`, exact up to 2^53.
    #[wasm_bindgen(getter, js_name = totalPixels)]
    pub fn total_pixels(&self) -> f64 {
        self.probe.total_pixels() as f64
    }

    /// The limit that keeps the file from opening as an animation:
    /// `"side"`, `"pixels"`, or `undefined` when it fits.
    #[wasm_bindgen(getter)]
    pub fn exceeded(&self) -> Option<String> {
        self.probe.exceeded_limit().map(|limit| {
            match limit {
                AnimationLimit::Side => "side",
                AnimationLimit::Pixels => "pixels",
            }
            .to_owned()
        })
    }

    /// The largest canvas width or height an animation may have.
    #[wasm_bindgen(getter, js_name = maxSide)]
    pub fn max_side(&self) -> u32 {
        ANIMATION_MAX_SIDE
    }

    /// The largest `frames × width × height` an animation may have.
    #[wasm_bindgen(getter, js_name = maxPixels)]
    pub fn max_pixels(&self) -> f64 {
        ANIMATION_MAX_PIXELS as f64
    }
}

/// Whether `bytes` hold an animated GIF, APNG or WebP (more than one
/// frame): its size and frame count, or `undefined` for any other file.
/// Only the first frame is decoded. Errors when the file announces several
/// frames but its first frame cannot be decoded or its canvas is empty.
#[wasm_bindgen(js_name = probeAnimation)]
pub fn probe_animation_js(bytes: &[u8]) -> Result<Option<AnimationProbe>, String> {
    probe_animation(bytes)
        .map(|probe| probe.map(|probe| AnimationProbe { probe }))
        .map_err(|e| e.to_string())
}

#[wasm_bindgen]
impl Document {
    /// Create a new RGBA document from PNG bytes: one image layer named
    /// `"Layer 1"` holding the decoded pixels at frame `0`, one 100 ms
    /// frame. Any PNG colour type / bit depth is accepted.
    #[wasm_bindgen(js_name = openPng)]
    pub fn open_png(bytes: &[u8]) -> Result<Document, String> {
        let img = import_png(bytes).map_err(|e| format!("failed to open PNG: {e}"))?;
        let mut doc = Document::new(img.width, img.height)?;
        let mut buf = PixelBuffer::empty(img.width, img.height, ColorMode::Rgba);
        buf.data = img.rgba;
        doc.cels
            .insert(Cel::image(crate::DEFAULT_LAYER_ID, FrameIndex::new(0), buf));
        Ok(doc)
    }

    /// Create a new RGBA document from an animated GIF, APNG or WebP: one
    /// image layer named `"Layer 1"`, one frame per animation frame in
    /// file order, each holding the full canvas as the viewer sees it and
    /// the file's delay as its duration; the palette of a new document.
    /// Errors for a file that is not animated, exceeds the size limits or
    /// cannot be read completely.
    #[wasm_bindgen(js_name = openAnimation)]
    pub fn open_animation(bytes: &[u8]) -> Result<Document, String> {
        let anim = import_animation(bytes).map_err(|e| e.to_string())?;
        let mut doc = Document::new(anim.width, anim.height)?;
        doc.sprite.frames = anim
            .frames
            .iter()
            .map(|f| Frame::new(f.duration_ms))
            .collect();
        for (i, frame) in anim.frames.into_iter().enumerate() {
            let mut buf = PixelBuffer::empty(anim.width, anim.height, ColorMode::Rgba);
            buf.data = frame.rgba;
            let index = u32::try_from(i).map_err(|_| "too many frames".to_owned())?;
            doc.cels.insert(Cel::image(
                crate::DEFAULT_LAYER_ID,
                FrameIndex::new(index),
                buf,
            ));
        }
        Ok(doc)
    }

    /// Decode PNG bytes and add them as a new image layer named `name` on
    /// top of the stack, at the current frame, anchored at the canvas
    /// origin and clipped to the canvas. Returns the new layer id (which
    /// also becomes the active paint target). One undo step removes the
    /// layer and its pixels (the Paste command, spec §5.4).
    #[wasm_bindgen(js_name = importPngAsLayer)]
    pub fn import_png_as_layer(&mut self, bytes: &[u8], name: &str) -> Result<u32, String> {
        self.paste_image(bytes, name, 0, 0, false, None)
            .map_err(|e| format!("failed to import PNG: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PNG bytes for a 2×1 image with the given two `0xRRGGBBAA` pixels,
    /// produced through the document's own exporter.
    fn png_2x1(left: u32, right: u32) -> Vec<u8> {
        let mut doc = Document::new(2, 1).expect("dims");
        doc.apply_tool("pencil", 0, 0, left).expect("paint");
        doc.apply_tool("pencil", 1, 0, right).expect("paint");
        doc.export_png(0, None).expect("export").into_vec()
    }

    #[test]
    fn open_png_builds_a_paintable_document_with_the_pixels() {
        let bytes = png_2x1(0xFF0000FF, 0x0000FF80);
        let mut doc = Document::open_png(&bytes).expect("open");
        assert_eq!((doc.width(), doc.height()), (2, 1));
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0xFF0000FF);
        assert_eq!(doc.pick_color(0, 1, 0).expect("pick"), 0x0000FF80);
        doc.apply_tool("pencil", 1, 0, 0x00FF00FF)
            .expect("paint on it");
        assert_eq!(doc.pick_color(0, 1, 0).expect("pick"), 0x00FF00FF);
        assert!(Document::open_png(&[1, 2, 3]).is_err());
    }

    /// APNG bytes of a 2×1 document with three frames: red / green /
    /// blue on the left pixel, durations 100, 40 and 230 ms.
    fn three_frame_apng() -> Vec<u8> {
        let mut doc = Document::new(2, 1).expect("dims");
        doc.sprite.frames = vec![Frame::new(100), Frame::new(40), Frame::new(230)];
        let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]];
        for (i, color) in colors.into_iter().enumerate() {
            let mut buf = PixelBuffer::empty(2, 1, ColorMode::Rgba);
            buf.data[..4].copy_from_slice(&color);
            let index = u32::try_from(i).expect("few");
            doc.cels.insert(Cel::image(
                crate::DEFAULT_LAYER_ID,
                FrameIndex::new(index),
                buf,
            ));
        }
        doc.export_animation("apng", None, 1, 0)
            .expect("export")
            .into_vec()
    }

    #[test]
    fn open_animation_builds_one_frame_per_animation_frame() {
        let doc = Document::open_animation(&three_frame_apng()).expect("open");
        assert_eq!((doc.width(), doc.height()), (2, 1));
        assert_eq!(doc.frame_count(), 3);
        assert_eq!(doc.layer_count(), 1);
        let layer = doc.layer_id_at(0).expect("layer");
        assert_eq!(doc.layer_name(layer), "Layer 1");
        assert_eq!(doc.layer_opacity(layer), 255);
        assert_eq!(doc.layer_blend_mode(layer), "normal");
        let durations: Vec<u16> = (0..3)
            .map(|f| doc.frame_duration(f).expect("frame"))
            .collect();
        assert_eq!(durations, [100, 40, 230]);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0xFF0000FF);
        assert_eq!(doc.pick_color(1, 0, 0).expect("pick"), 0x00FF00FF);
        assert_eq!(doc.pick_color(2, 0, 0).expect("pick"), 0x0000FFFF);
        assert_eq!(doc.pick_color(2, 1, 0).expect("pick") & 0xFF, 0);
        let fresh = Document::new(1, 1).expect("dims");
        assert_eq!(doc.palette_count(), fresh.palette_count());
        assert_eq!(
            doc.palette_color(5).expect("entry"),
            fresh.palette_color(5).expect("entry")
        );
        assert_eq!(doc.undo_depth(), 0, "opening is no undo step");
    }

    #[test]
    fn open_animation_refuses_a_static_or_truncated_file() {
        assert!(Document::open_animation(&png_2x1(0xFF0000FF, 0x0000FF80)).is_err());
        let bytes = three_frame_apng();
        let fdat = bytes
            .windows(4)
            .rposition(|w| w == b"fdAT")
            .expect("last frame");
        assert!(Document::open_animation(&bytes[..fdat + 6]).is_err());
    }

    #[test]
    fn probe_animation_js_reports_size_frames_and_limits() {
        let probe = probe_animation_js(&three_frame_apng())
            .expect("probe")
            .expect("animated");
        assert_eq!(probe.format(), "apng");
        assert_eq!(
            (probe.width(), probe.height(), probe.frame_count()),
            (2, 1, 3)
        );
        assert_eq!(probe.total_pixels(), 6.0);
        assert_eq!(probe.exceeded(), None);
        assert_eq!((probe.max_side(), probe.max_pixels()), (4096, 67_108_864.0));
        assert!(probe_animation_js(&png_2x1(0, 0)).expect("probe").is_none());
    }

    #[test]
    fn import_png_as_layer_adds_a_clipped_layer_on_top_in_one_undo_step() {
        let mut doc = Document::new(1, 1).expect("dims");
        let bytes = png_2x1(0x090909FF, 0x010101FF);
        let id = doc.import_png_as_layer(&bytes, "imported").expect("import");
        assert_eq!(doc.layer_count(), 2);
        assert_eq!(doc.layer_name(id), "imported");
        assert_eq!(doc.paint_target_layer_id(), id);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick"), 0x090909FF);
        assert_eq!(doc.layer_id_at(1).expect("id"), id, "on top of the stack");
        assert_eq!(doc.undo_depth(), 1, "one undo step");
        assert!(doc.undo(), "undo the import");
        assert_eq!(doc.layer_count(), 1);
        assert_eq!(doc.pick_color(0, 0, 0).expect("pick") & 0xFF, 0);
        assert!(!doc.undo(), "nothing left to undo");
    }
}
