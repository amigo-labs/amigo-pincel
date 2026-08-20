//! PNG export. See `docs/specs/pincel.md` §7 and CLAUDE.md §5.1.
//!
//! Two entry points, both driven by [`crate::render::compose`] so that what
//! ships in a PNG is byte-identical to what the editor shows on screen:
//!
//! - [`export_frame_png`] — one frame, full canvas, zoom 1.
//! - [`export_atlas_png`] — several frames packed into a fixed grid, plus an
//!   [`AtlasManifest`] describing where each frame landed.
//!
//! Packing is deliberately boring: a fixed-cell grid with a caller-chosen
//! column count and derived row count. No bin-packing, no trimming, no
//! rotation — a consumer can compute a frame's rect from its index alone,
//! and the manifest spells it out anyway.
//!
//! Both entry points inherit the limits of `compose()`: RGBA sprites with
//! `BlendMode::Normal` layers only. See the per-function docs.

use thiserror::Error;

use crate::document::{CelMap, FrameIndex, Sprite};
use crate::render::{ComposeRequest, RenderError, compose};

/// Errors raised by the PNG export path.
#[derive(Debug, Error)]
pub enum ExportError {
    /// Composition failed. Carries the underlying [`RenderError`] verbatim so
    /// that the renderer's current limits (non-RGBA color modes, non-`Normal`
    /// blend modes, group layers, …) surface unaltered to the caller instead
    /// of being flattened into a generic "export failed".
    #[error(transparent)]
    Render(#[from] RenderError),

    /// The `png` encoder rejected the buffer or failed while writing. The
    /// export path writes to an in-memory `Vec<u8>`, so this is practically
    /// limited to malformed dimension / buffer-length combinations.
    #[error("png encode failed: {0}")]
    Encode(String),

    /// The selected frame range is empty — either the sprite has no frames at
    /// all, or the requested tag covers none.
    #[error("no frames to export")]
    NoFrames,

    /// `AtlasOptions::columns` was zero. A grid needs at least one column.
    #[error("atlas columns must be at least 1")]
    InvalidColumns,

    /// [`AtlasOptions::tag`] named a tag that is not on the sprite.
    #[error("tag {name:?} not found in sprite.tags")]
    TagNotFound {
        /// The requested tag name.
        name: String,
    },

    /// A tag's `from..=to` range is not a valid range over `Sprite::frames`
    /// (inverted, or reaching past the last frame). Indicates a corrupt
    /// document — the read path keeps tag ranges in sync with the frame count.
    #[error("tag {name:?} frame range {from}..={to} is not valid over {frames} frame(s)")]
    TagRangeInvalid {
        /// The requested tag name.
        name: String,
        /// The tag's first frame.
        from: u32,
        /// The tag's last frame.
        to: u32,
        /// Number of frames on the sprite.
        frames: u32,
    },

    /// The derived atlas is larger than an addressable RGBA8 buffer on this
    /// target (`columns * width`, `rows * height`, or the total byte count
    /// does not fit).
    #[error("atlas dimensions {width}x{height} are too large to encode")]
    AtlasTooLarge {
        /// Derived atlas width in pixels.
        width: u64,
        /// Derived atlas height in pixels.
        height: u64,
    },
}

/// Knobs for [`export_atlas_png`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasOptions {
    /// Requested number of grid columns. Clamped down to the number of
    /// exported frames so a 3-frame sprite asked for 8 columns produces a
    /// 3×1 grid rather than five empty cells. Rows are then derived as
    /// `ceil(frames / columns)`. The effective value is reported by
    /// [`AtlasManifest::columns`].
    pub columns: u32,
    /// When `Some`, export only the frames covered by the named tag
    /// (inclusive of both endpoints). When `None`, export every frame of the
    /// sprite in playback order.
    pub tag: Option<String>,
}

impl AtlasOptions {
    /// Options for an every-frame atlas with the given column count.
    pub fn new(columns: u32) -> Self {
        Self { columns, tag: None }
    }

    /// Restrict the export to the frames of the named tag.
    pub fn with_tag(mut self, name: impl Into<String>) -> Self {
        self.tag = Some(name.into());
        self
    }
}

