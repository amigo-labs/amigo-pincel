//! Photoshop (`.psd`) import — a minimal reader of our own
//! (docs/specs/psd-import.md, `docs/specs/pincel.md` §15 2026-10-29).
//!
//! Format reference: Adobe's "Photoshop File Formats Specification"
//! (<https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/>).
//!
//! Only 8-bit RGB version-1 files are read; PSB, other depths and other
//! color modes are rejected. Raster layers become canvas-sized layers placed
//! at their bounds, `lsct` section dividers become groups. Channels are raw
//! or PackBits; a layer section that is missing, empty, ZIP-compressed,
//! unreadable or holds only unrepresentable layers falls back to the
//! flattened composite at the end of the file. Everything the import leaves
//! out or changes is reported as a [`PsdNotice`]. Pincel never writes PSD.

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::color::BlendMode;
use crate::document::{Document, ImageBuffer, Layer, MAX_CANVAS_DIM, MAX_LAYERS};
use crate::error::DocumentError;

/// Errors from reading a PSD file. A file that fails with one of these is
/// not opened at all.
#[derive(Debug, Error)]
pub enum PsdError {
    /// The file ends inside its header or before a section it announces.
    #[error("the PSD file is truncated")]
    Truncated,
    /// The header is not a valid PSD header.
    #[error("invalid PSD header: {0}")]
    InvalidHeader(String),
    /// The file is a PSB (Photoshop large document).
    #[error("PSB (large document format) is not supported")]
    Psb,
    /// The channels have a depth other than 8 bits.
    #[error("{0} bits per channel is not supported (only 8-bit)")]
    Depth(u16),
    /// The color mode is not RGB (the header's mode number).
    #[error("color mode {} is not supported (only RGB)", mode_name(*.0))]
    ColorMode(u16),
    /// The canvas exceeds the caller's [`PsdLimits`].
    #[error("canvas {width} × {height} exceeds the size limit")]
    CanvasTooLarge {
        /// Canvas width in pixels.
        width: u32,
        /// Canvas height in pixels.
        height: u32,
    },
    /// The flattened composite the import had to fall back to is unreadable.
    #[error("the flattened image cannot be read: {0}")]
    Composite(String),
    /// The resulting document violates a document invariant (e.g. more
    /// than 999 layers and groups).
    #[error(transparent)]
    Document(#[from] DocumentError),
}

fn mode_name(mode: u16) -> &'static str {
    match mode {
        0 => "Bitmap",
        1 => "Grayscale",
        2 => "Indexed",
        3 => "RGB",
        4 => "CMYK",
        7 => "Multichannel",
        8 => "Duotone",
        9 => "Lab",
        _ => "unknown",
    }
}

/// Canvas limits an import must respect; checked before any pixel is
/// decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsdLimits {
    /// Largest canvas edge in pixels.
    pub max_dim: u32,
    /// Largest canvas area (width × height) in pixels.
    pub max_area: u64,
}

impl Default for PsdLimits {
    /// The core's own canvas limit, without an area limit.
    fn default() -> Self {
        Self {
            max_dim: MAX_CANVAS_DIM,
            max_area: u64::MAX,
        }
    }
}

/// Why an import opened the flattened composite instead of the layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PsdFallback {
    /// The layer section is missing or holds no layers.
    NoLayers,
    /// Layer channels are ZIP-compressed (or use an unknown compression).
    Compressed,
    /// The layer section cannot be parsed.
    Unreadable,
    /// Every layer is one Pincel cannot represent (e.g. adjustment layers).
    Unrepresentable,
}

