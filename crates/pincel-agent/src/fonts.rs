//! Fonts for `draw.text`: the bundled Liberation Sans plus the faces the
//! host offers through a [`FontProvider`], the face choice of
//! docs/specs/system-fonts.md ("Terms", AC-07, AC-23) and `font.list`. The
//! crate enumerates nothing and reads no file itself: the headless server
//! and the desktop app each own the enumeration (system-fonts-mcp,
//! Decisions 2026-11-05).

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use pincel_image_core::{FontFace, Point, Text, TextStyle};
use serde_json::{Value, json};

use crate::args::Args;
use crate::image::FONT;
use crate::pixel::tool_schema;
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// The family name of the bundled face.
pub(crate) const BUNDLED_FAMILY: &str = "Liberation Sans";

/// Normal width class (OpenType `usWidthClass`).
const NORMAL_STRETCH: u16 = 5;

/// One face a host offers, as its font list reports it. Listing faces reads
/// font metadata only, never a face (system-fonts-mcp AC-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFaceInfo {
    /// Opaque id the host reads the face by ([`FontProvider::read`]).
    pub id: u64,
    /// The family name the host reports.
    pub family: String,
    /// The style name the host reports ("Bold Italic", "Condensed", …).
    pub style: String,
    /// 100–900 (1–1000 where the host reports finer weights).
    pub weight: u16,
    /// Italic or oblique.
    pub italic: bool,
    /// Width class 1–9; 5 is normal width.
    pub stretch: u16,
}

/// The fonts a host offers `draw.text` besides the bundled Liberation Sans.
/// The default ([`Workspace::new`](crate::Workspace::new)) offers none.
pub trait FontProvider: Send {
    /// Every face the host can offer, from font metadata only.
    fn faces(&mut self) -> Vec<FontFaceInfo>;
    /// The font file bytes holding the face `id` and the face's index
    /// inside it (0 unless the file is a collection), or why it cannot be
    /// read.
    fn read(&mut self, id: u64) -> Result<(Vec<u8>, u32), String>;
}

/// A host without fonts of its own: the bundled face alone.
struct NoFonts;

impl FontProvider for NoFonts {
    fn faces(&mut self) -> Vec<FontFaceInfo> {
        Vec::new()
    }

    fn read(&mut self, id: u64) -> Result<(Vec<u8>, u32), String> {
        Err(format!("no font face {id}"))
    }
}

/// A face read from the host and checked to parse: shared bytes and face
/// index.
type ReadFace = Result<(Arc<Vec<u8>>, u32), String>;

/// The session's font provider and the faces read so far; each face is read
/// at most once per session (system-fonts-mcp AC-10).
pub(crate) struct Fonts {
    provider: Box<dyn FontProvider>,
    reads: HashMap<u64, ReadFace>,
}

impl Fonts {
    pub fn new(provider: Box<dyn FontProvider>) -> Self {
        Self {
            provider,
            reads: HashMap::new(),
        }
    }

    /// Face `id`, read on first use and checked to parse.
    fn read(&mut self, id: u64) -> ReadFace {
        let provider = &mut self.provider;
        self.reads
            .entry(id)
            .or_insert_with(|| {
                let (bytes, index) = provider.read(id)?;
                if !parses(&bytes, index) {
                    return Err("the face could not be parsed as TrueType / OpenType".to_owned());
                }
                Ok((Arc::new(bytes), index))
            })
            .clone()
    }
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new(Box::new(NoFonts))
    }
}

impl fmt::Debug for Fonts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fonts")
            .field("read", &self.reads.len())
            .finish_non_exhaustive()
    }
}

/// Whether face `index` of `bytes` parses: the Text tool's own parser
/// decides, so a face that passes here draws.
fn parses(bytes: &[u8], index: u32) -> bool {
    Text::new("", Point::new(0.0, 0.0), TextStyle::default())
        .layout_lines(FontFace::new(bytes, index))
        .is_some()
}

