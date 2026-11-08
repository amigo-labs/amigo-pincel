//! Shared helpers of the integration tests: corpus files and models.

use std::path::{Path, PathBuf};

pub const ENG: &[u8] = include_bytes!("../../models/eng.traineddata");
pub const DEU: &[u8] = include_bytes!("../../models/deu.traineddata");

/// A decoded line image.
pub struct Line {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decodes a corpus PNG (RGB or RGBA) to RGBA.
pub fn load_png(path: &Path) -> Line {
    let file = std::fs::File::open(path).expect("corpus PNG exists");
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .expect("valid PNG");
    let mut buf = vec![0; reader.output_buffer_size().expect("buffer size")];
    let info = reader.next_frame(&mut buf).expect("PNG frame");
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::Rgba => data.to_vec(),
        other => panic!("unexpected PNG colour type {other:?}"),
    };
    Line {
        width: info.width,
        height: info.height,
        rgba,
    }
}

/// A word row (level 5) of a Tesseract TSV: text and left, top, width,
/// height.
#[derive(Debug, Clone)]
pub struct TsvWord {
    pub text: String,
    pub bbox: [i32; 4],
}

pub fn read_tsv(path: &Path) -> Vec<TsvWord> {
    std::fs::read_to_string(path)
        .expect("corpus TSV exists")
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 12 || f[0] != "5" {
                return None;
            }
            let n = |i: usize| f[i].parse::<i32>().expect("integer TSV field");
            Some(TsvWord {
                text: f[11].to_owned(),
                bbox: [n(6), n(7), n(8), n(9)],
            })
        })
        .collect()
}

/// The corpus lines of a language, sorted by name.
pub fn corpus(lang: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(lang);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("corpus directory")
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    paths.sort();
    paths
}
