//! Opening Photoshop (`.psd`) files in Image mode
//! (docs/specs/psd-import.md). The files are built byte by byte by the small
//! writer below, which follows Adobe's "Photoshop File Formats
//! Specification", independent of the reader under test.

use pincel_image_core::codec::psd::{
    PsdError, PsdFallback, PsdImport, PsdLimits, PsdNotice, from_psd_bytes,
};
use pincel_image_core::{BlendMode, Document, MAX_LAYERS, compose};

// ---------------------------------------------------------------- writer

fn be16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn be32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// The 26-byte file header.
fn header(w: u32, h: u32, channels: u16, depth: u16, mode: u16, version: u16) -> Vec<u8> {
    let mut out = b"8BPS".to_vec();
    be16(&mut out, version);
    out.extend_from_slice(&[0; 6]);
    be16(&mut out, channels);
    be32(&mut out, h);
    be32(&mut out, w);
    be16(&mut out, depth);
    be16(&mut out, mode);
    out
}

/// PackBits: repeat packets for runs of two or more, literals otherwise.
fn packbits(row: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < row.len() {
        let mut run = 1;
        while i + run < row.len() && row[i + run] == row[i] && run < 128 {
            run += 1;
        }
        if run >= 2 {
            out.push((1i16 - run as i16) as i8 as u8);
            out.push(row[i]);
            i += run;
        } else {
            let start = i;
            while i < row.len() && i - start < 128 && !(i + 1 < row.len() && row[i + 1] == row[i]) {
                i += 1;
            }
            if i == start {
                i += 1;
            }
            out.push((i - start - 1) as u8);
            out.extend_from_slice(&row[start..i]);
        }
    }
    out
}

/// How a block of channel planes is stored.
#[derive(Clone, Copy, PartialEq)]
enum Compression {
    Raw,
    Rle,
    Zip,
}

/// Planes of `w` × `h` bytes, each with its own compression word (layer
/// channels) — returns one encoded blob per plane.
fn encode_plane(plane: &[u8], w: usize, h: usize, compression: Compression) -> Vec<u8> {
    let mut out = Vec::new();
    match compression {
        Compression::Raw => {
            be16(&mut out, 0);
            out.extend_from_slice(plane);
        }
        Compression::Rle => {
            be16(&mut out, 1);
            let rows: Vec<Vec<u8>> = (0..h)
                .map(|y| packbits(&plane[y * w..(y + 1) * w]))
                .collect();
            for r in &rows {
                be16(&mut out, r.len() as u16);
            }
            for r in rows {
                out.extend_from_slice(&r);
            }
        }
        Compression::Zip => {
            be16(&mut out, 2);
            out.extend_from_slice(&[0x78, 0x9c, 0x03, 0x00]);
        }
    }
    out
}

/// The image-data section: `channels` planes of the `w` × `h` composite,
/// with one compression word for all of them.
fn composite(pixels: &[[u8; 4]], w: usize, h: usize, channels: usize, rle: bool) -> Vec<u8> {
    let planes: Vec<Vec<u8>> = (0..channels)
        .map(|c| pixels.iter().map(|p| p[c.min(3)]).collect())
        .collect();
    let mut out = Vec::new();
    if rle {
        be16(&mut out, 1);
        let rows: Vec<Vec<u8>> = planes
            .iter()
            .flat_map(|p| (0..h).map(move |y| packbits(&p[y * w..(y + 1) * w])))
            .collect();
        for r in &rows {
            be16(&mut out, r.len() as u16);
        }
        for r in rows {
            out.extend_from_slice(&r);
        }
    } else {
        be16(&mut out, 0);
        for p in planes {
            out.extend_from_slice(&p);
        }
    }
    out
}

/// One layer record plus its pixels.
#[derive(Clone)]
struct L {
    name: String,
    /// top, left, bottom, right
    rect: (i32, i32, i32, i32),
    /// Row-major RGBA of the rect.
    rgba: Vec<[u8; 4]>,
    opacity: u8,
    blend: [u8; 4],
    hidden: bool,
    clipping: u8,
    /// `lsct` type and optional blend key.
    section: Option<(u32, Option<[u8; 4]>)>,
    /// Extra tagged blocks (key, data).
    blocks: Vec<([u8; 4], Vec<u8>)>,
    /// Layer mask data (20 bytes) plus a user-mask channel.
    mask: bool,
    alpha: bool,
    compression: Compression,
}