/// One thing the import left out or changed (docs/specs/psd-import.md
/// AC-04, AC-06, AC-07, AC-14). Serialized with a `kind` tag for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PsdNotice {
    /// The file opened as one layer holding the flattened composite.
    Fallback {
        /// Why the layers were not used.
        reason: PsdFallback,
    },
    /// An adjustment or fill layer was left out.
    Adjustment {
        /// The layer's name.
        layer: String,
    },
    /// A text layer opened as its rasterized pixels.
    Text {
        /// The layer's name.
        layer: String,
    },
    /// A smart object opened as its rasterized pixels.
    SmartObject {
        /// The layer's name.
        layer: String,
    },
    /// A layer's effects (layer styles) were left out.
    Effects {
        /// The layer's name.
        layer: String,
    },
    /// A layer's mask (pixel or vector) was left out.
    Mask {
        /// The layer's name.
        layer: String,
    },
    /// A clipping mask was left out; the layer shows unclipped.
    Clipping {
        /// The layer's name.
        layer: String,
    },
    /// A blend mode Pincel lacks opened as Normal.
    Blend {
        /// The layer's or group's name.
        layer: String,
        /// The PSD blend key without trailing spaces (e.g. `hue`).
        key: String,
    },
    /// A pass-through group with a blending descendant opened as an
    /// isolated Normal group.
    PassThrough {
        /// The group's name.
        group: String,
    },
}

/// An imported PSD: the document and what the import changed.
#[derive(Debug)]
pub struct PsdImport {
    /// The opened document.
    pub document: Document,
    /// Everything left out or changed, in layer order (bottom first).
    pub notices: Vec<PsdNotice>,
}

/// A bounds-checked big-endian reader over `bytes[pos..end]`.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
    end: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], pos: usize, end: usize) -> Self {
        Self {
            bytes,
            pos,
            end: end.min(bytes.len()),
        }
    }

    fn remaining(&self) -> usize {
        self.end.saturating_sub(self.pos)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if n > self.remaining() {
            return None;
        }
        let out = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Some(out)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        self.array().map(u16::from_be_bytes)
    }

    fn i16(&mut self) -> Option<i16> {
        self.array().map(i16::from_be_bytes)
    }

    fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_be_bytes)
    }

    fn i32(&mut self) -> Option<i32> {
        self.array().map(i32::from_be_bytes)
    }
}

/// Unpacks one PackBits row into `out`; `None` when `src` runs short.
fn unpack_bits(src: &[u8], out: &mut [u8]) -> Option<()> {
    let (mut i, mut o) = (0, 0);
    while o < out.len() {
        let n = *src.get(i)? as i8;
        i += 1;
        if n >= 0 {
            let len = n as usize + 1;
            let run = src.get(i..i + len)?;
            let fit = len.min(out.len() - o);
            out[o..o + fit].copy_from_slice(&run[..fit]);
            o += fit;
            i += len;
        } else if n != -128 {
            let len = (1 - i32::from(n)) as usize;
            let v = *src.get(i)?;
            i += 1;
            let fit = len.min(out.len() - o);
            out[o..o + fit].fill(v);
            o += fit;
        }
    }
    Some(())
}

/// The header fields the import uses.
struct Header {
    width: u32,
    height: u32,
    channels: u16,
}

fn read_header(bytes: &[u8]) -> Result<Header, PsdError> {
    let mut c = Cursor::new(bytes, 0, bytes.len());
    let (sig, version, _, channels, height, width, depth, mode) = (|| {
        Some((
            c.array::<4>()?,
            c.u16()?,
            c.array::<6>()?,
            c.u16()?,
            c.u32()?,
            c.u32()?,
            c.u16()?,
            c.u16()?,
        ))
    })()
    .ok_or(PsdError::Truncated)?;
    if &sig != b"8BPS" {
        return Err(PsdError::InvalidHeader("missing the 8BPS signature".into()));
    }
    match version {
        1 => {}
        2 => return Err(PsdError::Psb),
        v => return Err(PsdError::InvalidHeader(format!("version {v}"))),
    }
    if !(1..=56).contains(&channels) {
        return Err(PsdError::InvalidHeader(format!("{channels} channels")));
    }
    if !(1..=30_000).contains(&width) || !(1..=30_000).contains(&height) {
        return Err(PsdError::InvalidHeader(format!("size {width} × {height}")));
    }
    match depth {
        8 => {}
        1 | 16 | 32 => return Err(PsdError::Depth(depth)),
        d => return Err(PsdError::InvalidHeader(format!("depth {d}"))),
    }
    match mode {
        3 => {}
        0 | 1 | 2 | 4 | 7 | 8 | 9 => return Err(PsdError::ColorMode(mode)),
        m => return Err(PsdError::InvalidHeader(format!("color mode {m}"))),
    }
    if channels < 3 {
        return Err(PsdError::InvalidHeader(format!(
            "{channels} channels for RGB"
        )));
    }
    Ok(Header {
        width,
        height,
        channels,
    })
}

