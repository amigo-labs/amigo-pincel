//! The animation canvas every format draws its frames onto
//! (docs/specs/animated-image-import.md AC-04): it starts fully
//! transparent, each frame is placed at its offset (clipped to the canvas)
//! and blended, the canvas is taken as the frame's image, then the frame's
//! disposal applies to its rectangle.

use super::ImportedFrame;

/// How a frame's pixels combine with the canvas below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Blend {
    /// The frame's pixels replace the canvas, alpha included.
    Source,
    /// Straight-alpha "over": transparent pixels keep the canvas.
    Over,
}

/// What happens to the frame's rectangle after the frame is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dispose {
    /// Leave the canvas as it is.
    None,
    /// Clear the rectangle to fully transparent.
    Background,
    /// Restore the rectangle to what it was before the frame.
    Previous,
}

/// One decoded frame before compositing: straight RGBA8 pixels of a
/// `width × height` rectangle at `(x, y)` on the canvas.
pub(super) struct RawFrame {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub blend: Blend,
    pub dispose: Dispose,
    pub duration_ms: u16,
}

/// The canvas plus the frames composited so far.
pub(super) struct Compositor {
    width: u32,
    height: u32,
    canvas: Vec<u8>,
    frames: Vec<ImportedFrame>,
}

/// The part of a frame rectangle inside the canvas, in canvas and frame
/// coordinates.
struct Clip {
    /// First canvas column / row covered.
    cx: usize,
    cy: usize,
    /// First frame column / row inside the canvas.
    fx: usize,
    fy: usize,
    /// Covered columns / rows.
    w: usize,
    h: usize,
}

impl Compositor {
    /// A fully transparent `width × height` canvas.
    pub(super) fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            canvas: vec![0; width as usize * height as usize * 4],
            frames: Vec::new(),
        }
    }

    /// Draw `frame`, record the canvas as the next frame's image, then
    /// apply its disposal.
    pub(super) fn push(&mut self, frame: RawFrame) {
        let clip = self.clip(&frame);
        let saved = match (&clip, frame.dispose) {
            (Some(c), Dispose::Previous) => Some(self.copy_rect(c)),
            _ => None,
        };
        if let Some(c) = &clip {
            self.draw(c, &frame);
        }
        self.frames.push(ImportedFrame {
            rgba: self.canvas.clone(),
            duration_ms: frame.duration_ms,
        });
        let Some(c) = clip else { return };
        match frame.dispose {
            Dispose::None => {}
            Dispose::Background => self.clear_rect(&c),
            Dispose::Previous => {
                if let Some(saved) = saved {
                    self.paste_rect(&c, &saved);
                }
            }
        }
    }

    /// The frames composited so far.
    pub(super) fn into_frames(self) -> Vec<ImportedFrame> {
        self.frames
    }

    fn clip(&self, f: &RawFrame) -> Option<Clip> {
        let x0 = f.x.max(0);
        let y0 = f.y.max(0);
        let x1 = (f.x + i64::from(f.width)).min(i64::from(self.width));
        let y1 = (f.y + i64::from(f.height)).min(i64::from(self.height));
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        let to_usize = |v: i64| usize::try_from(v).ok();
        Some(Clip {
            cx: to_usize(x0)?,
            cy: to_usize(y0)?,
            fx: to_usize(x0 - f.x)?,
            fy: to_usize(y0 - f.y)?,
            w: to_usize(x1 - x0)?,
            h: to_usize(y1 - y0)?,
        })
    }

    fn row_range(&self, c: &Clip, row: usize) -> std::ops::Range<usize> {
        let start = ((c.cy + row) * self.width as usize + c.cx) * 4;
        start..start + c.w * 4
    }

    fn copy_rect(&self, c: &Clip) -> Vec<u8> {
        let mut out = Vec::with_capacity(c.w * c.h * 4);
        for row in 0..c.h {
            out.extend_from_slice(&self.canvas[self.row_range(c, row)]);
        }
        out
    }

    fn paste_rect(&mut self, c: &Clip, saved: &[u8]) {
        for row in 0..c.h {
            let range = self.row_range(c, row);
            let src = row * c.w * 4;
            self.canvas[range].copy_from_slice(&saved[src..src + c.w * 4]);
        }
    }

    fn clear_rect(&mut self, c: &Clip) {
        for row in 0..c.h {
            let range = self.row_range(c, row);
            self.canvas[range].fill(0);
        }
    }

    fn draw(&mut self, c: &Clip, f: &RawFrame) {
        let frame_stride = f.width as usize * 4;
        for row in 0..c.h {
            let src_start = (c.fy + row) * frame_stride + c.fx * 4;
            let src = &f.rgba[src_start..src_start + c.w * 4];
            let range = self.row_range(c, row);
            let dst = &mut self.canvas[range];
            match f.blend {
                Blend::Source => dst.copy_from_slice(src),
                Blend::Over => {
                    for (d, s) in dst
                        .as_chunks_mut::<4>()
                        .0
                        .iter_mut()
                        .zip(src.as_chunks::<4>().0)
                    {
                        *d = over(*s, *d);
                    }
                }
            }
        }
    }
}

