//! The system's fonts for `draw.text` and `font.list`
//! (docs/specs/system-fonts-mcp.md AC-09): the installed TrueType / OpenType
//! faces as `fontdb` enumerates them, the enumerator the desktop app uses
//! (`src-tauri/src/fonts.rs`, Decision Log 2026-11-05). Enumeration reads
//! font metadata only and runs on first use, without any user interaction;
//! a face's file is read only when `pincel-agent` asks for it.

use std::path::PathBuf;

use fontdb::{Database, Source, Style};
use pincel_agent::{FontFaceInfo, FontProvider};

/// The enumerated faces; a face's id is its index here.
#[derive(Debug, Default)]
struct Catalog {
    faces: Vec<FontFaceInfo>,
    /// The file and face index of each face, by id.
    sources: Vec<(PathBuf, u32)>,
}

/// The style name for a weight, width class and slant, as the desktop
/// app's font list builds it (`fontdb` keeps no subfamily name).
fn style_name(weight: u16, stretch: u16, style: Style) -> String {
    const WEIGHTS: [&str; 9] = [
        "Thin",
        "ExtraLight",
        "Light",
        "Regular",
        "Medium",
        "SemiBold",
        "Bold",
        "ExtraBold",
        "Black",
    ];
    const WIDTHS: [&str; 9] = [
        "UltraCondensed",
        "ExtraCondensed",
        "Condensed",
        "SemiCondensed",
        "",
        "SemiExpanded",
        "Expanded",
        "ExtraExpanded",
        "UltraExpanded",
    ];
    let w = usize::from(weight.clamp(100, 900).div_ceil(100).saturating_sub(1)).min(8);
    let width = WIDTHS[usize::from(stretch.clamp(1, 9) - 1)];
    let slant = match style {
        Style::Normal => "",
        Style::Italic => "Italic",
        Style::Oblique => "Oblique",
    };
    let weight_name = if WEIGHTS[w] == "Regular" && (!slant.is_empty() || !width.is_empty()) {
        ""
    } else {
        WEIGHTS[w]
    };
    let parts: Vec<&str> = [width, weight_name, slant]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect();
    parts.join(" ")
}

impl Catalog {
    /// The file-backed faces of `db`.
    fn from_db(db: &Database) -> Self {
        let mut catalog = Catalog::default();
        for face in db.faces() {
            let path = match &face.source {
                Source::File(path) | Source::SharedFile(path, _) => path,
                Source::Binary(_) => continue,
            };
            let Some((family, _)) = face.families.first() else {
                continue;
            };
            let stretch = face.stretch.to_number();
            catalog.faces.push(FontFaceInfo {
                id: catalog.sources.len() as u64,
                family: family.clone(),
                style: style_name(face.weight.0, stretch, face.style),
                weight: face.weight.0,
                italic: face.style != Style::Normal,
                stretch,
            });
            catalog.sources.push((path.clone(), face.index));
        }
        catalog
    }
}

/// The system's fonts, enumerated on first use.
#[derive(Debug, Default)]
pub struct SystemFonts {
    catalog: Option<Catalog>,
}

impl SystemFonts {
    /// The fonts installed on this machine (enumerated on first use).
    pub fn new() -> Self {
        Self::default()
    }

    /// The fonts of an already loaded `db` (tests, fixture fonts).
    pub fn from_db(db: &Database) -> Self {
        Self {
            catalog: Some(Catalog::from_db(db)),
        }
    }

    fn catalog(&mut self) -> &Catalog {
        self.catalog.get_or_insert_with(|| {
            let mut db = Database::new();
            db.load_system_fonts();
            Catalog::from_db(&db)
        })
    }
}

impl FontProvider for SystemFonts {
    fn faces(&mut self) -> Vec<FontFaceInfo> {
        self.catalog().faces.clone()
    }

    fn read(&mut self, id: u64) -> Result<(Vec<u8>, u32), String> {
        let (path, index) = usize::try_from(id)
            .ok()
            .and_then(|i| self.catalog().sources.get(i))
            .ok_or_else(|| format!("unknown font face {id}"))?;
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok((bytes, *index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../pincel-core/tests/fixtures")
            .join(name)
    }

    fn fixture_fonts() -> SystemFonts {
        let mut db = Database::new();
        for name in [
            "PincelTestSerif.ttc",
            "PincelTestSerif-Italic.ttf",
            "LiberationSans-LICENSE.txt",
        ] {
            // A file that is no font is skipped.
            let _ = db.load_font_file(fixture(name));
        }
        SystemFonts::from_db(&db)
    }

    #[test]
    fn faces_lists_each_face_with_family_style_and_attributes() {
        let mut fonts = fixture_fonts();
        let mut faces: Vec<(String, String, u16, bool, u16)> = fonts
            .faces()
            .into_iter()
            .map(|f| (f.family, f.style, f.weight, f.italic, f.stretch))
            .collect();
        faces.sort();
        assert_eq!(
            faces,
            vec![
                ("Pincel Test Serif".into(), "Bold".into(), 700, false, 5),
                ("Pincel Test Serif".into(), "Italic".into(), 400, true, 5),
                ("Pincel Test Serif".into(), "Regular".into(), 400, false, 5),
            ]
        );
    }

    #[test]
    fn read_returns_the_file_and_face_index_of_a_listed_face() {
        let mut fonts = fixture_fonts();
        let bold = fonts
            .faces()
            .into_iter()
            .find(|f| f.style == "Bold")
            .expect("bold face");
        let (bytes, index) = fonts.read(bold.id).expect("read");
        assert_eq!(
            bytes,
            std::fs::read(fixture("PincelTestSerif.ttc")).expect("fixture")
        );
        assert_eq!(index, 1, "the Bold face is face 1 of the collection");
        assert!(fonts.read(99).is_err());
    }

    #[test]
    fn read_of_a_removed_file_fails_with_the_reason() {
        let dir = crate::test_dir("fonts-removed");
        let path = dir.join("Serif.ttf");
        std::fs::copy(fixture("PincelTestSerif-Regular.ttf"), &path).expect("copy");
        let mut db = Database::new();
        db.load_font_file(&path).expect("load");
        let mut fonts = SystemFonts::from_db(&db);
        let id = fonts.faces()[0].id;
        std::fs::remove_file(&path).expect("remove");
        let e = fonts.read(id).expect_err("file is gone");
        assert!(e.contains("Serif.ttf"), "{e}");
    }

    #[test]
    fn style_name_names_weight_width_and_slant() {
        assert_eq!(style_name(400, 5, Style::Normal), "Regular");
        assert_eq!(style_name(700, 5, Style::Italic), "Bold Italic");
        assert_eq!(style_name(400, 5, Style::Italic), "Italic");
        assert_eq!(style_name(400, 3, Style::Normal), "Condensed");
        assert_eq!(style_name(900, 5, Style::Oblique), "Black Oblique");
    }
}