/// Where one frame landed in the atlas, in atlas pixel coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasFrame {
    /// Source frame index in `Sprite::frames`.
    pub frame: FrameIndex,
    /// Left edge of the frame's cell.
    pub x: u32,
    /// Top edge of the frame's cell.
    pub y: u32,
    /// Cell width — always the sprite's canvas width (no trimming).
    pub w: u32,
    /// Cell height — always the sprite's canvas height (no trimming).
    pub h: u32,
    /// Name of the first tag whose range covers `frame`, if any.
    pub tag: Option<String>,
}

/// The sidecar description of an atlas: everything a game engine needs to
/// slice the PNG back into frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasManifest {
    /// Atlas width in pixels (`columns * frame_width`).
    pub width: u32,
    /// Atlas height in pixels (`rows * frame_height`).
    pub height: u32,
    /// Effective column count (see [`AtlasOptions::columns`]).
    pub columns: u32,
    /// Derived row count, `ceil(frames.len() / columns)`.
    pub rows: u32,
    /// Cell width — the sprite's canvas width.
    pub frame_width: u32,
    /// Cell height — the sprite's canvas height.
    pub frame_height: u32,
    /// One entry per exported frame, in row-major placement order.
    pub frames: Vec<AtlasFrame>,
}

impl AtlasManifest {
    /// Serialize the manifest as a single-line JSON object with camelCase
    /// keys. `tag` is `null` for frames outside every tag.
    ///
    /// Hand-rolled rather than derived: `pincel-core` deliberately carries no
    /// serialization dependency (CLAUDE.md §5.1), and this shape is small and
    /// stable enough not to need one.
    pub fn to_json(&self) -> String {
        let frames: Vec<String> = self
            .frames
            .iter()
            .map(|f| {
                let tag = match &f.tag {
                    Some(name) => format!("\"{}\"", escape_json(name)),
                    None => "null".to_string(),
                };
                format!(
                    "{{\"frame\":{},\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"tag\":{}}}",
                    f.frame.0, f.x, f.y, f.w, f.h, tag
                )
            })
            .collect();
        format!(
            "{{\"width\":{},\"height\":{},\"columns\":{},\"rows\":{},\
             \"frameWidth\":{},\"frameHeight\":{},\"frames\":[{}]}}",
            self.width,
            self.height,
            self.columns,
            self.rows,
            self.frame_width,
            self.frame_height,
            frames.join(",")
        )
    }
}

/// The result of [`export_atlas_png`]: the image plus its manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasOutput {
    /// Encoded PNG bytes (RGBA8, 8 bits per channel).
    pub png: Vec<u8>,
    /// Placement description for every exported frame.
    pub manifest: AtlasManifest,
}

/// Compose `frame` at zoom 1 over the full canvas and encode it as an RGBA8
/// PNG.
///
/// The pixels are exactly what [`crate::render::compose`] produces for
/// [`ComposeRequest::full`] — visible layers only, no overlays, no onion
/// skin.
///
/// # Limitations
///
/// Inherited from `compose()`: the sprite must use [`crate::ColorMode::Rgba`]
/// and every composited layer must use [`crate::BlendMode::Normal`]. Indexed
/// or grayscale color, any non-`Normal` blend mode, a group layer, or a linked
/// cel fails with [`ExportError::Render`] carrying the underlying
/// [`RenderError`]. Image and tilemap layers both compose. Export does not
/// work around these limits — it widens automatically as `compose()` does.
///
/// # Errors
///
/// [`ExportError::Render`] when composition fails (including an out-of-range
/// `frame`), [`ExportError::Encode`] if the PNG encoder rejects the buffer.
pub fn export_frame_png(
    sprite: &Sprite,
    cels: &CelMap,
    frame: FrameIndex,
) -> Result<Vec<u8>, ExportError> {
    let request = ComposeRequest::full(frame, sprite.width, sprite.height);
    let composed = compose(sprite, cels, &request)?;
    encode_rgba8(&composed.pixels, composed.width, composed.height)
}