// --- Face choice (docs/specs/system-fonts.md "Terms") ----------------------
//
// The same rules as the editor's ui/src/lib/fonts/faces.ts; the case table
// src/face-choice-cases.json is checked against both.

/// The face a text is drawn with (`None`: the bundled face) and what is
/// left to synthesize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FaceUse<'a> {
    pub face: Option<&'a FontFaceInfo>,
    pub synth_bold: bool,
    pub synth_italic: bool,
}

/// The faces a Text tool uses for Regular, Bold, Italic and Bold Italic;
/// a `regular` of `None` is the bundled face.
struct FamilyFaces<'a> {
    regular: Option<&'a FontFaceInfo>,
    bold: Option<&'a FontFaceInfo>,
    italic: Option<&'a FontFaceInfo>,
    bold_italic: Option<&'a FontFaceInfo>,
}

/// `a` is a better pick than `b` for weight `target`: closer weight, then
/// normal width, then name order.
fn better(a: &FontFaceInfo, b: &FontFaceInfo, target: u16) -> bool {
    let da = a.weight.abs_diff(target);
    let db = b.weight.abs_diff(target);
    if da != db {
        return da < db;
    }
    let na = a.stretch == NORMAL_STRETCH;
    let nb = b.stretch == NORMAL_STRETCH;
    if na != nb {
        return na;
    }
    a.style < b.style
}

fn closest<'a>(
    faces: impl IntoIterator<Item = &'a FontFaceInfo>,
    target: u16,
) -> Option<&'a FontFaceInfo> {
    faces.into_iter().fold(None, |best, f| match best {
        Some(b) if !better(f, b, target) => Some(b),
        _ => Some(f),
    })
}

/// The bold face among `faces` other than `regular`: weight at least 600,
/// closest to 700.
fn bold_of<'a>(
    faces: &[&'a FontFaceInfo],
    regular: Option<&'a FontFaceInfo>,
) -> Option<&'a FontFaceInfo> {
    closest(
        faces
            .iter()
            .copied()
            .filter(|f| !regular.is_some_and(|r| std::ptr::eq(*f, r)) && f.weight >= 600),
        700,
    )
}

/// The faces of one family; a family without upright faces uses its
/// slanted ones as upright. `None` when `faces` is empty.
fn choose_faces<'a>(faces: &[&'a FontFaceInfo]) -> Option<FamilyFaces<'a>> {
    let slanted: Vec<&FontFaceInfo> = faces.iter().copied().filter(|f| f.italic).collect();
    let upright_only: Vec<&FontFaceInfo> = faces.iter().copied().filter(|f| !f.italic).collect();
    let upright = if upright_only.is_empty() {
        &slanted
    } else {
        &upright_only
    };
    let regular = closest(upright.iter().copied(), 400)?;
    let italic = closest(slanted.iter().copied(), 400);
    Some(FamilyFaces {
        regular: Some(regular),
        bold: bold_of(upright, Some(regular)),
        italic,
        bold_italic: bold_of(&slanted, italic),
    })
}

/// The faces of "Liberation Sans" (system-fonts AC-23): upright regular is
/// always the bundled face; host faces of that family serve only as its
/// Bold, Italic and Bold Italic.
fn bundled_family_faces<'a>(others: &[&'a FontFaceInfo]) -> FamilyFaces<'a> {
    let slanted: Vec<&FontFaceInfo> = others.iter().copied().filter(|f| f.italic).collect();
    let upright: Vec<&FontFaceInfo> = others.iter().copied().filter(|f| !f.italic).collect();
    let italic = closest(slanted.iter().copied(), 400);
    FamilyFaces {
        regular: None,
        bold: bold_of(&upright, None),
        italic,
        bold_italic: bold_of(&slanted, italic),
    }
}