/// Tagged-block keys of adjustment and fill layers.
const ADJUSTMENT_KEYS: [&[u8; 4]; 20] = [
    b"SoCo", b"GdFl", b"PtFl", b"brit", b"levl", b"curv", b"expA", b"vibA", b"hue ", b"hue2",
    b"blnc", b"blwh", b"phfl", b"mixr", b"clrL", b"nvrt", b"post", b"thrs", b"grdm", b"selc",
];

/// One layer record of the layer info section.
struct Record {
    /// top, left, bottom, right
    rect: (i32, i32, i32, i32),
    /// (channel id, data offset, data length) in file order.
    channels: Vec<(i16, usize, usize)>,
    blend: [u8; 4],
    opacity: u8,
    clipping: u8,
    hidden: bool,
    name: String,
    /// `lsct` / `lsdk` divider type and its own blend key.
    section: Option<(u32, Option<[u8; 4]>)>,
    adjustment: bool,
    text: bool,
    smart: bool,
    effects: bool,
    mask: bool,
}

fn read_record(c: &mut Cursor<'_>) -> Option<Record> {
    let rect = (c.i32()?, c.i32()?, c.i32()?, c.i32()?);
    let count = c.u16()?;
    if count > 56 {
        return None;
    }
    let mut channels = Vec::with_capacity(usize::from(count));
    let mut mask = false;
    for _ in 0..count {
        let id = c.i16()?;
        let len = c.u32()? as usize;
        mask |= id == -2 || id == -3;
        channels.push((id, 0, len));
    }
    if &c.array::<4>()? != b"8BIM" {
        return None;
    }
    let blend = c.array::<4>()?;
    let opacity = c.u8()?;
    let clipping = c.u8()?;
    let flags = c.u8()?;
    c.u8()?;
    let extra_len = c.u32()? as usize;
    let extra = c.take(extra_len)?;
    let mut e = Cursor::new(extra, 0, extra.len());
    let mask_len = e.u32()? as usize;
    e.take(mask_len)?;
    mask |= mask_len > 0;
    let ranges_len = e.u32()? as usize;
    e.take(ranges_len)?;
    let n = usize::from(e.u8()?);
    let raw = e.take(n)?;
    let mut name = String::from_utf8(raw.to_vec())
        .unwrap_or_else(|_| raw.iter().map(|&b| char::from(b)).collect());
    let pad = (4 - (n + 1) % 4) % 4;
    e.take(pad.min(e.remaining()))?;

    let mut record = Record {
        rect,
        channels,
        blend,
        opacity,
        clipping,
        hidden: flags & 0x02 != 0,
        name: String::new(),
        section: None,
        adjustment: false,
        text: false,
        smart: false,
        effects: false,
        mask,
    };
    while e.remaining() >= 12 {
        let sig = e.array::<4>()?;
        if &sig != b"8BIM" && &sig != b"8B64" {
            // Some writers pad a block to an even or 4-byte length.
            e.pos -= 3;
            continue;
        }
        let key = e.array::<4>()?;
        let len = e.u32()? as usize;
        let Some(data) = e.take(len) else { break };
        let mut d = Cursor::new(data, 0, data.len());
        match &key {
            b"luni" => {
                if let Some(units) = d
                    .u32()
                    .and_then(|n| (0..n).map(|_| d.u16()).collect::<Option<Vec<u16>>>())
                {
                    name = String::from_utf16_lossy(&units)
                        .trim_end_matches('\0')
                        .to_string();
                }
            }
            b"lsct" | b"lsdk" => {
                if let Some(kind) = d.u32() {
                    let key = d.array::<4>().and_then(|_| d.array::<4>());
                    record.section = Some((kind, key));
                }
            }
            b"TySh" | b"tySh" => record.text = true,
            b"SoLd" | b"SoLE" | b"PlLd" | b"plLd" => record.smart = true,
            b"lrFX" | b"lfx2" | b"lfxs" | b"lmfx" => record.effects = true,
            b"vmsk" | b"vsms" => record.mask = true,
            k if ADJUSTMENT_KEYS.contains(&k) => record.adjustment = true,
            _ => {}
        }
    }
    record.name = name;
    Some(record)
}