impl L {
    fn new(name: &str, rect: (i32, i32, i32, i32), color: [u8; 4]) -> Self {
        let n = ((rect.2 - rect.0).max(0) * (rect.3 - rect.1).max(0)) as usize;
        Self {
            name: name.into(),
            rect,
            rgba: vec![color; n],
            opacity: 255,
            blend: *b"norm",
            hidden: false,
            clipping: 0,
            section: None,
            blocks: Vec::new(),
            mask: false,
            alpha: true,
            compression: Compression::Raw,
        }
    }

    fn group_open(name: &str) -> Self {
        Self {
            section: Some((1, Some(*b"pass"))),
            blend: *b"pass",
            ..Self::new(name, (0, 0, 0, 0), [0; 4])
        }
    }

    fn group_end() -> Self {
        Self {
            section: Some((3, None)),
            ..Self::new("</Layer group>", (0, 0, 0, 0), [0; 4])
        }
    }

    fn block(mut self, key: &[u8; 4], data: Vec<u8>) -> Self {
        self.blocks.push((*key, data));
        self
    }

    /// (channel id, encoded data) in record order.
    fn channels(&self) -> Vec<(i16, Vec<u8>)> {
        let (t, l, b, r) = self.rect;
        let (w, h) = ((r - l).max(0) as usize, (b - t).max(0) as usize);
        let mut ids: Vec<i16> = if self.alpha {
            vec![-1, 0, 1, 2]
        } else {
            vec![0, 1, 2]
        };
        if self.mask {
            ids.push(-2);
        }
        ids.into_iter()
            .map(|id| {
                let data = if id == -2 {
                    // A 1 × 1 mask (its rect is in the mask data).
                    encode_plane(&[0], 1, 1, Compression::Raw)
                } else {
                    let c = if id == -1 { 3 } else { id as usize };
                    let plane: Vec<u8> = self.rgba.iter().map(|p| p[c]).collect();
                    encode_plane(&plane, w, h, self.compression)
                };
                (id, data)
            })
            .collect()
    }