/// The face for Bold / Italic (system-fonts AC-07): the matching face where
/// the family has it, else the nearest one plus synthesis.
fn face_for<'a>(faces: &FamilyFaces<'a>, bold: bool, italic: bool) -> FaceUse<'a> {
    let at = |face, synth_bold, synth_italic| FaceUse {
        face,
        synth_bold,
        synth_italic,
    };
    match (bold, italic) {
        (true, true) => match (faces.bold_italic, faces.bold, faces.italic) {
            (Some(f), _, _) => at(Some(f), false, false),
            (None, Some(f), _) => at(Some(f), false, true),
            (None, None, Some(f)) => at(Some(f), true, false),
            (None, None, None) => at(faces.regular, true, true),
        },
        (true, false) => match faces.bold {
            Some(f) => at(Some(f), false, false),
            None => at(faces.regular, true, false),
        },
        (false, true) => match faces.italic {
            Some(f) => at(Some(f), false, false),
            None => at(faces.regular, false, true),
        },
        (false, false) => at(faces.regular, false, false),
    }
}

/// The face family `family` (whose host faces are `faces`) uses for Bold /
/// Italic; `None` for a family without faces.
pub(crate) fn choose<'a>(
    family: &str,
    faces: &'a [FontFaceInfo],
    bold: bool,
    italic: bool,
) -> Option<FaceUse<'a>> {
    let refs: Vec<&FontFaceInfo> = faces.iter().filter(|f| f.family == family).collect();
    let chosen = if family == BUNDLED_FAMILY {
        bundled_family_faces(&refs)
    } else {
        choose_faces(&refs)?
    };
    Some(face_for(&chosen, bold, italic))
}

// --- Families and `font` matching -------------------------------------------

/// Case-insensitive name order; exact order breaks ties (as the editor's
/// font list sorts).
fn compare_families(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

/// Every family once: the bundled one plus the host's, sorted.
fn family_names(faces: &[FontFaceInfo]) -> Vec<&str> {
    let mut names: Vec<&str> = std::iter::once(BUNDLED_FAMILY)
        .chain(faces.iter().map(|f| f.family.as_str()))
        .collect();
    names.sort_by(|a, b| compare_families(a, b));
    names.dedup();
    names
}

/// The family `font` names: the exact name, else the one family that
/// matches ignoring case (system-fonts-mcp AC-06, AC-07, AC-13).
fn match_family<'a>(families: &[&'a str], font: &str) -> Result<&'a str, AgentError> {
    let unknown = || {
        AgentError::tool(format!(
            "font \"{font}\" matches no family; font.list lists the families available"
        ))
    };
    if font.is_empty() {
        return Err(unknown());
    }
    if let Some(exact) = families.iter().find(|f| **f == font) {
        return Ok(exact);
    }
    let lower = font.to_lowercase();
    let matches: Vec<&str> = families
        .iter()
        .copied()
        .filter(|f| f.to_lowercase() == lower)
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => Err(unknown()),
        many => Err(AgentError::tool(format!(
            "font \"{font}\" is ambiguous: {}; pass the exact name",
            many.join(", ")
        ))),
    }
}

/// The bytes a text is drawn with: the bundled face or a host face.
#[derive(Debug, Clone)]
pub(crate) enum FaceBytes {
    Bundled,
    Host(Arc<Vec<u8>>),
}

/// The face `draw.text` draws with and what is left to synthesize.
#[derive(Debug, Clone)]
pub(crate) struct TextFace {
    bytes: FaceBytes,
    pub index: u32,
    pub synth_bold: bool,
    pub synth_italic: bool,
}

impl TextFace {
    /// The font file holding the face.
    pub fn bytes(&self) -> &[u8] {
        match &self.bytes {
            FaceBytes::Bundled => FONT,
            FaceBytes::Host(b) => b,
        }
    }

    /// The face as the Image Text tool takes it.
    pub fn font_face(&self) -> FontFace<'_> {
        FontFace::new(self.bytes(), self.index)
    }
}

