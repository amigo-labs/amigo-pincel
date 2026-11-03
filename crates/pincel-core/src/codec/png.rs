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

use super::scale::{scaled_size, upscale_nearest};

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

    /// The requested export scale is outside `1..=10`.
    #[error("export scale {scale} is outside the valid range 1–10")]
    InvalidScale {
        /// The requested scale factor.
        scale: u32,
    },

    /// The scaled output is wider or higher than the per-edge export limit
    /// of 16384 px.
    #[error("scaled output {width}x{height} exceeds the 16384 px per-edge export limit")]
    ScaledTooLarge {
        /// Scaled output width in pixels.
        width: u64,
        /// Scaled output height in pixels.
        height: u64,
    },

    /// The `gif` encoder failed while writing.
    #[error("gif encode failed: {0}")]
    GifEncode(String),
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
    /// Integer nearest-neighbour upscale of the sheet, `1..=10`
    /// (docs/specs/scaled-export.md). `1` exports at the sprite's size.
    pub scale: u32,
}

impl AtlasOptions {
    /// Options for an every-frame atlas with the given column count, at
    /// scale 1.
    pub fn new(columns: u32) -> Self {
        Self {
            columns,
            tag: None,
            scale: 1,
        }
    }

    /// Upscale the sheet by `scale` (validated to `1..=10` on export).
    pub fn with_scale(mut self, scale: u32) -> Self {
        self.scale = scale;
        self
    }

    /// Restrict the export to the frames of the named tag.
    pub fn with_tag(mut self, name: impl Into<String>) -> Self {
        self.tag = Some(name.into());
        self
    }
}

/// Where one frame landed in the atlas, in atlas pixel coordinates (of the
/// scaled sheet when [`AtlasManifest::scale`] is above 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasFrame {
    /// Source frame index in `Sprite::frames`.
    pub frame: FrameIndex,
    /// Left edge of the frame's cell.
    pub x: u32,
    /// Top edge of the frame's cell.
    pub y: u32,
    /// Cell width — always the sprite's canvas width times the scale (no
    /// trimming).
    pub w: u32,
    /// Cell height — always the sprite's canvas height times the scale (no
    /// trimming).
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
    /// Cell width — the sprite's canvas width times [`Self::scale`].
    pub frame_width: u32,
    /// Cell height — the sprite's canvas height times [`Self::scale`].
    pub frame_height: u32,
    /// One entry per exported frame, in row-major placement order.
    pub frames: Vec<AtlasFrame>,
    /// The integer upscale every size and position above already includes
    /// (`1` for an unscaled sheet).
    pub scale: u32,
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
/// Inherited from `compose()`: the sprite must use [`crate::ColorMode::Rgba`].
/// Indexed or grayscale color and linked cels fail with
/// [`ExportError::Render`] carrying the underlying [`RenderError`]. Image and
/// tilemap layers both compose, under every blend mode. Export does not work
/// around these limits — it widens automatically as `compose()` does.
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
    export_frame_png_scaled(sprite, cels, frame, 1)
}

