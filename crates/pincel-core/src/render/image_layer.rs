//! Image-cel composition path. See `docs/specs/pincel.md` §4.

use crate::document::PixelBuffer;
use crate::geometry::Rect;

use super::blend::{blend_normal_into, mul_u8};

pub(super) fn composite_image_cel(
    dst: &mut [u8],
    viewport: Rect,
    cel_pos: (i32, i32),
    src: &PixelBuffer,
    layer_opacity: u8,
    cel_opacity: u8,
) {
    let combined_opacity = mul_u8(layer_opacity, cel_opacity);
    if combined_opacity == 0 {
        return;
    }

    let vp_w = i64::from(viewport.width);
    let vp_h = i64::from(viewport.height);
    let vp_x = i64::from(viewport.x);
    let vp_y = i64::from(viewport.y);

    let cel_w = i64::from(src.width);
    let cel_h = i64::from(src.height);
    let cel_x = i64::from(cel_pos.0);
    let cel_y = i64::from(cel_pos.1);

    // Intersection of cel and viewport, in sprite coordinates.
    let x_start = cel_x.max(vp_x);
    let y_start = cel_y.max(vp_y);
    let x_end = (cel_x + cel_w).min(vp_x + vp_w);
    let y_end = (cel_y + cel_h).min(vp_y + vp_h);

    if x_start >= x_end || y_start >= y_end {
        return;
    }

    let src_stride = (src.width as usize) * 4;
    let dst_stride = (viewport.width as usize) * 4;

    for y in y_start..y_end {
        let src_row = (y - cel_y) as usize * src_stride;
        let dst_row = (y - vp_y) as usize * dst_stride;
        for x in x_start..x_end {
            let s = src_row + (x - cel_x) as usize * 4;
            let d = dst_row + (x - vp_x) as usize * 4;
            let sa = mul_u8(src.data[s + 3], combined_opacity);
            blend_normal_into(
                &mut dst[d..d + 4],
                src.data[s],
                src.data[s + 1],
                src.data[s + 2],
                sa,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, CelMap, ColorMode, Frame, FrameIndex, Layer, LayerId, Sprite};
    use crate::render::compose::compose;
    use crate::render::request::ComposeRequest;
    use crate::render::test_support::{full_req, one_layer_sprite, solid};

    #[test]
    fn opaque_cel_matches_source() {
        let sprite = one_layer_sprite(2, 2, 1);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 2, [10, 20, 30, 255]),
        ));

        let r = compose(&sprite, &cels, &full_req(2, 2)).unwrap();
        assert_eq!((r.width, r.height), (2, 2));
        assert_eq!(r.pixels, [10u8, 20, 30, 255].repeat(4));
    }

    #[test]
    fn layer_opacity_scales_alpha() {
        let mut layer = Layer::image(LayerId::new(0), "bg");
        layer.opacity = 128;
        let sprite = Sprite::builder(1, 1)
            .add_layer(layer)
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        ));
        let r = compose(&sprite, &cels, &full_req(1, 1)).unwrap();
        // mul_u8(128, 255) = 128, so out alpha = 128 over transparent backdrop.
        assert_eq!(r.pixels, vec![255, 0, 0, 128]);
    }

    #[test]
    fn cel_opacity_scales_alpha() {
        let sprite = one_layer_sprite(1, 1, 1);
        let mut cel = Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [255, 0, 0, 255]),
        );
        cel.opacity = 64;
        let mut cels = CelMap::new();
        cels.insert(cel);
        let r = compose(&sprite, &cels, &full_req(1, 1)).unwrap();
        // mul_u8(255, 64) → (255*64 + 127)/255 = 16447/255 = 64.
        assert_eq!(r.pixels[3], 64);
    }

    #[test]
    fn cel_clipped_to_viewport() {
        let sprite = one_layer_sprite(4, 4, 1);
        let mut cel = Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 2, [100, 100, 100, 255]),
        );
        cel.position = (-1, -1);
        let mut cels = CelMap::new();
        cels.insert(cel);
        let r = compose(&sprite, &cels, &full_req(4, 4)).unwrap();
        // Cel covers sprite coords (-1..1, -1..1); only pixel (0,0) is inside the canvas.
        assert_eq!(&r.pixels[0..4], &[100, 100, 100, 255]);
        assert_eq!(&r.pixels[4..8], &[0, 0, 0, 0]); // (1, 0) is outside cel
    }

    #[test]
    fn viewport_offset_renders_subregion() {
        let sprite = one_layer_sprite(4, 4, 1);
        let mut buf = PixelBuffer::empty(4, 4, ColorMode::Rgba);
        // Mark pixel (2, 2) red, the rest white.
        for px in buf.data.chunks_exact_mut(4) {
            px.copy_from_slice(&[255, 255, 255, 255]);
        }
        let idx = (2 * 4 + 2) * 4;
        buf.data[idx..idx + 4].copy_from_slice(&[255, 0, 0, 255]);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));

        let req = ComposeRequest {
            viewport: Rect::new(2, 2, 2, 2),
            ..ComposeRequest::full(FrameIndex::new(0), 4, 4)
        };
        let r = compose(&sprite, &cels, &req).unwrap();
        assert_eq!((r.width, r.height), (2, 2));
        assert_eq!(&r.pixels[0..4], &[255, 0, 0, 255]);
        assert_eq!(&r.pixels[4..8], &[255, 255, 255, 255]);
    }
}
