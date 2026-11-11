//! System fonts for the Text tools (docs/specs/system-fonts.md AC-15, AC-16):
//! the installed TrueType / OpenType faces as `fontdb` enumerates them, and
//! the bytes of one listed font file. The webview picks the face and hands
//! its bytes to the core; nothing here rasterizes.
//!
//! A face read stays inside the enumerated set: `system_font_file` takes the
//! index of a file the list returned, never a path, so it does not widen the
//! file-read surface of the open file commands.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fontdb::{Database, Source, Style};
use serde::Serialize;

/// One installed face as the font list sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemFace {
    /// Index of the face's file in the catalog (`system_font_file`).
    pub file: u32,
    /// Face index inside the file (a collection holds several).
    pub index: u32,
    /// The family name `fontdb` reports (typographic family first).
    pub family: String,
    /// A style name built from weight, width and slant ("Bold Italic").
    pub style: String,
    /// 1–1000.
    pub weight: u16,
    /// Italic or oblique.
    pub italic: bool,
    /// Width class 1–9; 5 is normal width.
    pub stretch: u16,
    /// PostScript name.
    pub postscript: String,
}

/// The enumerated faces and the files they live in.
#[derive(Debug, Default)]
pub struct Catalog {
    faces: Vec<SystemFace>,
    files: Vec<PathBuf>,
}

/// The style name for a weight, width class and slant.
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
    /// The file-backed faces of `db`, each file once.
    pub fn from_db(db: &Database) -> Self {
        let mut catalog = Catalog::default();
        for face in db.faces() {
            let path = match &face.source {
                Source::File(path) | Source::SharedFile(path, _) => path,
                Source::Binary(_) => continue,
            };
            let Some((family, _)) = face.families.first() else {
                continue;
            };
            let file = match catalog.files.iter().position(|p| p == path) {
                Some(i) => i,
                None => {
                    catalog.files.push(path.clone());
                    catalog.files.len() - 1
                }
            };
            let stretch = face.stretch.to_number();
            catalog.faces.push(SystemFace {
                file: u32::try_from(file).unwrap_or(u32::MAX),
                index: face.index,
                family: family.clone(),
                style: style_name(face.weight.0, stretch, face.style),
                weight: face.weight.0,
                italic: face.style != Style::Normal,
                stretch,
                postscript: face.post_script_name.clone(),
            });
        }
        catalog
    }

    /// The listed faces.
    pub fn faces(&self) -> &[SystemFace] {
        &self.faces
    }

    /// The bytes of the listed file `file`.
    pub fn read_file(&self, file: u32) -> Result<Vec<u8>, String> {
        let path = self
            .files
            .get(file as usize)
            .ok_or_else(|| format!("unknown font file {file}"))?;
        std::fs::read(path).map_err(|e| format!("read font: {e}"))
    }
}

/// The catalog, enumerated on first use.
#[derive(Default)]
pub struct SystemFonts(Mutex<Option<Arc<Catalog>>>);

impl SystemFonts {
    fn catalog(&self) -> Arc<Catalog> {
        let mut slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        slot.get_or_insert_with(|| {
            let mut db = Database::new();
            db.load_system_fonts();
            Arc::new(Catalog::from_db(&db))
        })
        .clone()
    }
}

/// Every installed TrueType / OpenType face (AC-15).
#[tauri::command]
pub async fn system_font_faces(
    fonts: tauri::State<'_, SystemFonts>,
) -> Result<Vec<SystemFace>, String> {
    Ok(fonts.catalog().faces().to_vec())
}

/// The bytes of one listed font file, as a binary IPC response.
#[tauri::command]
pub async fn system_font_file(
    fonts: tauri::State<'_, SystemFonts>,
    file: u32,
) -> Result<tauri::ipc::Response, String> {
    fonts
        .catalog()
        .read_file(file)
        .map(tauri::ipc::Response::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/pincel-core/tests/fixtures")
            .join(name)
    }

    fn catalog() -> Catalog {
        let mut db = Database::new();
        for name in [
            "PincelTestSerif.ttc",
            "PincelTestSerif-Italic.ttf",
            "LiberationSans-Regular.ttf",
            "LiberationSans-LICENSE.txt",
        ] {
            // A file that is no font is skipped by fontdb (AC-16).
            let _ = db.load_font_file(fixture(name));
        }
        Catalog::from_db(&db)
    }

    #[test]
    fn from_db_lists_each_face_with_its_file_and_index() {
        let c = catalog();
        let mut faces: Vec<(String, String, u32, u16, bool)> = c
            .faces()
            .iter()
            .map(|f| {
                (
                    f.family.clone(),
                    f.style.clone(),
                    f.index,
                    f.weight,
                    f.italic,
                )
            })
            .collect();
        faces.sort();
        assert_eq!(
            faces,
            vec![
                ("Liberation Sans".into(), "Regular".into(), 0, 400, false),
                ("Pincel Test Serif".into(), "Bold".into(), 1, 700, false),
                ("Pincel Test Serif".into(), "Italic".into(), 0, 400, true),
                ("Pincel Test Serif".into(), "Regular".into(), 0, 400, false),
            ]
        );
        let ttc: Vec<&SystemFace> = c
            .faces()
            .iter()
            .filter(|f| f.postscript.starts_with("PincelTestSerif-Regular") || f.style == "Bold")
            .collect();
        assert_eq!(ttc.len(), 2);
        assert_eq!(ttc[0].file, ttc[1].file, "a collection is one file");
        assert_eq!(c.files.len(), 3);
    }

    #[test]
    fn read_file_returns_a_listed_file_and_refuses_others() {
        let c = catalog();
        let bold = c
            .faces()
            .iter()
            .find(|f| f.style == "Bold")
            .expect("bold face");
        let bytes = c.read_file(bold.file).expect("read");
        assert_eq!(
            bytes,
            std::fs::read(fixture("PincelTestSerif.ttc")).expect("fixture")
        );
        assert!(c.read_file(99).is_err());
    }

    #[test]
    fn style_name_names_weight_width_and_slant() {
        assert_eq!(style_name(400, 5, Style::Normal), "Regular");
        assert_eq!(style_name(700, 5, Style::Italic), "Bold Italic");
        assert_eq!(style_name(400, 5, Style::Italic), "Italic");
        assert_eq!(style_name(400, 3, Style::Normal), "Condensed");
        assert_eq!(style_name(900, 5, Style::Oblique), "Black Oblique");
        assert_eq!(style_name(350, 5, Style::Normal), "Regular");
    }
}