/// Pack frames into a fixed-cell grid atlas and encode it as an RGBA8 PNG,
/// returning the image alongside an [`AtlasManifest`] of per-frame source
/// rects.
///
/// Every cell is the full sprite canvas; each frame is composed at zoom 1 and
/// blitted into its cell. Cells past the last frame in the final row stay
/// fully transparent. Frames are placed row-major in playback order, so
/// `index = row * columns + column`.
///
/// # Limitations
///
/// Same as [`export_frame_png`] — every frame goes through `compose()`, so a
/// sprite it refuses (non-RGBA color mode, non-`Normal` blend mode, group
/// layer, linked cel) fails the whole atlas with [`ExportError::Render`]. No
/// trimming, no rotation, and no bin-packing: the grid is the contract.
///
/// # Errors
///
/// [`ExportError::InvalidColumns`] for a zero column count,
/// [`ExportError::TagNotFound`] / [`ExportError::TagRangeInvalid`] for a bad
/// tag filter, [`ExportError::NoFrames`] when the selection is empty,
/// [`ExportError::AtlasTooLarge`] when the derived grid does not fit in an
/// addressable buffer, [`ExportError::Render`] / [`ExportError::Encode`] as
/// for [`export_frame_png`].
pub fn export_atlas_png(
    sprite: &Sprite,
    cels: &CelMap,
    opts: &AtlasOptions,
) -> Result<AtlasOutput, ExportError> {
    if opts.columns == 0 {
        return Err(ExportError::InvalidColumns);
    }
    let selected = select_frames(sprite, opts.tag.as_deref())?;
    let count = u32::try_from(selected.len()).unwrap_or(u32::MAX);
    if count == 0 {
        return Err(ExportError::NoFrames);
    }

    let columns = opts.columns.min(count);
    // `columns >= 1`, so the division cannot trap and the sum cannot wrap.
    let rows = count.div_ceil(columns);

    let cell_w = sprite.width;
    let cell_h = sprite.height;
    let width_64 = u64::from(columns) * u64::from(cell_w);
    let height_64 = u64::from(rows) * u64::from(cell_h);
    let too_large = || ExportError::AtlasTooLarge {
        width: width_64,
        height: height_64,
    };
    let width = u32::try_from(width_64).map_err(|_| too_large())?;
    let height = u32::try_from(height_64).map_err(|_| too_large())?;
    // Both dimensions fit in `u32` by the two checks above, but their product
    // times four still exceeds `u64::MAX` for extreme sprites. An unchecked
    // multiply here would panic (debug) or wrap into an undersized allocation
    // (release) instead of reporting the very condition this guard exists for.
    let byte_len = width_64
        .checked_mul(height_64)
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(too_large)?;

    let mut atlas = vec![0u8; byte_len];
    let atlas_stride = (width as usize) * 4;
    let cell_stride = (cell_w as usize) * 4;
    let mut frames = Vec::with_capacity(selected.len());

    for (slot, &frame) in selected.iter().enumerate() {
        let slot = u32::try_from(slot).unwrap_or(u32::MAX);
        let column = slot % columns;
        let row = slot / columns;
        let origin_x = column * cell_w;
        let origin_y = row * cell_h;

        let request = ComposeRequest::full(frame, cell_w, cell_h);
        let composed = compose(sprite, cels, &request)?;

        // `compose()` with `ComposeRequest::full` at zoom 1 yields exactly
        // `cell_w * cell_h * 4` bytes, so the row copies below are in bounds.
        for y in 0..cell_h as usize {
            let src = y * cell_stride;
            let dst = (origin_y as usize + y) * atlas_stride + (origin_x as usize) * 4;
            atlas[dst..dst + cell_stride].copy_from_slice(&composed.pixels[src..src + cell_stride]);
        }

        frames.push(AtlasFrame {
            frame,
            x: origin_x,
            y: origin_y,
            w: cell_w,
            h: cell_h,
            // An explicit tag filter names the atlas: every exported frame
            // belongs to the requested tag by construction. Falling back to
            // `tag_for_frame` here would label the frames with whichever tag
            // happens to sit first in `sprite.tags`, which is a different tag
            // whenever ranges overlap (an enclosing tag plus per-animation
            // sub-tags is ordinary Aseprite authoring).
            tag: match opts.tag.as_deref() {
                Some(requested) => Some(requested.to_string()),
                None => tag_for_frame(sprite, frame).map(str::to_string),
            },
        });
    }

    Ok(AtlasOutput {
        png: encode_rgba8(&atlas, width, height)?,
        manifest: AtlasManifest {
            width,
            height,
            columns,
            rows,
            frame_width: cell_w,
            frame_height: cell_h,
            frames,
        },
    })
}