/// The parsed layer info: records with their channel data located, and
/// whether the merged composite carries transparency.
struct LayerInfo {
    records: Vec<Record>,
    merged_alpha: bool,
}

/// Parses the layer and mask information section at `c`.
fn read_layer_info(bytes: &[u8], start: usize, len: usize) -> Result<LayerInfo, PsdFallback> {
    if len == 0 {
        return Err(PsdFallback::NoLayers);
    }
    let mut c = Cursor::new(bytes, start, start.saturating_add(len));
    let info_len = c.u32().ok_or(PsdFallback::Unreadable)? as usize;
    if info_len == 0 {
        return Err(PsdFallback::NoLayers);
    }
    let info_end = c.pos.saturating_add(info_len);
    if info_end > c.end {
        return Err(PsdFallback::Unreadable);
    }
    let mut c = Cursor::new(bytes, c.pos, info_end);
    let count = c.i16().ok_or(PsdFallback::Unreadable)?;
    if count == 0 {
        return Err(PsdFallback::NoLayers);
    }
    let mut records = Vec::with_capacity(usize::from(count.unsigned_abs()));
    for _ in 0..count.unsigned_abs() {
        records.push(read_record(&mut c).ok_or(PsdFallback::Unreadable)?);
    }
    let mut pos = c.pos;
    for record in &mut records {
        for (_, offset, len) in &mut record.channels {
            *offset = pos;
            pos = pos.checked_add(*len).ok_or(PsdFallback::Unreadable)?;
        }
    }
    if pos > info_end {
        return Err(PsdFallback::Unreadable);
    }
    Ok(LayerInfo {
        records,
        merged_alpha: count < 0,
    })
}

/// A node of the layer tree, children bottom first.
enum Node {
    Raster(usize),
    Group(usize, Vec<Node>),
}

fn node_count(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Raster(_) => 1,
            Node::Group(_, children) => 1 + node_count(children),
        })
        .sum()
}

/// The Pincel blend mode of a PSD blend key; `None` when Pincel lacks it.
fn blend_mode(key: &[u8; 4]) -> Option<BlendMode> {
    Some(match key {
        b"norm" => BlendMode::Normal,
        b"mul " => BlendMode::Multiply,
        b"scrn" => BlendMode::Screen,
        b"over" => BlendMode::Overlay,
        b"dark" => BlendMode::Darken,
        b"lite" => BlendMode::Lighten,
        b"div " => BlendMode::ColorDodge,
        b"idiv" => BlendMode::ColorBurn,
        b"hLit" => BlendMode::HardLight,
        b"sLit" => BlendMode::SoftLight,
        b"diff" => BlendMode::Difference,
        b"smud" => BlendMode::Exclusion,
        _ => return None,
    })
}

fn key_text(key: &[u8; 4]) -> String {
    String::from_utf8_lossy(key).trim_end().to_string()
}

/// Blend mode for `key`, pushing a notice when it falls back to Normal.
fn mapped_blend(key: &[u8; 4], name: &str, notices: &mut Vec<PsdNotice>) -> BlendMode {
    blend_mode(key).unwrap_or_else(|| {
        notices.push(PsdNotice::Blend {
            layer: name.to_string(),
            key: key_text(key),
        });
        BlendMode::Normal
    })
}