    fn record(&self, channels: &[(i16, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        let (t, l, b, r) = self.rect;
        for v in [t, l, b, r] {
            out.extend_from_slice(&v.to_be_bytes());
        }
        be16(&mut out, channels.len() as u16);
        for (id, data) in channels {
            out.extend_from_slice(&id.to_be_bytes());
            be32(&mut out, data.len() as u32);
        }
        out.extend_from_slice(b"8BIM");
        out.extend_from_slice(&self.blend);
        out.push(self.opacity);
        out.push(self.clipping);
        out.push(if self.hidden { 0x0a } else { 0x08 });
        out.push(0);
        let mut extra = Vec::new();
        if self.mask {
            be32(&mut extra, 20);
            for v in [0i32, 0, 1, 1] {
                extra.extend_from_slice(&v.to_be_bytes());
            }
            extra.extend_from_slice(&[255, 0, 0, 0]);
        } else {
            be32(&mut extra, 0);
        }
        be32(&mut extra, 0); // blending ranges
        // Pascal name padded to a multiple of 4 (length byte included).
        let name = self.name.as_bytes();
        let n = name.len().min(255);
        extra.push(n as u8);
        extra.extend_from_slice(&name[..n]);
        while !extra.len().is_multiple_of(4) {
            extra.push(0);
        }
        let mut blocks = self.blocks.clone();
        if let Some((kind, key)) = self.section {
            let mut data = Vec::new();
            be32(&mut data, kind);
            if let Some(key) = key {
                data.extend_from_slice(b"8BIM");
                data.extend_from_slice(&key);
            }
            blocks.push((*b"lsct", data));
        }
        for (key, data) in blocks {
            extra.extend_from_slice(b"8BIM");
            extra.extend_from_slice(&key);
            be32(&mut extra, data.len() as u32);
            extra.extend_from_slice(&data);
        }
        be32(&mut out, extra.len() as u32);
        out.extend_from_slice(&extra);
        out
    }
}

/// The layer and mask information section (with its length word).
fn layer_section(layers: &[L], merged_alpha: bool) -> Vec<u8> {
    let mut info = Vec::new();
    let count = layers.len() as i16;
    info.extend_from_slice(&(if merged_alpha { -count } else { count }).to_be_bytes());
    let channels: Vec<_> = layers.iter().map(L::channels).collect();
    for (layer, ch) in layers.iter().zip(&channels) {
        info.extend_from_slice(&layer.record(ch));
    }
    for ch in &channels {
        for (_, data) in ch {
            info.extend_from_slice(data);
        }
    }
    if info.len() % 2 == 1 {
        info.push(0);
    }
    let mut section = Vec::new();
    be32(&mut section, info.len() as u32);
    section.extend_from_slice(&info);
    be32(&mut section, 0); // global layer mask info
    let mut out = Vec::new();
    be32(&mut out, section.len() as u32);
    out.extend_from_slice(&section);
    out
}

/// A complete 8-bit RGB PSD: header, empty color-mode data and resources,
/// `layers` (an empty section when `None`) and the composite.
fn psd(w: u32, h: u32, layers: Option<&[L]>, merged: &[[u8; 4]]) -> Vec<u8> {
    psd_with(w, h, layers, merged, 3, true)
}

fn psd_with(
    w: u32,
    h: u32,
    layers: Option<&[L]>,
    merged: &[[u8; 4]],
    channels: u16,
    rle: bool,
) -> Vec<u8> {
    let mut out = header(w, h, channels, 8, 3, 1);
    be32(&mut out, 0); // color mode data
    be32(&mut out, 0); // image resources
    match layers {
        Some(layers) => out.extend_from_slice(&layer_section(layers, channels > 3)),
        None => be32(&mut out, 0),
    }
    out.extend_from_slice(&composite(
        merged,
        w as usize,
        h as usize,
        channels as usize,
        rle,
    ));
    out
}

fn open(bytes: &[u8]) -> PsdImport {
    from_psd_bytes(bytes, PsdLimits::default()).expect("opens")
}

fn open_err(bytes: &[u8]) -> PsdError {
    match from_psd_bytes(bytes, PsdLimits::default()) {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

fn px(img: &pincel_image_core::ImageBuffer, x: u32, y: u32) -> [u8; 4] {
    let c = img.get_pixel(x, y).expect("in bounds");
    [c.r, c.g, c.b, c.a]
}

/// A 3 × 2 composite with six distinct colors.
fn merged_3x2() -> Vec<[u8; 4]> {
    (0..6u8)
        .map(|i| [i * 40, 255 - i * 40, i * 10, 255])
        .collect()
}

// ---------------------------------------------------------------- header, composite

#[test]
fn psd_without_layers_opens_composite() {
    for rle in [false, true] {
        let merged = merged_3x2();
        let import = open(&psd_with(3, 2, None, &merged, 3, rle));
        let doc = &import.document;
        assert_eq!((doc.canvas.width(), doc.canvas.height()), (3, 2));
        assert_eq!(doc.layer_count(), 1);
        for (i, p) in merged.iter().enumerate() {
            assert_eq!(px(&doc.layers()[0].pixels, i as u32 % 3, i as u32 / 3), *p);
        }
        assert_eq!(
            import.notices,
            vec![PsdNotice::Fallback {
                reason: PsdFallback::NoLayers
            }]
        );
    }
}

#[test]
fn psd_composite_alpha_comes_from_the_fourth_channel_only_with_merged_alpha() {
    let mut merged = merged_3x2();
    merged[0][3] = 0;
    // Layer count negative: the fourth channel is the merged transparency.
    let layered = psd_with(
        3,
        2,
        Some(&[L::new("Adj", (0, 0, 0, 0), [0; 4]).block(b"curv", vec![0; 4])]),
        &merged,
        4,
        true,
    );
    assert_eq!(px(&open(&layered).document.layers()[0].pixels, 0, 0)[3], 0);
    // Without a layer section a fourth channel is a saved selection.
    let flat = psd_with(3, 2, None, &merged, 4, true);
    assert_eq!(px(&open(&flat).document.layers()[0].pixels, 0, 0)[3], 255);
}

#[test]
fn psd_16_bit_is_rejected_with_error() {
    let mut bytes = psd(3, 2, None, &merged_3x2());
    bytes[22..24].copy_from_slice(&16u16.to_be_bytes());
    let err = open_err(&bytes);
    assert!(matches!(err, PsdError::Depth(16)), "{err:?}");
    assert!(err.to_string().contains("16 bits per channel"), "{err}");
}

#[test]
fn psd_psb_and_non_rgb_modes_are_rejected_naming_the_property() {
    let mut psb = psd(3, 2, None, &merged_3x2());
    psb[4..6].copy_from_slice(&2u16.to_be_bytes());
    let err = open_err(&psb);
    assert!(matches!(err, PsdError::Psb), "{err:?}");
    assert!(err.to_string().contains("PSB"), "{err}");

    for (mode, name) in [(4u16, "CMYK"), (9, "Lab"), (1, "Grayscale"), (2, "Indexed")] {
        let mut bytes = psd(3, 2, None, &merged_3x2());
        bytes[24..26].copy_from_slice(&mode.to_be_bytes());
        let err = open_err(&bytes);
        assert!(
            matches!(err, PsdError::ColorMode(m) if m == mode),
            "{err:?}"
        );
        assert!(err.to_string().contains(name), "{err}");
    }
}

#[test]
fn psd_truncated_or_invalid_header_is_an_error() {
    let bytes = psd(3, 2, None, &merged_3x2());
    for len in [0, 4, 12, 25] {
        assert!(
            matches!(open_err(&bytes[..len]), PsdError::Truncated),
            "len {len}"
        );
    }
    let mut version = bytes.clone();
    version[4..6].copy_from_slice(&7u16.to_be_bytes());
    assert!(matches!(open_err(&version), PsdError::InvalidHeader(_)));
    let mut zero = bytes.clone();
    zero[18..22].copy_from_slice(&0u32.to_be_bytes());
    assert!(matches!(open_err(&zero), PsdError::InvalidHeader(_)));
    let mut sig = bytes.clone();
    sig[0] = b'X';
    assert!(matches!(open_err(&sig), PsdError::InvalidHeader(_)));
    // A section length running past the end of the file.
    let mut resources = bytes;
    resources[30..34].copy_from_slice(&1000u32.to_be_bytes());
    assert!(matches!(open_err(&resources), PsdError::Truncated));
}

#[test]
fn psd_unreadable_composite_is_an_error() {
    let bytes = psd(3, 2, None, &merged_3x2());
    // Cut into the RLE rows.
    let err = open_err(&bytes[..bytes.len() - 3]);
    assert!(matches!(err, PsdError::Composite(_)), "{err:?}");
    // ZIP-compressed composite.
    let mut zip = psd_with(3, 2, None, &merged_3x2(), 3, false);
    let at = 26 + 4 + 4 + 4;
    zip[at..at + 2].copy_from_slice(&2u16.to_be_bytes());
    assert!(matches!(open_err(&zip), PsdError::Composite(_)));
}

#[test]
fn psd_canvas_over_the_limits_is_an_error() {
    let bytes = psd(3, 2, None, &merged_3x2());
    let small = PsdLimits {
        max_dim: 2,
        max_area: 100,
    };
    let err = from_psd_bytes(&bytes, small).expect_err("too wide");
    assert!(
        matches!(
            err,
            PsdError::CanvasTooLarge {
                width: 3,
                height: 2
            }
        ),
        "{err:?}"
    );
    let area = PsdLimits {
        max_dim: 100,
        max_area: 5,
    };
    assert!(matches!(
        from_psd_bytes(&bytes, area),
        Err(PsdError::CanvasTooLarge { .. })
    ));
}

#[test]
fn psd_composite_matches_compose_of_the_document() {
    let merged = merged_3x2();
    let doc = open(&psd(3, 2, None, &merged)).document;
    let out = compose(doc.layers());
    assert_eq!(px(&out, 2, 1), merged[5]);
}

// ---------------------------------------------------------------- layers, groups

fn names(doc: &Document) -> Vec<&str> {
    doc.layers().iter().map(|l| l.name.as_str()).collect()
}

/// A canvas-filling layer of one color on a 4 × 3 canvas.
fn full(name: &str, color: [u8; 4]) -> L {
    L::new(name, (0, 0, 3, 4), color)
}

fn layered(layers: &[L]) -> Vec<u8> {
    psd(4, 3, Some(layers), &[[9, 9, 9, 255]; 12])
}

#[test]
fn psd_two_layers_keep_order_opacity_blend() {
    let mut ink = full("Ink", [0, 0, 255, 255]);
    ink.opacity = 128;
    ink.blend = *b"mul ";
    let mut hidden = full("Hidden", [0, 255, 0, 255]);
    hidden.hidden = true;
    // A Unicode name block wins over the Pascal name.
    let mut luni = Vec::new();
    let text: Vec<u16> = "Tinte ä".encode_utf16().collect();
    be32(&mut luni, text.len() as u32);
    for u in text {
        be16(&mut luni, u);
    }
    let named = full("Tinte ?", [1, 2, 3, 255]).block(b"luni", luni);
    let import = open(&layered(&[
        full("Back", [255, 0, 0, 255]),
        ink,
        hidden,
        named,
    ]));
    let doc = &import.document;
    assert_eq!((doc.canvas.width(), doc.canvas.height()), (4, 3));
    assert_eq!(names(doc), ["Back", "Ink", "Hidden", "Tinte ä"]);
    let l = doc.layers();
    assert!((l[1].opacity - 128.0 / 255.0).abs() < 1e-6);
    assert_eq!(l[0].opacity, 1.0);
    assert_eq!(l[1].blend_mode, BlendMode::Multiply);
    assert_eq!(l[0].blend_mode, BlendMode::Normal);
    assert!(l[0].visible && l[1].visible && !l[2].visible && l[3].visible);
    assert_eq!(
        px(&l[2].pixels, 3, 2),
        [0, 255, 0, 255],
        "hidden pixels intact"
    );
    assert_eq!(px(&l[1].pixels, 0, 0), [0, 0, 255, 255]);
    assert!(l.iter().all(|l| l.parent.is_none() && !l.is_group()));
    assert!(import.notices.is_empty(), "{:?}", import.notices);
}

#[test]
fn psd_offset_layer_is_placed_at_bounds() {
    let mut rle = L::new("Inner", (1, 2, 3, 4), [10, 20, 30, 200]);
    rle.compression = Compression::Rle;
    let edge = L::new("Edge", (-1, -1, 1, 1), [50, 60, 70, 255]);
    let mut opaque = L::new("NoAlpha", (2, 0, 3, 1), [80, 90, 100, 0]);
    opaque.alpha = false;
    let empty = L::new("Empty", (0, 0, 0, 0), [0; 4]);
    let doc = open(&layered(&[rle, edge, opaque, empty])).document;
    assert_eq!(names(&doc), ["Inner", "Edge", "NoAlpha", "Empty"]);
    let l = doc.layers();
    for y in 0..3 {
        for x in 0..4 {
            let inside = (2..4).contains(&x) && (1..3).contains(&y);
            let want = if inside { [10, 20, 30, 200] } else { [0; 4] };
            assert_eq!(px(&l[0].pixels, x, y), want, "Inner at {x},{y}");
            let edge = if (x, y) == (0, 0) {
                [50, 60, 70, 255]
            } else {
                [0; 4]
            };
            assert_eq!(px(&l[1].pixels, x, y), edge, "Edge at {x},{y}");
            let no_alpha = if (x, y) == (0, 2) {
                [80, 90, 100, 255]
            } else {
                [0; 4]
            };
            assert_eq!(px(&l[2].pixels, x, y), no_alpha, "NoAlpha at {x},{y}");
            assert_eq!(px(&l[3].pixels, x, y), [0; 4]);
        }
    }
    assert_eq!((l[3].pixels.width(), l[3].pixels.height()), (4, 3));
}

#[test]
fn psd_blend_keys_map_to_blend_modes() {
    let table: [(&[u8; 4], BlendMode); 12] = [
        (b"norm", BlendMode::Normal),
        (b"mul ", BlendMode::Multiply),
        (b"scrn", BlendMode::Screen),
        (b"over", BlendMode::Overlay),
        (b"dark", BlendMode::Darken),
        (b"lite", BlendMode::Lighten),
        (b"div ", BlendMode::ColorDodge),
        (b"idiv", BlendMode::ColorBurn),
        (b"hLit", BlendMode::HardLight),
        (b"sLit", BlendMode::SoftLight),
        (b"diff", BlendMode::Difference),
        (b"smud", BlendMode::Exclusion),
    ];
    let layers: Vec<L> = table
        .iter()
        .map(|(key, _)| {
            let mut l = full(std::str::from_utf8(*key).expect("ascii"), [1, 1, 1, 255]);
            l.blend = **key;
            l
        })
        .collect();
    let import = open(&layered(&layers));
    for (layer, (_, mode)) in import.document.layers().iter().zip(table) {
        assert_eq!(layer.blend_mode, mode, "{}", layer.name);
    }
    assert!(import.notices.is_empty(), "{:?}", import.notices);
}

#[test]
fn psd_unknown_blend_key_falls_back_to_normal() {
    let mut hue = full("Glow", [1, 1, 1, 255]);
    hue.blend = *b"hue ";
    let mut dodge = full("Shine", [1, 1, 1, 255]);
    dodge.blend = *b"lddg";
    let import = open(&layered(&[hue, dodge]));
    assert!(
        import
            .document
            .layers()
            .iter()
            .all(|l| l.blend_mode == BlendMode::Normal)
    );
    assert_eq!(
        import.notices,
        [
            PsdNotice::Blend {
                layer: "Glow".into(),
                key: "hue".into()
            },
            PsdNotice::Blend {
                layer: "Shine".into(),
                key: "lddg".into()
            },
        ]
    );
}

#[test]
fn psd_groups_open_as_groups() {
    let mut group = L::group_open("Folder");
    group.opacity = 51;
    let mut inner = L::group_open("Inner");
    inner.hidden = true;
    let layers = [
        full("Base", [255, 0, 0, 255]),
        L::group_end(),
        full("C1", [0, 255, 0, 255]),
        L::group_end(),
        full("Deep", [0, 0, 255, 255]),
        inner,
        full("C2", [0, 255, 0, 255]),
        group,
        full("Top", [9, 9, 9, 0]),
    ];
    let import = open(&layered(&layers));
    let doc = &import.document;
    assert_eq!(
        names(doc),
        ["Base", "Folder", "C1", "Inner", "Deep", "C2", "Top"]
    );
    let l = doc.layers();
    let folder = l[1].id;
    assert!(l[1].is_group() && l[3].is_group());
    assert!((l[1].opacity - 0.2).abs() < 1e-6);
    assert!(l[1].visible && !l[3].visible);
    assert_eq!(l[2].parent, Some(folder));
    assert_eq!(l[3].parent, Some(folder));
    assert_eq!(l[4].parent, Some(l[3].id));
    assert_eq!(l[5].parent, Some(folder));
    assert_eq!(l[6].parent, None);
    assert!(doc.is_well_formed());
    assert!(import.notices.is_empty(), "{:?}", import.notices);
}

#[test]
fn psd_hidden_group_children_never_show() {
    let mut group = L::group_open("Hidden folder");
    group.hidden = true;
    let layers = [
        full("Base", [0, 0, 255, 255]),
        L::group_end(),
        full("Red", [255, 0, 0, 255]),
        group,
    ];
    let doc = open(&layered(&layers)).document;
    let out = compose(doc.layers());
    assert_eq!(px(&out, 1, 1), [0, 0, 255, 255]);
}

#[test]
fn psd_pass_group_opens_normal_and_is_noticed_only_with_a_blending_descendant() {
    let plain = [
        L::group_end(),
        full("A", [1, 1, 1, 255]),
        L::group_open("Plain"),
    ];
    let import = open(&layered(&plain));
    assert_eq!(import.document.layers()[0].blend_mode, BlendMode::Normal);
    assert!(import.notices.is_empty(), "{:?}", import.notices);

    let mut mul = full("B", [1, 1, 1, 255]);
    mul.blend = *b"mul ";
    let blending = [L::group_end(), mul, L::group_open("Pass")];
    let import = open(&layered(&blending));
    assert_eq!(import.document.layers()[0].blend_mode, BlendMode::Normal);
    assert_eq!(
        import.notices,
        [PsdNotice::PassThrough {
            group: "Pass".into()
        }]
    );

    // A group with its own blend key keeps it.
    let mut screen = L::group_open("Screen");
    screen.section = Some((1, Some(*b"scrn")));
    let import = open(&layered(&[
        L::group_end(),
        full("C", [1, 1, 1, 255]),
        screen,
    ]));
    assert_eq!(import.document.layers()[0].blend_mode, BlendMode::Screen);
}

#[test]
fn psd_unrepresentable_content_is_dropped_with_notices() {
    let mut masked = full("Masked", [4, 4, 4, 255]);
    masked.mask = true;
    let mut clipped = full("Clipped", [5, 5, 5, 255]);
    clipped.clipping = 1;
    let layers = [
        full("Base", [255, 0, 0, 255]),
        full("Curves", [0; 4]).block(b"curv", vec![0; 8]),
        full("Title", [1, 1, 1, 255]).block(b"TySh", vec![0; 8]),
        full("Smart", [2, 2, 2, 255]).block(b"SoLd", vec![0; 8]),
        full("Glow", [3, 3, 3, 255]).block(b"lfx2", vec![0; 8]),
        masked,
        clipped,
    ];
    let import = open(&layered(&layers));
    let doc = &import.document;
    assert_eq!(
        names(doc),
        ["Base", "Title", "Smart", "Glow", "Masked", "Clipped"]
    );
    let l = doc.layers();
    assert_eq!(
        px(&l[1].pixels, 0, 0),
        [1, 1, 1, 255],
        "text keeps its pixels"
    );
    assert_eq!(px(&l[2].pixels, 3, 2), [2, 2, 2, 255]);
    assert_eq!(px(&l[4].pixels, 3, 2), [4, 4, 4, 255], "mask not applied");
    let layer = |s: &str| s.to_string();
    assert_eq!(
        import.notices,
        [
            PsdNotice::Adjustment {
                layer: layer("Curves")
            },
            PsdNotice::Text {
                layer: layer("Title")
            },
            PsdNotice::SmartObject {
                layer: layer("Smart")
            },
            PsdNotice::Effects {
                layer: layer("Glow")
            },
            PsdNotice::Mask {
                layer: layer("Masked")
            },
            PsdNotice::Clipping {
                layer: layer("Clipped")
            },
        ]
    );
}

fn composite_fallback(import: &PsdImport, merged: &[[u8; 4]]) {
    let doc = &import.document;
    assert_eq!(doc.layer_count(), 1);
    assert_eq!(px(&doc.layers()[0].pixels, 3, 2), merged[11]);
}

#[test]
fn psd_zip_or_unreadable_layers_fall_back_to_composite() {
    let merged: Vec<[u8; 4]> = (0..12u8).map(|i| [i, i * 2, i * 3, 255]).collect();
    let mut zip = full("Zipped", [1, 1, 1, 255]);
    zip.compression = Compression::Zip;
    let import = open(&psd(4, 3, Some(&[zip]), &merged));
    composite_fallback(&import, &merged);
    assert_eq!(
        import.notices,
        [PsdNotice::Fallback {
            reason: PsdFallback::Compressed
        }]
    );

    // A layer record whose blend signature is broken.
    let mut bytes = psd(4, 3, Some(&[full("Bad", [1, 1, 1, 255])]), &merged);
    let sig = bytes
        .windows(4)
        .position(|w| w == b"8BIM")
        .expect("signature");
    bytes[sig] = b'X';
    let import = open(&bytes);
    composite_fallback(&import, &merged);
    assert_eq!(
        import.notices,
        [PsdNotice::Fallback {
            reason: PsdFallback::Unreadable
        }]
    );

    // No layers in the layer section.
    let import = open(&psd(4, 3, Some(&[]), &merged));
    composite_fallback(&import, &merged);
    assert_eq!(
        import.notices,
        [PsdNotice::Fallback {
            reason: PsdFallback::NoLayers
        }]
    );
}

#[test]
fn psd_layer_bounds_larger_than_their_data_fall_back_to_composite() {
    let merged = [[3, 3, 3, 255]; 12];
    let mut huge = full("Huge", [1, 1, 1, 255]);
    huge.compression = Compression::Zip; // two bytes per channel
    huge.rect = (0, 0, 1_000_000, 1_000_000);
    let mut bytes = psd(4, 3, Some(&[huge]), &merged);
    // Make the channels "raw" so the reader would have to trust the rect.
    let mut i = 0;
    while let Some(at) = bytes[i..].windows(4).position(|w| w == [0, 2, 0x78, 0x9c]) {
        bytes[i + at + 1] = 0;
        i += at + 4;
    }
    let import = open(&bytes);
    assert_eq!(import.document.layer_count(), 1);
    assert_eq!(
        import.notices,
        [PsdNotice::Fallback {
            reason: PsdFallback::Unreadable
        }]
    );
}

#[test]
fn psd_only_unrepresentable_layers_open_composite() {
    let merged: Vec<[u8; 4]> = (0..12u8).map(|i| [i, 7, 7, 255]).collect();
    let layers = [full("Levels", [0; 4]).block(b"levl", vec![0; 4])];
    let import = open(&psd(4, 3, Some(&layers), &merged));
    composite_fallback(&import, &merged);
    assert_eq!(
        import.notices,
        [
            PsdNotice::Adjustment {
                layer: "Levels".into()
            },
            PsdNotice::Fallback {
                reason: PsdFallback::Unrepresentable
            },
        ]
    );
}

#[test]
fn psd_over_the_layer_limit_is_an_error() {
    let layers: Vec<L> = (0..=MAX_LAYERS)
        .map(|i| L::new(&format!("L{i}"), (0, 0, 0, 0), [0; 4]))
        .collect();
    let err = open_err(&layered(&layers));
    assert!(err.to_string().contains("layer limit"), "{err}");
}

// ---------------------------------------------------------------- fixtures

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// `tests/fixtures/sample.psd` (40 × 30, also opened by `ui/e2e/psd.spec.ts`):
/// a red base, a pass-through group holding a Multiply layer at an offset,
/// a text layer and a Curves adjustment layer.
fn sample_psd() -> Vec<u8> {
    let mut ink = L::new("Ink", (5, 10, 15, 30), [0, 0, 255, 255]);
    ink.blend = *b"mul ";
    ink.compression = Compression::Rle;
    let layers = [
        L::new("Base", (0, 0, 30, 40), [255, 0, 0, 255]),
        L::group_end(),
        ink,
        L::group_open("Folder"),
        L::new("Title", (20, 2, 25, 12), [255, 255, 255, 255]).block(b"TySh", vec![0; 8]),
        L::new("Curves", (0, 0, 0, 0), [0; 4]).block(b"curv", vec![0; 8]),
    ];
    psd(40, 30, Some(&layers), &[[255, 0, 0, 255]; 1200])
}

/// `tests/fixtures/flat.psd` (40 × 30): no layer section, only the
/// composite.
fn flat_psd() -> Vec<u8> {
    let merged: Vec<[u8; 4]> = (0..1200u32)
        .map(|i| [(i % 40 * 6) as u8, (i / 40 * 8) as u8, 128, 255])
        .collect();
    psd(40, 30, None, &merged)
}

/// The committed fixtures are what the writer above produces; run with
/// `PINCEL_WRITE_PSD_FIXTURES=1` to rewrite them after changing it.
#[test]
fn psd_fixtures_match_the_writer() {
    for (name, bytes) in [("sample.psd", sample_psd()), ("flat.psd", flat_psd())] {
        let path = fixture_path(name);
        if std::env::var_os("PINCEL_WRITE_PSD_FIXTURES").is_some() {
            std::fs::write(&path, &bytes).expect("write fixture");
        }
        assert_eq!(std::fs::read(&path).expect("fixture"), bytes, "{name}");
    }
}

#[test]
fn psd_sample_fixture_opens_with_group_and_notices() {
    let import = open(&std::fs::read(fixture_path("sample.psd")).expect("fixture"));
    let doc = &import.document;
    assert_eq!(names(doc), ["Base", "Folder", "Ink", "Title"]);
    assert_eq!(doc.layers()[2].parent, Some(doc.layers()[1].id));
    assert_eq!(px(&doc.layers()[2].pixels, 10, 5), [0, 0, 255, 255]);
    assert_eq!(px(&doc.layers()[2].pixels, 9, 5), [0; 4]);
    assert_eq!(
        import.notices,
        [
            PsdNotice::PassThrough {
                group: "Folder".into()
            },
            PsdNotice::Text {
                layer: "Title".into()
            },
            PsdNotice::Adjustment {
                layer: "Curves".into()
            },
        ]
    );
}

/// A file written by ImageMagick (`convert merged.png back.png ink.png
/// -depth 8 -type TrueColorAlpha -compress RLE`), independent of the writer
/// above: an 8 × 6 red "Back" and a transparent "Ink" with a blue rectangle
/// at x 2–5, y 1–3.
#[test]
fn psd_imagemagick_file_opens_its_layers() {
    let import = open(&std::fs::read(fixture_path("imagemagick-layers.psd")).expect("fixture"));
    let doc = &import.document;
    assert_eq!((doc.canvas.width(), doc.canvas.height()), (8, 6));
    assert_eq!(names(doc), ["Back", "Ink"]);
    for y in 0..6 {
        for x in 0..8 {
            assert_eq!(px(&doc.layers()[0].pixels, x, y), [255, 0, 0, 255]);
            let inside = (2..=5).contains(&x) && (1..=3).contains(&y);
            let ink = px(&doc.layers()[1].pixels, x, y);
            if inside {
                assert_eq!(ink, [0, 0, 255, 255], "{x},{y}");
            } else {
                assert_eq!(ink[3], 0, "{x},{y}");
            }
        }
    }
    assert!(import.notices.is_empty(), "{:?}", import.notices);
}
