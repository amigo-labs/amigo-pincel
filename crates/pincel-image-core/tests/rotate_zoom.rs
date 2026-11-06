//! Layer ▸ Rotate / Zoom through the editor's effect path
//! (`jobs::effect_region` + one `SetPixels`), checked against the editor's
//! exact rotations and the selection rules
//! (docs/specs/image-transform-handles-rotate-zoom.md AC-06, AC-07, AC-11).

use pincel_effects::EffectSpec;
use pincel_image_core::jobs::effect_region;
use pincel_image_core::{
    CanvasRotation, Color, CommandBus, Document, ImageBuffer, Rect, RotateCanvas, RotateLayer90,
    SelectionMask, SetPixels,
};

/// A one-layer document whose layer has a distinct colour per pixel, some
/// of them semi-transparent.
fn bus(w: u32, h: u32) -> CommandBus {
    let mut patch = ImageBuffer::new_transparent(w, h);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let a = if i.is_multiple_of(3) { 140 } else { 255 };
            patch.set_pixel(
                x,
                y,
                Color::rgba((x * 31) as u8, (y * 23) as u8, i as u8, a),
            );
        }
    }
    let mut bus = CommandBus::new(Document::new(w, h).expect("doc"));
    bus.apply(Box::new(SetPixels::new(0, Rect::new(0, 0, w, h), patch)))
        .expect("pattern");
    bus.history.clear();
    bus
}

/// The editor's Apply: the effect's region of layer 0, committed as one
/// `SetPixels` (as `pincel-image-wasm`'s `apply_effect` does).
fn apply(bus: &mut CommandBus, params: &[f64]) {
    let spec = EffectSpec::from_params("rotate_zoom", params).expect("spec");
    let region = effect_region(&bus.document, 0, &spec).expect("region");
    if let Some((rect, pixels)) = region {
        bus.apply(Box::new(SetPixels::new(0, rect, pixels)))
            .expect("apply");
    }
}

fn layer(bus: &CommandBus) -> Vec<u8> {
    bus.document.layers()[0].pixels.data().to_vec()
}

#[test]
fn rotate_zoom_nearest_quarter_turn_equals_rotate_layer_90_cw() {
    for size in [4, 5, 6, 7] {
        let mut via_effect = bus(size, size);
        apply(&mut via_effect, &[90.0, 0.0, 0.0, 1.0, 0.0]);
        let mut via_command = bus(size, size);
        via_command
            .apply(Box::new(RotateLayer90::new(0, false)))
            .expect("rotate");
        assert_eq!(layer(&via_effect), layer(&via_command), "{size} x {size}");
    }
}

#[test]
fn rotate_zoom_nearest_half_turn_equals_rotate_canvas_180() {
    // Even and odd widths and heights in every combination.
    for (w, h) in [(4, 6), (5, 3), (6, 5), (5, 4), (1, 1)] {
        let mut via_command = bus(w, h);
        via_command
            .apply(Box::new(RotateCanvas::new(CanvasRotation::Rotate180)))
            .expect("rotate");
        for angle in [180.0, -180.0] {
            let mut via_effect = bus(w, h);
            apply(&mut via_effect, &[angle, 0.0, 0.0, 1.0, 0.0]);
            assert_eq!(
                layer(&via_effect),
                layer(&via_command),
                "{w} x {h} at {angle}"
            );
        }
    }
}

#[test]
fn rotate_zoom_nearest_one_pixel_pan_shifts_every_pixel_right() {
    for (w, h) in [(4, 3), (7, 2), (3, 5)] {
        let mut b = bus(w, h);
        let before = b.document.layers()[0].pixels.clone();
        apply(&mut b, &[0.0, 100.0 / f64::from(w), 0.0, 1.0, 0.0]);
        let after = &b.document.layers()[0].pixels;
        for y in 0..h {
            assert_eq!(after.get_pixel(0, y), Some(Color::TRANSPARENT), "{w} x {h}");
            for x in 1..w {
                assert_eq!(
                    after.get_pixel(x, y),
                    before.get_pixel(x - 1, y),
                    "{w} x {h}"
                );
            }
        }
    }
}

#[test]
fn rotate_zoom_in_a_feathered_selection_blends_by_coverage_and_reads_the_whole_layer() {
    let (w, h) = (8, 8);
    let mut b = bus(w, h);
    let before = b.document.layers()[0].pixels.clone();
    // Right half selected; its left column at half coverage (a feather).
    let mut mask = SelectionMask::new_empty(w, h);
    for y in 0..h {
        mask.set(4, y, 128);
        for x in 5..w {
            mask.set(x, y, 255);
        }
    }
    b.document.selection = Some(mask);
    apply(&mut b, &[180.0, 0.0, 0.0, 1.0, 0.0]);
    let after = &b.document.layers()[0].pixels;
    let lerp = |o: u8, n: u8, cov: u32| {
        ((u32::from(o) * (255 - cov) + u32::from(n) * cov + 127) / 255) as u8
    };
    for y in 0..h {
        for x in 0..w {
            let old = before.get_pixel(x, y).expect("pixel");
            let got = after.get_pixel(x, y).expect("pixel");
            // The half turn reads from the unselected left half.
            let new = before.get_pixel(w - 1 - x, h - 1 - y).expect("pixel");
            let want = match x {
                0..4 => old,
                4 => Color::rgba(
                    lerp(old.r, new.r, 128),
                    lerp(old.g, new.g, 128),
                    lerp(old.b, new.b, 128),
                    lerp(old.a, new.a, 128),
                ),
                _ => new,
            };
            assert_eq!(got, want, "({x}, {y})");
        }
    }
}