/// [`export_frame_png`] upscaled by an integer `scale` (`1..=10`) with
/// nearest-neighbour, so every sprite pixel becomes a `scale × scale`
/// block of exactly its RGBA value (docs/specs/scaled-export.md). Scale 1
/// gives the same bytes as [`export_frame_png`].
///
/// # Errors
///
/// [`ExportError::InvalidScale`] for a scale outside `1..=10`,
/// [`ExportError::ScaledTooLarge`] when an edge of the output would exceed
/// 16384 px, otherwise as [`export_frame_png`].
pub fn export_frame_png_scaled(
    sprite: &Sprite,
    cels: &CelMap,
    frame: FrameIndex,
    scale: u32,
) -> Result<Vec<u8>, ExportError> {
    let (out_w, out_h) = scaled_size(sprite.width, sprite.height, scale)?;
    let request = ComposeRequest::full(frame, sprite.width, sprite.height);
    let mut pixels = Vec::new();
    let composed = compose(sprite, cels, &request, &mut pixels)?;
    if scale == 1 {
        return encode_png(&pixels, composed.width, composed.height);
    }
    let mut scaled = Vec::new();
    upscale_nearest(&pixels, composed.width, composed.height, scale, &mut scaled);
    encode_png(&scaled, out_w, out_h)
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
    // Refuse a bad scale before composing anything.
    scaled_size(1, 1, opts.scale)?;
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
    let scale = opts.scale;
    let (out_w, out_h) = scaled_size(width, height, scale)?;

    let mut atlas = vec![0u8; byte_len];
    let atlas_stride = (width as usize) * 4;
    let cell_stride = (cell_w as usize) * 4;
    let mut frames = Vec::with_capacity(selected.len());
    // One scratch buffer for the whole loop: `compose()` writes into a
    // caller-owned `Vec` and every cell is the same size.
    let mut cell = Vec::new();

    for (slot, &frame) in selected.iter().enumerate() {
        let slot = u32::try_from(slot).unwrap_or(u32::MAX);
        let column = slot % columns;
        let row = slot / columns;
        let origin_x = column * cell_w;
        let origin_y = row * cell_h;

        let request = ComposeRequest::full(frame, cell_w, cell_h);
        compose(sprite, cels, &request, &mut cell)?;

        // `compose()` with `ComposeRequest::full` at zoom 1 yields exactly
        // `cell_w * cell_h * 4` bytes, so the row copies below are in bounds.
        for y in 0..cell_h as usize {
            let src = y * cell_stride;
            let dst = (origin_y as usize + y) * atlas_stride + (origin_x as usize) * 4;
            atlas[dst..dst + cell_stride].copy_from_slice(&cell[src..src + cell_stride]);
        }

        // Within 16384 px by `scaled_size` above, so these cannot wrap.
        frames.push(AtlasFrame {
            frame,
            x: origin_x * scale,
            y: origin_y * scale,
            w: cell_w * scale,
            h: cell_h * scale,
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

    let png = if scale == 1 {
        encode_png(&atlas, width, height)?
    } else {
        let mut scaled = Vec::new();
        upscale_nearest(&atlas, width, height, scale, &mut scaled);
        encode_png(&scaled, out_w, out_h)?
    };
    Ok(AtlasOutput {
        png,
        manifest: AtlasManifest {
            width: out_w,
            height: out_h,
            columns,
            rows,
            frame_width: cell_w * scale,
            frame_height: cell_h * scale,
            frames,
            scale,
        },
    })
}

/// The frames to export, in playback order.
pub(super) fn select_frames(
    sprite: &Sprite,
    tag: Option<&str>,
) -> Result<Vec<FrameIndex>, ExportError> {
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

/// Encode row-major, non-premultiplied RGBA8 `pixels` (`width × height × 4`
/// bytes) as a PNG — what Copy puts on the clipboard (spec §5.4).
pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, ExportError> {
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
pub(super) fn escape_json(value: &str) -> String {
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

/// Errors raised by the PNG import path.
#[derive(Debug, Error)]
pub enum ImportError {
    /// The bytes are not a PNG the decoder can read.
    #[error("png decode failed: {0}")]
    Decode(String),
    /// The decoder produced a colour layout this importer does not handle.
    #[error("unsupported png colour layout {0:?}")]
    UnsupportedLayout(String),
}

/// A decoded raster image, straight-alpha RGBA8, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decode a PNG byte stream into RGBA8 (spec §7.3 counterpart for import;
/// Fineliner parity). Every PNG colour type and bit depth is normalised —
/// palette, grayscale, 16-bit, and tRNS transparency all expand to 8-bit
/// straight-alpha RGBA. Animated PNGs yield their first frame.
///
/// # Errors
///
/// [`ImportError::Decode`] for malformed or truncated input;
/// [`ImportError::UnsupportedLayout`] if the decoder hands back a layout
/// other than 8-bit gray / gray+alpha / RGB / RGBA (not expected after
/// normalisation, kept as a guard rather than a panic).
pub fn import_png(bytes: &[u8]) -> Result<ImportedImage, ImportError> {
    let mut decoder = ::png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(::png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|e| ImportError::Decode(e.to_string()))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| ImportError::Decode("image too large to buffer".to_owned()))?;
    let mut buf = vec![0u8; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| ImportError::Decode(e.to_string()))?;
    buf.truncate(info.buffer_size());
    if info.bit_depth != ::png::BitDepth::Eight {
        return Err(ImportError::UnsupportedLayout(format!(
            "{:?} {:?}",
            info.color_type, info.bit_depth
        )));
    }
    let pixel_count = info.width as usize * info.height as usize;
    let rgba = match info.color_type {
        ::png::ColorType::Rgba => buf,
        ::png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for px in buf.as_chunks::<3>().0 {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        ::png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for &g in &buf {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        ::png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for px in buf.as_chunks::<2>().0 {
                out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
            out
        }
        other => return Err(ImportError::UnsupportedLayout(format!("{other:?}"))),
    };
    if rgba.len() != pixel_count * 4 {
        return Err(ImportError::Decode(format!(
            "decoded {} bytes for {}x{} pixels",
            rgba.len(),
            info.width,
            info.height
        )));
    }
    Ok(ImportedImage {
        width: info.width,
        height: info.height,
        rgba,
    })
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
        for px in buf.data.as_chunks_mut::<4>().0 {
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
        let mut pixels = Vec::new();
        compose(
            sprite,
            cels,
            &ComposeRequest::full(FrameIndex::new(frame), sprite.width, sprite.height),
            &mut pixels,
        )
        .expect("compose succeeds");
        pixels
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
    fn export_atlas_png_fails_whole_when_one_frame_cannot_be_composed() {
        // Frames 0 and 1 compose; only the last frame is broken, and the
        // sheet still fails with the composer's error for that frame.
        let mut corrupt = solid(2, 2, [1, 2, 3, 255]);
        corrupt.data.truncate(4);
        let last = |data: CelData| Cel {
            data,
            ..Cel::image(LayerId::new(0), FrameIndex::new(2), solid(2, 2, [0; 4]))
        };
        let cases = [
            (
                last(CelData::Linked(FrameIndex::new(0))),
                RenderError::LinkedCelUnsupported {
                    layer: LayerId::new(0),
                    frame: FrameIndex::new(2),
                },
            ),
            (
                last(CelData::Image(corrupt)),
                RenderError::MalformedCelBuffer {
                    layer: LayerId::new(0),
                    frame: FrameIndex::new(2),
                },
            ),
        ];
        for (broken, expected) in cases {
            let (sprite, mut cels) = striped_sprite(2, 2, 3, Vec::new());
            cels.insert(broken);
            export_frame_png(&sprite, &cels, FrameIndex::new(1)).expect("frame 1 composes");
            match export_atlas_png(&sprite, &cels, &AtlasOptions::new(3)) {
                Err(ExportError::Render(err)) => assert_eq!(err, expected),
                Err(err) => panic!("unexpected error: {err}"),
                Ok(_) => panic!("a sheet with a broken frame produced an image"),
            }
        }

        // A non-RGBA sprite fails the sheet the same way.
        let sprite = Sprite::builder(2, 2)
            .color_mode(ColorMode::Indexed {
                transparent_index: 0,
            })
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        match export_atlas_png(&sprite, &CelMap::new(), &AtlasOptions::new(2)) {
            Err(ExportError::Render(RenderError::UnsupportedColorMode { .. })) => {}
            Err(err) => panic!("unexpected error: {err}"),
            Ok(_) => panic!("an indexed sheet produced an image"),
        }
    }

    #[test]
    fn export_exports_non_normal_blend_mode_like_compose() {
        // `compose()` renders every separable blend mode (spec §4), so export
        // inherits that rather than refusing the sprite.
        let mut layer = Layer::image(LayerId::new(0), "art");
        layer.blend_mode = BlendMode::Multiply;
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

        let png = export_frame_png(&sprite, &cels, FrameIndex::new(0)).expect("frame exports");
        let (w, h, pixels) = decode(&png);
        assert_eq!((w, h), (2, 2));
        assert_eq!(pixels, composed_pixels(&sprite, &cels, 0));

        export_atlas_png(&sprite, &cels, &AtlasOptions::new(1)).expect("atlas exports");
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
    fn export_composes_group_layers_like_compose() {
        // `compose()` skips group layers and renders their children (spec §4),
        // so a group no longer fails the export.
        let group = Layer::group(LayerId::new(0), "folder");
        let mut child = Layer::image(LayerId::new(1), "art");
        child.parent = Some(LayerId::new(0));
        let sprite = Sprite::builder(2, 2)
            .add_layer(group)
            .add_layer(child)
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(2, 2, [10, 20, 30, 255]),
        ));

        let png = export_frame_png(&sprite, &cels, FrameIndex::new(0)).expect("frame exports");
        let (w, h, pixels) = decode(&png);
        assert_eq!((w, h), (2, 2));
        assert_eq!(pixels, composed_pixels(&sprite, &cels, 0));
        assert_eq!(&pixels[..4], &[10, 20, 30, 255]);
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
            scale: 1,
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

    fn encode_with(
        color: ::png::ColorType,
        depth: ::png::BitDepth,
        w: u32,
        h: u32,
        data: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = ::png::Encoder::new(&mut out, w, h);
            enc.set_color(color);
            enc.set_depth(depth);
            let mut writer = enc.write_header().expect("header");
            writer.write_image_data(data).expect("data");
        }
        out
    }

    #[test]
    fn import_png_round_trips_an_exported_frame() {
        let (sprite, cels) = striped_sprite(2, 2, 1, Vec::new());
        let bytes = export_frame_png(&sprite, &cels, FrameIndex::new(0)).expect("export");
        let img = import_png(&bytes).expect("import");
        assert_eq!((img.width, img.height), (2, 2));
        assert_eq!(img.rgba, composed_pixels(&sprite, &cels, 0));
    }

    #[test]
    fn import_png_normalises_rgb_gray_and_palette_to_rgba() {
        let rgb = encode_with(
            ::png::ColorType::Rgb,
            ::png::BitDepth::Eight,
            2,
            1,
            &[1, 2, 3, 4, 5, 6],
        );
        assert_eq!(
            import_png(&rgb).unwrap().rgba,
            vec![1, 2, 3, 255, 4, 5, 6, 255]
        );
        let gray = encode_with(
            ::png::ColorType::Grayscale,
            ::png::BitDepth::Eight,
            1,
            1,
            &[9],
        );
        assert_eq!(import_png(&gray).unwrap().rgba, vec![9, 9, 9, 255]);
        let ga = encode_with(
            ::png::ColorType::GrayscaleAlpha,
            ::png::BitDepth::Eight,
            1,
            1,
            &[7, 8],
        );
        assert_eq!(import_png(&ga).unwrap().rgba, vec![7, 7, 7, 8]);
        let mut palette = Vec::new();
        {
            let mut enc = ::png::Encoder::new(&mut palette, 1, 1);
            enc.set_color(::png::ColorType::Indexed);
            enc.set_depth(::png::BitDepth::Eight);
            enc.set_palette(vec![10, 20, 30]);
            let mut writer = enc.write_header().expect("header");
            writer.write_image_data(&[0]).expect("data");
        }
        assert_eq!(import_png(&palette).unwrap().rgba, vec![10, 20, 30, 255]);
        let sixteen = encode_with(
            ::png::ColorType::Rgba,
            ::png::BitDepth::Sixteen,
            1,
            1,
            &[255, 255, 0, 0, 0, 0, 128, 0],
        );
        let px = import_png(&sixteen).unwrap().rgba;
        assert_eq!(px[0], 255);
        assert_eq!(px[3], 128);
    }

    #[test]
    fn import_png_rejects_garbage() {
        assert!(matches!(
            import_png(&[1, 2, 3]),
            Err(ImportError::Decode(_))
        ));
    }

    // Scaled export (docs/specs/scaled-export.md).

    /// A 2×2 sprite with four distinct pixels, one of them half transparent.
    fn four_colour_sprite() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(LayerId::new(0), "art"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut buf = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        buf.data.copy_from_slice(&[
            255, 0, 0, 255, 0, 255, 0, 255, //
            0, 0, 255, 128, 9, 8, 7, 255,
        ]);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));
        (sprite, cels)
    }

    /// `src` (`w × h`) upscaled by `n`, written independently of the
    /// production helper.
    fn blocks(src: &[u8], w: u32, h: u32, n: u32) -> Vec<u8> {
        let mut out = Vec::new();
        for y in 0..h * n {
            for x in 0..w * n {
                let i = (((y / n) * w + x / n) * 4) as usize;
                out.extend_from_slice(&src[i..i + 4]);
            }
        }
        out
    }

    #[test]
    fn export_frame_png_scaled_4x4_at_4_is_16x16() {
        let (sprite, cels) = striped_sprite(4, 4, 1, Vec::new());
        let png = export_frame_png_scaled(&sprite, &cels, FrameIndex::new(0), 4).expect("export");
        let (w, h, _) = decode(&png);
        assert_eq!((w, h), (16, 16));
        // IHDR width / height, bytes 16–23.
        assert_eq!(&png[16..24], &[0, 0, 0, 16, 0, 0, 0, 16]);
    }

    #[test]
    fn export_frame_png_scaled_turns_each_pixel_into_an_exact_block() {
        let (sprite, cels) = four_colour_sprite();
        let source = composed_pixels(&sprite, &cels, 0);
        let png = export_frame_png_scaled(&sprite, &cels, FrameIndex::new(0), 3).expect("export");
        let (w, h, rgba) = decode(&png);
        assert_eq!((w, h), (6, 6));
        assert_eq!(rgba, blocks(&source, 2, 2, 3));
        // No colour that is not in the unscaled export, alpha kept.
        let colours: std::collections::BTreeSet<&[u8]> = source.chunks(4).collect();
        assert!(rgba.chunks(4).all(|px| colours.contains(px)));
        assert!(rgba.chunks(4).any(|px| px == [0, 0, 255, 128]));
    }

    #[test]
    fn export_frame_png_scale_one_matches_the_unscaled_export() {
        let (sprite, cels) = four_colour_sprite();
        let plain = export_frame_png(&sprite, &cels, FrameIndex::new(0)).expect("export");
        let one = export_frame_png_scaled(&sprite, &cels, FrameIndex::new(0), 1).expect("export");
        assert_eq!(plain, one);
        // The pre-feature path: compose at 1× and encode.
        let reference = encode_png(&composed_pixels(&sprite, &cels, 0), 2, 2).expect("encode");
        assert_eq!(plain, reference);
    }

    #[test]
    fn export_frame_png_scaled_out_of_range_scale_returns_err_naming_range() {
        let (sprite, cels) = four_colour_sprite();
        for scale in [0, 11] {
            let err = export_frame_png_scaled(&sprite, &cels, FrameIndex::new(0), scale)
                .expect_err("refused");
            assert!(matches!(err, ExportError::InvalidScale { .. }));
            assert!(err.to_string().contains("1–10"), "{err}");
        }
    }

    #[test]
    fn export_frame_png_scaled_above_16384_returns_err_naming_limit() {
        let (sprite, cels) = striped_sprite(1639, 1, 1, Vec::new());
        let err = export_frame_png_scaled(&sprite, &cels, FrameIndex::new(0), 10)
            .expect_err("16390 px refused");
        assert!(matches!(err, ExportError::ScaledTooLarge { .. }));
        assert!(err.to_string().contains("16384"), "{err}");
        // Exactly 16384 px is exported.
        let (sprite, cels) = striped_sprite(4096, 1, 1, Vec::new());
        let png = export_frame_png_scaled(&sprite, &cels, FrameIndex::new(0), 4).expect("at limit");
        assert_eq!(decode(&png).0, 16384);
    }

    #[test]
    fn export_atlas_png_scaled_is_n_times_the_unscaled_sheet() {
        let (sprite, cels) = striped_sprite(3, 2, 3, Vec::new());
        let plain = export_atlas_png(&sprite, &cels, &AtlasOptions::new(2)).expect("atlas");
        let scaled = export_atlas_png(&sprite, &cels, &AtlasOptions::new(2).with_scale(3))
            .expect("scaled atlas");
        let (pw, ph, prgba) = decode(&plain.png);
        let (sw, sh, srgba) = decode(&scaled.png);
        assert_eq!((pw, ph), (6, 4));
        assert_eq!((sw, sh), (18, 12));
        assert_eq!(srgba, blocks(&prgba, pw, ph, 3));
        let m = &scaled.manifest;
        assert_eq!(
            (m.width, m.height, m.frame_width, m.frame_height),
            (18, 12, 9, 6)
        );
        assert_eq!((m.columns, m.rows, m.scale), (2, 2, 3));
        let f = &m.frames[1];
        assert_eq!((f.x, f.y, f.w, f.h), (9, 0, 9, 6));
        let f = &m.frames[2];
        assert_eq!((f.x, f.y, f.w, f.h), (0, 6, 9, 6));
    }

    #[test]
    fn export_atlas_png_scale_one_matches_the_unscaled_sheet() {
        let (sprite, cels) = striped_sprite(3, 2, 3, Vec::new());
        let plain = export_atlas_png(&sprite, &cels, &AtlasOptions::new(2)).expect("atlas");
        let one =
            export_atlas_png(&sprite, &cels, &AtlasOptions::new(2).with_scale(1)).expect("atlas");
        assert_eq!(plain, one);
        assert_eq!(plain.manifest.scale, 1);
    }

    #[test]
    fn export_atlas_png_scaled_past_the_limit_returns_err() {
        // 3000×1 at ×1 fits; ×6 is 18000 px wide.
        let (sprite, cels) = striped_sprite(1000, 1, 3, Vec::new());
        assert!(export_atlas_png(&sprite, &cels, &AtlasOptions::new(3)).is_ok());
        let err = export_atlas_png(&sprite, &cels, &AtlasOptions::new(3).with_scale(6))
            .expect_err("refused");
        assert!(matches!(err, ExportError::ScaledTooLarge { .. }));
        assert!(err.to_string().contains("16384"), "{err}");
        let err = export_atlas_png(&sprite, &cels, &AtlasOptions::new(3).with_scale(11))
            .expect_err("refused");
        assert!(err.to_string().contains("1–10"), "{err}");
    }
}