/// Straight-alpha source-over in integer arithmetic, rounded to nearest:
/// `a = sa + da·(255 − sa)/255`, `c = (sc·sa + dc·da·(255 − sa)/255) / a`.
pub(super) fn over(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    let sa = u32::from(src[3]);
    let da = u32::from(dst[3]);
    if sa == 0 {
        return dst;
    }
    if sa == 255 || da == 0 {
        return src;
    }
    // Everything scaled by 255: alpha·255 = sa·255 + da·(255 − sa).
    let dst_weight = da * (255 - sa);
    let alpha_255 = sa * 255 + dst_weight;
    let channel = |i: usize| {
        let num = u32::from(src[i]) * sa * 255 + u32::from(dst[i]) * dst_weight;
        u8::try_from((num + alpha_255 / 2) / alpha_255).unwrap_or(u8::MAX)
    };
    let alpha = u8::try_from((alpha_255 + 127) / 255).unwrap_or(u8::MAX);
    [channel(0), channel(1), channel(2), alpha]
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: [u8; 4] = [255, 0, 0, 255];
    const G: [u8; 4] = [0, 255, 0, 255];
    const T: [u8; 4] = [0, 0, 0, 0];

    fn frame(x: i64, y: i64, w: u32, h: u32, px: &[[u8; 4]]) -> RawFrame {
        RawFrame {
            x,
            y,
            width: w,
            height: h,
            rgba: px.concat(),
            blend: Blend::Over,
            dispose: Dispose::None,
            duration_ms: 100,
        }
    }

    fn pixels(f: &ImportedFrame) -> Vec<[u8; 4]> {
        f.rgba.as_chunks::<4>().0.to_vec()
    }

    #[test]
    fn push_offset_frame_is_clipped_to_the_canvas() {
        let mut c = Compositor::new(2, 2);
        c.push(frame(1, 1, 2, 2, &[R, R, R, R]));
        c.push(frame(-1, -1, 2, 2, &[G, G, G, G]));
        let frames = c.into_frames();
        assert_eq!(pixels(&frames[0]), [T, T, T, R]);
        assert_eq!(pixels(&frames[1]), [G, T, T, R]);
    }

    #[test]
    fn push_background_disposal_clears_the_rectangle_after_the_frame() {
        let mut c = Compositor::new(2, 1);
        c.push(frame(0, 0, 2, 1, &[R, R]));
        let mut f = frame(1, 0, 1, 1, &[G]);
        f.dispose = Dispose::Background;
        c.push(f);
        c.push(frame(0, 0, 1, 1, &[T]));
        let frames = c.into_frames();
        assert_eq!(pixels(&frames[1]), [R, G]);
        assert_eq!(pixels(&frames[2]), [R, T]);
    }

    #[test]
    fn push_previous_disposal_restores_the_rectangle() {
        let mut c = Compositor::new(2, 1);
        c.push(frame(0, 0, 2, 1, &[R, R]));
        let mut f = frame(0, 0, 2, 1, &[G, T]);
        f.dispose = Dispose::Previous;
        c.push(f);
        c.push(frame(0, 0, 1, 1, &[T]));
        let frames = c.into_frames();
        assert_eq!(pixels(&frames[1]), [G, R]);
        assert_eq!(pixels(&frames[2]), [R, R]);
    }

    #[test]
    fn push_source_blend_replaces_alpha_too() {
        let mut c = Compositor::new(1, 1);
        c.push(frame(0, 0, 1, 1, &[R]));
        let mut f = frame(0, 0, 1, 1, &[T]);
        f.blend = Blend::Source;
        c.push(f);
        assert_eq!(pixels(&c.into_frames()[1]), [T]);
    }

    #[test]
    fn over_partial_alpha_mixes_by_weight() {
        // 50 % blue over opaque red: half each, opaque.
        assert_eq!(over([0, 0, 255, 128], R), [127, 0, 128, 255]);
        // Over transparent: the source unchanged.
        assert_eq!(over([0, 0, 255, 128], T), [0, 0, 255, 128]);
        // 50 % over 50 %: alpha 1 − 0.5·0.5 = 0.75.
        assert_eq!(over([0, 0, 255, 128], [255, 0, 0, 128]), [85, 0, 170, 192]);
    }
}