/// The frames to export, in playback order.
fn select_frames(sprite: &Sprite, tag: Option<&str>) -> Result<Vec<FrameIndex>, ExportError> {
    let frame_count = u32::try_from(sprite.frames.len()).unwrap_or(u32::MAX);
    let Some(name) = tag else {
        return Ok((0..frame_count).map(FrameIndex::new).collect());
    };
    let Some(tag) = sprite.tags.iter().find(|t| t.name == name) else {
        return Err(ExportError::TagNotFound {
            name: name.to_string(),
        });
    };
    if tag.from > tag.to || tag.to.0 >= frame_count {
        return Err(ExportError::TagRangeInvalid {
            name: tag.name.clone(),
            from: tag.from.0,
            to: tag.to.0,
            frames: frame_count,
        });
    }
    Ok((tag.from.0..=tag.to.0).map(FrameIndex::new).collect())
}

/// Name of the first tag whose range covers `frame`, if any. Aseprite allows
/// overlapping tags; the lowest-index one wins, matching the order the tags
/// were authored in.
fn tag_for_frame(sprite: &Sprite, frame: FrameIndex) -> Option<&str> {
    sprite
        .tags
        .iter()
        .find(|t| t.from <= frame && frame <= t.to)
        .map(|t| t.name.as_str())
}

/// Encode a non-premultiplied RGBA8 buffer as PNG bytes.
fn encode_rgba8(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::new();
    let mut encoder = ::png::Encoder::new(&mut out, width, height);
    encoder.set_color(::png::ColorType::Rgba);
    encoder.set_depth(::png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| ExportError::Encode(e.to_string()))?;
    writer
        .write_image_data(pixels)
        .map_err(|e| ExportError::Encode(e.to_string()))?;
    writer
        .finish()
        .map_err(|e| ExportError::Encode(e.to_string()))?;
    Ok(out)
}

