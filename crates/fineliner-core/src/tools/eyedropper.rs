//! The Eyedropper tool (spec §9.2).
//!
//! Samples a color from the canvas to become the active color. Sampling reads
//! either the current layer or the flattened composite, optionally averaging a
//! square neighborhood. The eyedropper emits no command — color changes are not
//! undoable (spec §9.2 Eyedropper).

use super::SampleSource;
use crate::color::Color;
use crate::document::{Document, ImageBuffer};
use crate::geometry::Point;
use crate::render::compose;

/// Size of the square neighborhood the eyedropper averages (spec §9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SampleSize {
    /// A single pixel — no averaging.
    #[default]
    One,
    /// 3×3 average.
    ThreeByThree,
    /// 5×5 average.
    FiveByFive,
    /// 11×11 average.
    ElevenByEleven,
    /// 31×31 average.
    ThirtyOneByThirtyOne,
}

impl SampleSize {
    /// Edge length of the sampling square in pixels.
    pub fn edge(self) -> u32 {
        match self {
            SampleSize::One => 1,
            SampleSize::ThreeByThree => 3,
            SampleSize::FiveByFive => 5,
            SampleSize::ElevenByEleven => 11,
            SampleSize::ThirtyOneByThirtyOne => 31,
        }
    }
}

/// The Eyedropper tool — samples a color from the canvas (spec §9.2).
#[derive(Debug, Clone, Copy, Default)]
pub struct Eyedropper {
    /// Whether to read the current layer or the flattened composite.
    pub sample: SampleSource,
    /// Neighborhood size to average over.
    pub size: SampleSize,
}

impl Eyedropper {
    /// Creates an eyedropper with the given sample source and size.
    pub fn new(sample: SampleSource, size: SampleSize) -> Self {
        Self { sample, size }
    }

    /// Samples the color at `point`, averaging the configured neighborhood.
    ///
    /// `CurrentLayer` reads the active layer; `AllLayers` reads the composite.
    /// Out-of-canvas pixels in the neighborhood are skipped, so sampling near an
    /// edge averages only the in-bounds pixels. Returns `None` only when no
    /// pixel of the neighborhood lies on the canvas (e.g. the point is well off
    /// the canvas).
    pub fn pick(&self, point: Point, doc: &Document) -> Option<Color> {
        // Borrow the active layer directly; only the composite needs allocating.
        let composite;
        let buf: &ImageBuffer = match self.sample {
            SampleSource::CurrentLayer => &doc.active_layer().pixels,
            SampleSource::AllLayers => {
                composite = compose(doc.layers());
                &composite
            }
        };

        let cx = point.x.floor() as i32;
        let cy = point.y.floor() as i32;
        let half = (self.size.edge() / 2) as i32;

        let mut sum = [0u32; 4];
        let mut count = 0u32;
        for dy in -half..=half {
            for dx in -half..=half {
                let x = cx + dx;
                let y = cy + dy;
                if x < 0 || y < 0 {
                    continue;
                }
                if let Some(c) = buf.get_pixel(x as u32, y as u32) {
                    sum[0] += c.r as u32;
                    sum[1] += c.g as u32;
                    sum[2] += c.b as u32;
                    sum[3] += c.a as u32;
                    count += 1;
                }
            }
        }
        if count == 0 {
            return None;
        }
        let avg = |i: usize| ((sum[i] + count / 2) / count) as u8;
        Some(Color::rgba(avg(0), avg(1), avg(2), avg(3)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ImageBuffer;

    fn doc_with(w: u32, h: u32, paint: impl Fn(&mut ImageBuffer)) -> Document {
        let mut doc = Document::new(w, h).unwrap();
        paint(&mut doc.layers[0].pixels);
        doc
    }

    #[test]
    fn pick_one_returns_exact_pixel() {
        let c = Color::rgba(10, 20, 30, 40);
        let doc = doc_with(4, 4, |b| b.set_pixel(2, 1, c));
        let eye = Eyedropper::new(SampleSource::CurrentLayer, SampleSize::One);
        assert_eq!(eye.pick(Point::new(2.5, 1.5), &doc), Some(c));
    }

    #[test]
    fn pick_three_by_three_averages_neighborhood() {
        // Center pixel 90, the eight neighbors 0 → average over 9 = 10.
        let doc = doc_with(3, 3, |b| b.set_pixel(1, 1, Color::rgba(90, 90, 90, 90)));
        let eye = Eyedropper::new(SampleSource::CurrentLayer, SampleSize::ThreeByThree);
        let got = eye.pick(Point::new(1.5, 1.5), &doc).unwrap();
        assert_eq!(got, Color::rgba(10, 10, 10, 10));
    }

    #[test]
    fn pick_at_corner_ignores_out_of_bounds() {
        // 3×3 at the top-left corner only covers the 4 in-bounds pixels.
        // One of them is 200, the others 0 → average over 4 = 50.
        let doc = doc_with(4, 4, |b| b.set_pixel(0, 0, Color::rgba(200, 200, 200, 200)));
        let eye = Eyedropper::new(SampleSource::CurrentLayer, SampleSize::ThreeByThree);
        let got = eye.pick(Point::new(0.5, 0.5), &doc).unwrap();
        assert_eq!(got, Color::rgba(50, 50, 50, 50));
    }

    #[test]
    fn pick_off_canvas_returns_none() {
        let doc = Document::new(4, 4).unwrap();
        let eye = Eyedropper::new(SampleSource::CurrentLayer, SampleSize::One);
        assert!(eye.pick(Point::new(-5.0, -5.0), &doc).is_none());
        assert!(eye.pick(Point::new(10.0, 10.0), &doc).is_none());
    }

    #[test]
    fn pick_composite_sees_lower_layer_through_transparent_top() {
        // Bottom layer opaque white; transparent top layer is active.
        let mut doc = Document::new(2, 2).unwrap();
        for y in 0..2 {
            for x in 0..2 {
                doc.layers[0].pixels.set_pixel(x, y, Color::WHITE);
            }
        }
        doc.add_layer("Top").unwrap();

        let current = Eyedropper::new(SampleSource::CurrentLayer, SampleSize::One);
        assert_eq!(
            current.pick(Point::new(0.5, 0.5), &doc),
            Some(Color::TRANSPARENT)
        );

        let composite = Eyedropper::new(SampleSource::AllLayers, SampleSize::One);
        assert_eq!(
            composite.pick(Point::new(0.5, 0.5), &doc),
            Some(Color::WHITE)
        );
    }
}