/// Builds the layer tree from the bottom-first records: an end marker
/// (`lsct` type 3) opens a group's scope, the folder record (type 1 / 2)
/// above its children closes it. Adjustment layers are left out.
fn build_tree(records: &[Record], notices: &mut Vec<PsdNotice>) -> Vec<Node> {
    // Each frame: its nodes and whether a descendant blends non-Normal.
    let mut stack: Vec<(Vec<Node>, bool)> = vec![(Vec::new(), false)];
    for (i, r) in records.iter().enumerate() {
        match r.section.map(|(kind, _)| kind) {
            Some(3) => stack.push((Vec::new(), false)),
            Some(1 | 2) => {
                let (children, blending) = if stack.len() > 1 {
                    stack.pop().unwrap_or_default()
                } else {
                    (Vec::new(), false)
                };
                let key = r.section.and_then(|(_, key)| key).unwrap_or(r.blend);
                if &key == b"pass" {
                    if blending {
                        notices.push(PsdNotice::PassThrough {
                            group: r.name.clone(),
                        });
                    }
                } else {
                    mapped_blend(&key, &r.name, notices);
                }
                if let Some(frame) = stack.last_mut() {
                    frame.1 |= blending || !matches!(&key, b"pass" | b"norm");
                    frame.0.push(Node::Group(i, children));
                }
            }
            _ => {
                let layer = || r.name.clone();
                if r.adjustment {
                    notices.push(PsdNotice::Adjustment { layer: layer() });
                    continue;
                }
                if r.text {
                    notices.push(PsdNotice::Text { layer: layer() });
                }
                if r.smart {
                    notices.push(PsdNotice::SmartObject { layer: layer() });
                }
                if r.effects {
                    notices.push(PsdNotice::Effects { layer: layer() });
                }
                if r.mask {
                    notices.push(PsdNotice::Mask { layer: layer() });
                }
                if r.clipping != 0 {
                    notices.push(PsdNotice::Clipping { layer: layer() });
                }
                mapped_blend(&r.blend, &r.name, notices);
                if let Some(frame) = stack.last_mut() {
                    frame.1 |= &r.blend != b"norm";
                    frame.0.push(Node::Raster(i));
                }
            }
        }
    }
    // Recover groups left open: their children join the level below.
    while stack.len() > 1 {
        let (children, blending) = stack.pop().unwrap_or_default();
        if let Some(frame) = stack.last_mut() {
            frame.0.extend(children);
            frame.1 |= blending;
        }
    }
    stack.pop().map(|(nodes, _)| nodes).unwrap_or_default()
}

/// Decodes a record's color and alpha channels into a canvas-sized buffer,
/// placed at the record's bounds and clipped to the canvas.
fn decode_layer(bytes: &[u8], r: &Record, w: u32, h: u32) -> Result<ImageBuffer, PsdFallback> {
    let mut out = ImageBuffer::new_transparent(w, h);
    let (top, left, bottom, right) = r.rect;
    let (lw, lh) = (
        i64::from(right) - i64::from(left),
        i64::from(bottom) - i64::from(top),
    );
    if lw < 0 || lh < 0 {
        return Err(PsdFallback::Unreadable);
    }
    if lw == 0 || lh == 0 {
        return Ok(out);
    }
    // Raw data holds a byte per pixel and PackBits at least one per 128,
    // so bounds larger than that are corrupt (and would allocate wildly).
    let budget: u64 = r.channels.iter().map(|&(_, _, len)| len as u64).sum();
    if (lw as u64).saturating_mul(lh as u64) > budget.saturating_mul(128) {
        return Err(PsdFallback::Unreadable);
    }
    let (lw, lh) = (lw as usize, lh as usize);
    let stride = w as usize * 4;
    // Canvas columns the layer covers, as layer-relative and canvas ranges.
    let col0 = (-i64::from(left)).clamp(0, lw as i64) as usize;
    let col1 = (i64::from(w) - i64::from(left)).clamp(0, lw as i64) as usize;
    let data = out.data_mut();
    let mut place = |plane_row: &[u8], row: usize, channel: usize| {
        let y = i64::from(top) + row as i64;
        if y < 0 || y >= i64::from(h) || col0 >= col1 {
            return;
        }
        let x0 = (i64::from(left) + col0 as i64) as usize;
        let base = y as usize * stride + x0 * 4 + channel;
        for (k, &v) in plane_row[col0..col1].iter().enumerate() {
            data[base + k * 4] = v;
        }
    };
    let has_alpha = r.channels.iter().any(|&(id, _, _)| id == -1);
    if !has_alpha {
        let opaque = vec![255u8; lw];
        for row in 0..lh {
            place(&opaque, row, 3);
        }
    }
    let mut row_buf = vec![0u8; lw];
    for &(id, offset, len) in &r.channels {
        let channel = match id {
            0..=2 => id as usize,
            -1 => 3,
            _ => continue,
        };
        let mut c = Cursor::new(bytes, offset, offset + len);
        match c.u16().ok_or(PsdFallback::Unreadable)? {
            0 => {
                if lw.checked_mul(lh).is_none_or(|n| n > c.remaining()) {
                    return Err(PsdFallback::Unreadable);
                }
                for row in 0..lh {
                    place(c.take(lw).ok_or(PsdFallback::Unreadable)?, row, channel);
                }
            }
            1 => {
                let counts = c
                    .take(lh.checked_mul(2).ok_or(PsdFallback::Unreadable)?)
                    .ok_or(PsdFallback::Unreadable)?;
                for (row, n) in counts.as_chunks::<2>().0.iter().enumerate() {
                    let src = c
                        .take(usize::from(u16::from_be_bytes(*n)))
                        .ok_or(PsdFallback::Unreadable)?;
                    unpack_bits(src, &mut row_buf).ok_or(PsdFallback::Unreadable)?;
                    place(&row_buf, row, channel);
                }
            }
            2 | 3 => return Err(PsdFallback::Compressed),
            _ => return Err(PsdFallback::Unreadable),
        }
    }
    Ok(out)
}