/// Escape a string for embedding in a JSON string literal.
fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        BlendMode, Cel, CelData, ColorMode, Frame, Layer, LayerId, PixelBuffer, Tag, TileImage,
        TileRef, Tileset, TilesetId,
    };

    /// The eight-byte PNG file signature.
    const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    /// A solid-color RGBA buffer.
    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> PixelBuffer {
        let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
        for px in buf.data.chunks_exact_mut(4) {
            px.copy_from_slice(&rgba);
        }
        buf
    }

    /// A `frames`-frame sprite with a single image layer; frame `i` is a
    /// solid `(10 + 10i, 20, 30, 255)` canvas so frames are distinguishable.
    fn striped_sprite(w: u32, h: u32, frames: u32, tags: Vec<Tag>) -> (Sprite, CelMap) {
        let mut builder = Sprite::builder(w, h).add_layer(Layer::image(LayerId::new(0), "art"));
        for _ in 0..frames {
            builder = builder.add_frame(Frame::new(100));
        }
        for tag in tags {
            builder = builder.add_tag(tag);
        }
        let sprite = builder.build().expect("sprite builds");
        let mut cels = CelMap::new();
        for i in 0..frames {
            let shade = u8::try_from(10 + i * 10).unwrap_or(u8::MAX);
            cels.insert(Cel::image(
                LayerId::new(0),
                FrameIndex::new(i),
                solid(w, h, [shade, 20, 30, 255]),
            ));
        }
        (sprite, cels)
    }

    /// Decode PNG bytes back to `(width, height, rgba8)` using the same crate
    /// that wrote them.
    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let decoder = ::png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("png header parses");
        let size = reader.output_buffer_size().expect("known output size");
        let mut buf = vec![0u8; size];
        let info = reader.next_frame(&mut buf).expect("frame decodes");
        assert_eq!(info.color_type, ::png::ColorType::Rgba);
        assert_eq!(info.bit_depth, ::png::BitDepth::Eight);
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    }

    /// The pixels `compose()` produces for one frame at zoom 1.
    fn composed_pixels(sprite: &Sprite, cels: &CelMap, frame: u32) -> Vec<u8> {
        compose(
            sprite,
            cels,
            &ComposeRequest::full(FrameIndex::new(frame), sprite.width, sprite.height),
        )
        .expect("compose succeeds")
        .pixels
    }

    /// Extract the `col`,`row` cell of an atlas as a tight RGBA8 buffer.
    fn cell(atlas: &[u8], atlas_w: u32, cell_w: u32, cell_h: u32, col: u32, row: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity((cell_w * cell_h * 4) as usize);
        for y in 0..cell_h {
            let start = (((row * cell_h + y) * atlas_w + col * cell_w) * 4) as usize;
            out.extend_from_slice(&atlas[start..start + (cell_w * 4) as usize]);
        }
        out
    }

    #[test]
    fn export_frame_png_1x1_starts_with_png_signature() {
        let (sprite, cels) = striped_sprite(1, 1, 1, Vec::new());
        let bytes = export_frame_png(&sprite, &cels, FrameIndex::new(0)).expect("export succeeds");
        assert_eq!(&bytes[..8], &PNG_SIGNATURE);
    }

    #[test]
    fn export_frame_png_1x1_round_trips_to_composed_pixels() {
        let (sprite, cels) = striped_sprite(1, 1, 1, Vec::new());
        let bytes = export_frame_png(&sprite, &cels, FrameIndex::new(0)).expect("export succeeds");
        let (w, h, pixels) = decode(&bytes);
        assert_eq!((w, h), (1, 1));
        assert_eq!(pixels, vec![10, 20, 30, 255]);
        assert_eq!(pixels, composed_pixels(&sprite, &cels, 0));
    }

    #[test]
    fn export_frame_png_preserves_partial_alpha_and_layer_stack() {
        // 2×2 sprite: opaque red base, half-alpha white 1×1 dot on top.
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_layer(Layer::image(LayerId::new(1), "fg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 2, [255, 0, 0, 255]),
        ));
        let mut dot = Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(1, 1, [255, 255, 255, 128]),
        );
        dot.position = (1, 1);
        cels.insert(dot);

        let bytes = export_frame_png(&sprite, &cels, FrameIndex::new(0)).expect("export succeeds");
        let (w, h, pixels) = decode(&bytes);
        assert_eq!((w, h), (2, 2));
        assert_eq!(pixels, composed_pixels(&sprite, &cels, 0));
        // The blended corner is neither of the two source colors.
        assert_ne!(&pixels[12..16], &[255, 0, 0, 255]);
    }

    #[test]
    fn export_frame_png_rejects_frame_past_end() {
        let (sprite, cels) = striped_sprite(2, 2, 1, Vec::new());
        let err = export_frame_png(&sprite, &cels, FrameIndex::new(7)).expect_err("out of range");
        assert!(
            matches!(
                err,
                ExportError::Render(RenderError::UnknownFrame { frame }) if frame == FrameIndex::new(7)
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_atlas_png_packs_four_frames_into_two_by_two_grid() {
        let (sprite, cels) = striped_sprite(3, 2, 4, Vec::new());
        let out = export_atlas_png(&sprite, &cels, &AtlasOptions::new(2)).expect("export succeeds");

        assert_eq!(&out.png[..8], &PNG_SIGNATURE);
        assert_eq!((out.manifest.columns, out.manifest.rows), (2, 2));
        assert_eq!((out.manifest.width, out.manifest.height), (6, 4));
        assert_eq!(
            (out.manifest.frame_width, out.manifest.frame_height),
            (3, 2)
        );

        let expected_rects = [(0, 0), (3, 0), (0, 2), (3, 2)];
        assert_eq!(out.manifest.frames.len(), 4);
        for (i, entry) in out.manifest.frames.iter().enumerate() {
            let (x, y) = expected_rects[i];
            assert_eq!(
                entry.frame,
                FrameIndex::new(u32::try_from(i).expect("small"))
            );
            assert_eq!((entry.x, entry.y, entry.w, entry.h), (x, y, 3, 2));
            assert_eq!(entry.tag, None);
        }

        let (w, h, pixels) = decode(&out.png);
        assert_eq!((w, h), (6, 4));
        for (i, entry) in out.manifest.frames.iter().enumerate() {
            let frame = u32::try_from(i).expect("small");
            let col = entry.x / 3;
            let row = entry.y / 2;
            assert_eq!(
                cell(&pixels, w, 3, 2, col, row),
                composed_pixels(&sprite, &cels, frame),
                "cell for frame {frame} does not match compose()"
            );
        }
    }

    #[test]
    fn export_atlas_png_leaves_unused_trailing_cells_transparent() {
        // 3 frames over 2 columns → 2 rows, one empty cell at (1, 1).
        let (sprite, cels) = striped_sprite(2, 2, 3, Vec::new());
        let out = export_atlas_png(&sprite, &cels, &AtlasOptions::new(2)).expect("export succeeds");

        assert_eq!((out.manifest.columns, out.manifest.rows), (2, 2));
        assert_eq!((out.manifest.width, out.manifest.height), (4, 4));
        assert_eq!(out.manifest.frames.len(), 3);

        let (w, h, pixels) = decode(&out.png);
        assert_eq!((w, h), (4, 4));
        assert!(
            cell(&pixels, w, 2, 2, 1, 1).iter().all(|&b| b == 0),
            "trailing cell should be fully transparent"
        );
        // …and the three occupied cells still match compose().
        for (frame, (col, row)) in [(0, 0), (1, 0), (0, 1)].into_iter().enumerate() {
            let frame = u32::try_from(frame).expect("small");
            assert_eq!(
                cell(&pixels, w, 2, 2, col, row),
                composed_pixels(&sprite, &cels, frame)
            );
        }
    }

    #[test]
    fn export_atlas_png_with_tag_exports_only_that_tags_frames() {
        let tags = vec![
            Tag::new("idle", FrameIndex::new(0), FrameIndex::new(0)),
            Tag::new("walk", FrameIndex::new(1), FrameIndex::new(2)),
        ];
        let (sprite, cels) = striped_sprite(2, 2, 4, tags);
        let out = export_atlas_png(&sprite, &cels, &AtlasOptions::new(4).with_tag("walk"))
            .expect("export succeeds");

        // Only 2 frames selected, so the 4 requested columns clamp to 2.
        assert_eq!((out.manifest.columns, out.manifest.rows), (2, 1));
        assert_eq!((out.manifest.width, out.manifest.height), (4, 2));
        let names: Vec<_> = out.manifest.frames.iter().map(|f| f.tag.clone()).collect();
        assert_eq!(
            names,
            vec![Some("walk".to_string()), Some("walk".to_string())]
        );
        let indices: Vec<_> = out.manifest.frames.iter().map(|f| f.frame).collect();
        assert_eq!(indices, vec![FrameIndex::new(1), FrameIndex::new(2)]);

        let (w, _h, pixels) = decode(&out.png);
        assert_eq!(
            cell(&pixels, w, 2, 2, 0, 0),
            composed_pixels(&sprite, &cels, 1)
        );
        assert_eq!(
            cell(&pixels, w, 2, 2, 1, 0),
            composed_pixels(&sprite, &cels, 2)
        );
    }

    #[test]
    fn export_atlas_png_with_overlapping_tags_labels_frames_with_the_requested_tag() {
        // An enclosing tag plus per-animation sub-tags is ordinary Aseprite
        // authoring, and `all` sorts first. The manifest must name the tag the
        // caller asked to export, not whichever one `sprite.tags` lists first —
        // a consumer builds its animation clips from these names.
        let tags = vec![
            Tag::new("all", FrameIndex::new(0), FrameIndex::new(3)),
            Tag::new("walk", FrameIndex::new(1), FrameIndex::new(2)),
        ];
        let (sprite, cels) = striped_sprite(2, 2, 4, tags);
        let out = export_atlas_png(&sprite, &cels, &AtlasOptions::new(2).with_tag("walk"))
            .expect("export succeeds");

        let indices: Vec<_> = out.manifest.frames.iter().map(|f| f.frame).collect();
        assert_eq!(indices, vec![FrameIndex::new(1), FrameIndex::new(2)]);
        let names: Vec<_> = out.manifest.frames.iter().map(|f| f.tag.clone()).collect();
        assert_eq!(
            names,
            vec![Some("walk".to_string()), Some("walk".to_string())],
            "the enclosing `all` tag must not claim a walk-only atlas"
        );
    }

    #[test]
    fn export_atlas_png_reports_too_large_instead_of_overflowing_the_byte_count() {
        // Both atlas dimensions fit in `u32` here, so only the `* 4` byte count
        // overflows `u64`. The guard must report that, not trap on the multiply.
        // Built without cels on purpose: the size check runs before any frame is
        // composed, and a real pixel buffer at these dimensions is unallocatable.
        let sprite = Sprite::builder(u32::MAX, u32::MAX)
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let err = export_atlas_png(&sprite, &CelMap::new(), &AtlasOptions::new(1))
            .expect_err("an atlas this size cannot be addressed");
        assert!(
            matches!(err, ExportError::AtlasTooLarge { .. }),
            "expected AtlasTooLarge, got {err:?}"
        );
    }

    #[test]
    fn export_atlas_png_untagged_frames_report_no_tag() {
        let tags = vec![Tag::new("walk", FrameIndex::new(0), FrameIndex::new(0))];
        let (sprite, cels) = striped_sprite(1, 1, 2, tags);
        let out = export_atlas_png(&sprite, &cels, &AtlasOptions::new(2)).expect("export succeeds");
        let names: Vec<_> = out.manifest.frames.iter().map(|f| f.tag.clone()).collect();
        assert_eq!(names, vec![Some("walk".to_string()), None]);
    }

    #[test]
    fn export_atlas_png_rejects_zero_columns() {
        let (sprite, cels) = striped_sprite(1, 1, 1, Vec::new());
        let err = export_atlas_png(&sprite, &cels, &AtlasOptions::new(0))
            .expect_err("zero columns rejected");
        assert!(matches!(err, ExportError::InvalidColumns));
    }

    #[test]
    fn export_atlas_png_rejects_unknown_tag() {
        let (sprite, cels) = striped_sprite(1, 1, 1, Vec::new());
        let err = export_atlas_png(&sprite, &cels, &AtlasOptions::new(1).with_tag("nope"))
            .expect_err("unknown tag rejected");
        assert!(
            matches!(&err, ExportError::TagNotFound { name } if name == "nope"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_atlas_png_rejects_tag_range_past_last_frame() {
        let tags = vec![Tag::new("walk", FrameIndex::new(0), FrameIndex::new(5))];
        let (sprite, cels) = striped_sprite(1, 1, 2, tags);
        let err = export_atlas_png(&sprite, &cels, &AtlasOptions::new(1).with_tag("walk"))
            .expect_err("bad tag range rejected");
        assert!(
            matches!(
                &err,
                ExportError::TagRangeInvalid { name, from: 0, to: 5, frames: 2 } if name == "walk"
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_atlas_png_rejects_sprite_without_frames() {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .build()
            .expect("sprite builds");
        let err = export_atlas_png(&sprite, &CelMap::new(), &AtlasOptions::new(2))
            .expect_err("frameless sprite rejected");
        assert!(matches!(err, ExportError::NoFrames));
    }

    #[test]
    fn export_rejects_not_yet_implemented_blend_mode_with_render_error() {
        let mut layer = Layer::image(LayerId::new(0), "art");
        // Any mode `render::blend` has not implemented yet; retarget as modes land.
        layer.blend_mode = BlendMode::Luminosity;
        let sprite = Sprite::builder(2, 2)
            .add_layer(layer)
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(2, 2, [1, 2, 3, 255]),
        ));

        let frame_err =
            export_frame_png(&sprite, &cels, FrameIndex::new(0)).expect_err("blend mode refused");
        assert!(
            matches!(
                frame_err,
                ExportError::Render(RenderError::UnsupportedBlendMode {
                    layer: LayerId(0),
                    mode: BlendMode::Luminosity
                })
            ),
            "unexpected error: {frame_err}"
        );

        let atlas_err = export_atlas_png(&sprite, &cels, &AtlasOptions::new(1))
            .expect_err("blend mode refused");
        assert!(
            matches!(
                atlas_err,
                ExportError::Render(RenderError::UnsupportedBlendMode { .. })
            ),
            "unexpected error: {atlas_err}"
        );
    }

    #[test]
    fn export_rejects_non_rgba_color_mode_with_render_error() {
        let sprite = Sprite::builder(2, 2)
            .color_mode(ColorMode::Indexed {
                transparent_index: 0,
            })
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let err = export_frame_png(&sprite, &CelMap::new(), FrameIndex::new(0))
            .expect_err("indexed color refused");
        assert!(
            matches!(
                err,
                ExportError::Render(RenderError::UnsupportedColorMode {
                    mode: ColorMode::Indexed {
                        transparent_index: 0
                    }
                })
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_rejects_group_layer_with_render_error() {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::group(LayerId::new(0), "folder"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let err = export_frame_png(&sprite, &CelMap::new(), FrameIndex::new(0))
            .expect_err("group layer refused");
        assert!(
            matches!(
                err,
                ExportError::Render(RenderError::UnsupportedLayerKind { layer: LayerId(0) })
            ),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn export_atlas_png_composes_tilemap_layers_like_compose() {
        // Tilemap layers are supported by `compose()`, so they export too.
        let mut tileset = Tileset::new(TilesetId::new(0), "tiles", (2, 2));
        tileset.tiles.push(TileImage {
            pixels: solid(2, 2, [0, 0, 0, 0]),
        });
        tileset.tiles.push(TileImage {
            pixels: solid(2, 2, [9, 8, 7, 255]),
        });
        let sprite = Sprite::builder(2, 2)
            .add_tileset(tileset)
            .add_layer(Layer::tilemap(LayerId::new(0), "tiles", TilesetId::new(0)))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        let mut cel = Cel::tilemap(LayerId::new(0), FrameIndex::new(0), 1, 1);
        cel.data = CelData::Tilemap {
            grid_w: 1,
            grid_h: 1,
            tiles: vec![TileRef::new(1)],
        };
        cels.insert(cel);

        let out = export_atlas_png(&sprite, &cels, &AtlasOptions::new(1)).expect("export succeeds");
        let (w, h, pixels) = decode(&out.png);
        assert_eq!((w, h), (2, 2));
        assert_eq!(pixels, composed_pixels(&sprite, &cels, 0));
        assert_eq!(&pixels[..4], &[9, 8, 7, 255]);
    }

    #[test]
    fn manifest_to_json_emits_camel_case_keys_and_null_tags() {
        let manifest = AtlasManifest {
            width: 4,
            height: 2,
            columns: 2,
            rows: 1,
            frame_width: 2,
            frame_height: 2,
            frames: vec![
                AtlasFrame {
                    frame: FrameIndex::new(0),
                    x: 0,
                    y: 0,
                    w: 2,
                    h: 2,
                    tag: Some("walk".to_string()),
                },
                AtlasFrame {
                    frame: FrameIndex::new(1),
                    x: 2,
                    y: 0,
                    w: 2,
                    h: 2,
                    tag: None,
                },
            ],
        };
        assert_eq!(
            manifest.to_json(),
            "{\"width\":4,\"height\":2,\"columns\":2,\"rows\":1,\
             \"frameWidth\":2,\"frameHeight\":2,\"frames\":[\
             {\"frame\":0,\"x\":0,\"y\":0,\"w\":2,\"h\":2,\"tag\":\"walk\"},\
             {\"frame\":1,\"x\":2,\"y\":0,\"w\":2,\"h\":2,\"tag\":null}]}"
        );
    }

    #[test]
    fn manifest_to_json_escapes_tag_names() {
        let tags = vec![Tag::new(
            "a\"b\\c\nd\u{1}",
            FrameIndex::new(0),
            FrameIndex::new(0),
        )];
        let (sprite, cels) = striped_sprite(1, 1, 1, tags);
        let out = export_atlas_png(&sprite, &cels, &AtlasOptions::new(1)).expect("export succeeds");
        assert!(
            out.manifest
                .to_json()
                .contains("\"tag\":\"a\\\"b\\\\c\\nd\\u0001\""),
            "unescaped JSON: {}",
            out.manifest.to_json()
        );
    }

    #[test]
    fn atlas_options_with_tag_sets_the_filter() {
        let opts = AtlasOptions::new(3).with_tag("run");
        assert_eq!(opts.columns, 3);
        assert_eq!(opts.tag.as_deref(), Some("run"));
    }
}