/// The face of `draw.text`'s `font`, `bold` and `italic`: without `font`
/// the bundled face with Bold / Italic synthesized, as before
/// system-fonts-mcp (AC-04); with it the family's face as the editor
/// chooses it (AC-05), read at most once per session (AC-10).
pub(crate) fn text_face(ctx: &mut Ctx<'_>, args: &Args) -> Result<TextFace, AgentError> {
    let bold = args.bool("bold").unwrap_or(false);
    let italic = args.bool("italic").unwrap_or(false);
    let bundled = |synth_bold, synth_italic| TextFace {
        bytes: FaceBytes::Bundled,
        index: 0,
        synth_bold,
        synth_italic,
    };
    let Some(font) = args.get("font") else {
        return Ok(bundled(bold, italic));
    };
    let font = font
        .as_str()
        .ok_or_else(|| AgentError::tool("argument `font` must be a family name from font.list"))?;
    let fonts = ctx.ws.fonts_mut();
    let faces = fonts.provider.faces();
    let family = match_family(&family_names(&faces), font)?.to_owned();
    let use_ = choose(&family, &faces, bold, italic).ok_or_else(|| {
        AgentError::tool(format!(
            "font \"{family}\" cannot be used: the family has no faces"
        ))
    })?;
    let Some(face) = use_.face else {
        return Ok(bundled(use_.synth_bold, use_.synth_italic));
    };
    let (bytes, index) = fonts.read(face.id).map_err(|reason| {
        AgentError::tool(format!("font \"{family}\" cannot be used: {reason}"))
    })?;
    Ok(TextFace {
        bytes: FaceBytes::Host(bytes),
        index,
        synth_bold: use_.synth_bold,
        synth_italic: use_.synth_italic,
    })
}

// --- font.list -------------------------------------------------------------

/// Style names of `faces` in face order: weight, upright before slanted,
/// then width and name; each name once.
fn style_names(mut faces: Vec<(&str, u16, bool, u16)>) -> Vec<&str> {
    faces.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(a.2.cmp(&b.2))
            .then(
                a.3.abs_diff(NORMAL_STRETCH)
                    .cmp(&b.3.abs_diff(NORMAL_STRETCH)),
            )
            .then(a.3.cmp(&b.3))
            .then(a.0.cmp(b.0))
    });
    let mut names: Vec<&str> = Vec::with_capacity(faces.len());
    for (name, ..) in faces {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

pub(crate) const LIST: Tool = Tool {
    name: "font.list",
    description: "List the font families `draw.text` can use as its `font`, each once and sorted by name, with the style names of its faces (weight order, upright before italic): the bundled Liberation Sans plus the fonts installed on the system. A non-empty `query` lists only the families whose name contains it, ignoring case. Reads no font file.",
    schema: || {
        tool_schema(
            json!({
                "query": { "type": "string", "description": "Only families whose name contains this text (case-insensitive); empty lists all" },
            }),
            &[],
        )
    },
    run: font_list,
};

fn font_list(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let faces = ctx.ws.fonts_mut().provider.faces();
    let query = args.str("query").unwrap_or("").to_lowercase();
    let families: Vec<Value> = family_names(&faces)
        .into_iter()
        .filter(|name| query.is_empty() || name.to_lowercase().contains(&query))
        .map(|name| {
            let mut styles: Vec<(&str, u16, bool, u16)> = faces
                .iter()
                .filter(|f| f.family == name)
                .map(|f| (f.style.as_str(), f.weight, f.italic, f.stretch))
                .collect();
            if name == BUNDLED_FAMILY {
                styles.push(("Regular", 400, false, NORMAL_STRETCH));
            }
            json!({ "name": name, "styles": style_names(styles) })
        })
        .collect();
    Ok(ToolOutput::json(json!({ "families": families })))
}