/// Appends `nodes` (bottom first) in storage order: each group before its
/// children.
fn emit(
    nodes: Vec<Node>,
    parent: Option<Uuid>,
    records: &[Record],
    pixels: &mut [Option<ImageBuffer>],
    out: &mut Vec<Layer>,
) {
    for node in nodes {
        let (i, children) = match node {
            Node::Raster(i) => (i, None),
            Node::Group(i, children) => (i, Some(children)),
        };
        let r = &records[i];
        let mut layer = match &children {
            Some(_) => Layer::new_group(r.name.clone()),
            None => Layer::from_pixels(
                r.name.clone(),
                pixels[i]
                    .take()
                    .unwrap_or_else(|| ImageBuffer::new_transparent(0, 0)),
            ),
        };
        let key = match &children {
            Some(_) => r.section.and_then(|(_, key)| key).unwrap_or(r.blend),
            None => r.blend,
        };
        layer.blend_mode = blend_mode(&key).unwrap_or(BlendMode::Normal);
        layer.opacity = f32::from(r.opacity) / 255.0;
        layer.visible = !r.hidden;
        layer.parent = parent;
        let id = layer.id;
        out.push(layer);
        if let Some(children) = children {
            emit(children, Some(id), records, pixels, out);
        }
    }
}

/// Reads the flattened composite (the image data section at `start`).
fn read_composite(
    bytes: &[u8],
    start: usize,
    header: &Header,
    merged_alpha: bool,
) -> Result<ImageBuffer, PsdError> {
    let (w, h) = (header.width as usize, header.height as usize);
    let mut c = Cursor::new(bytes, start, bytes.len());
    let short = || PsdError::Composite("the file ends early".into());
    let compression = c.u16().ok_or_else(short)?;
    let channels = usize::from(header.channels);
    let used = if merged_alpha { channels.min(4) } else { 3 };
    let mut out = ImageBuffer::new_transparent(header.width, header.height);
    let data = out.data_mut();
    if used < 4 {
        for px in data.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
    }
    let mut put = |row: &[u8], y: usize, channel: usize| {
        for (x, &v) in row.iter().enumerate() {
            data[(y * w + x) * 4 + channel] = v;
        }
    };
    match compression {
        0 => {
            for channel in 0..used {
                for y in 0..h {
                    put(c.take(w).ok_or_else(short)?, y, channel);
                }
            }
        }
        1 => {
            let counts = c.take(channels * h * 2).ok_or_else(short)?;
            let counts = counts.as_chunks::<2>().0;
            let mut row = vec![0u8; w];
            for channel in 0..used {
                for y in 0..h {
                    let n = usize::from(u16::from_be_bytes(counts[channel * h + y]));
                    let src = c.take(n).ok_or_else(short)?;
                    unpack_bits(src, &mut row).ok_or_else(short)?;
                    put(&row, y, channel);
                }
            }
        }
        2 | 3 => return Err(PsdError::Composite("it is ZIP-compressed".into())),
        n => return Err(PsdError::Composite(format!("unknown compression {n}"))),
    }
    Ok(out)
}

/// Opens an 8-bit RGB PSD as a document with its layers and groups, or —
/// when the layer section is unusable — its flattened composite as one
/// layer. `limits` is checked before any pixel is decoded.
pub fn from_psd_bytes(bytes: &[u8], limits: PsdLimits) -> Result<PsdImport, PsdError> {
    let header = read_header(bytes)?;
    let (w, h) = (header.width, header.height);
    if w > limits.max_dim || h > limits.max_dim || u64::from(w) * u64::from(h) > limits.max_area {
        return Err(PsdError::CanvasTooLarge {
            width: w,
            height: h,
        });
    }
    let mut c = Cursor::new(bytes, 26, bytes.len());
    for _ in 0..2 {
        // Color mode data, image resources.
        let len = c.u32().ok_or(PsdError::Truncated)? as usize;
        c.take(len).ok_or(PsdError::Truncated)?;
    }
    let layers_len = c.u32().ok_or(PsdError::Truncated)? as usize;
    let layers_start = c.pos;
    let image_start = layers_start.saturating_add(layers_len);

    let mut notices = Vec::new();
    let mut merged_alpha = false;
    let fallback = match read_layer_info(bytes, layers_start, layers_len) {
        Err(reason) => reason,
        Ok(info) => {
            merged_alpha = info.merged_alpha;
            let tree = build_tree(&info.records, &mut notices);
            if node_count(&tree) > MAX_LAYERS {
                return Err(DocumentError::LayerLimit { max: MAX_LAYERS }.into());
            }
            match layers_document(bytes, &info.records, tree, w, h) {
                Ok(Some(document)) => return Ok(PsdImport { document, notices }),
                Ok(None) => {
                    if notices.is_empty() {
                        PsdFallback::NoLayers
                    } else {
                        PsdFallback::Unrepresentable
                    }
                }
                Err(reason) => {
                    notices.clear();
                    reason
                }
            }
        }
    };
    notices.push(PsdNotice::Fallback { reason: fallback });
    let pixels = read_composite(bytes, image_start, &header, merged_alpha)?;
    Ok(PsdImport {
        document: Document::from_pixels(pixels)?,
        notices,
    })
}

/// The layered document; `Ok(None)` when no raster layer is left.
fn layers_document(
    bytes: &[u8],
    records: &[Record],
    tree: Vec<Node>,
    w: u32,
    h: u32,
) -> Result<Option<Document>, PsdFallback> {
    fn rasters(nodes: &[Node], out: &mut Vec<usize>) {
        for node in nodes {
            match node {
                Node::Raster(i) => out.push(*i),
                Node::Group(_, children) => rasters(children, out),
            }
        }
    }
    let mut kept = Vec::new();
    rasters(&tree, &mut kept);
    if kept.is_empty() {
        return Ok(None);
    }
    let mut pixels: Vec<Option<ImageBuffer>> = (0..records.len()).map(|_| None).collect();
    for i in kept {
        pixels[i] = Some(decode_layer(bytes, &records[i], w, h)?);
    }
    let mut layers = Vec::new();
    emit(tree, None, records, &mut pixels, &mut layers);
    let mut doc = Document::new(w, h).map_err(|_| PsdFallback::Unreadable)?;
    let top = layers.len() - 1;
    doc.layers = layers;
    doc.set_active_layer(top)
        .map_err(|_| PsdFallback::Unreadable)?;
    Ok(Some(doc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpack_bits_expands_literals_and_runs() {
        let mut out = [0u8; 7];
        // 2 literals, a run of 4 × 9, a no-op, 1 literal.
        unpack_bits(&[1, 5, 6, 0xfd, 9, 0x80, 0, 7], &mut out).expect("unpacks");
        assert_eq!(out, [5, 6, 9, 9, 9, 9, 7]);
    }

    #[test]
    fn unpack_bits_short_input_is_none() {
        let mut out = [0u8; 4];
        assert!(unpack_bits(&[0xfe], &mut out).is_none());
        assert!(unpack_bits(&[3, 1, 2], &mut out).is_none());
    }
}
