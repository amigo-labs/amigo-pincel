//! The large-page check of docs/specs/ocr-page.md AC-12 for the wasm32
//! build: reads a white 8192 × 8192 px page with three text lines and
//! panics (a trap in wasm) unless the page text reaches 98 % character
//! accuracy. `tests/page.rs` builds this example for
//! `wasm32-unknown-unknown` and runs its exported `main` in Node; it needs
//! no imports, so it runs without wasm-bindgen.

const SIDE: usize = 8192;
const MODEL: &[u8] = include_bytes!("../models/eng.traineddata");
const LINES: [&[u8]; 3] = [
    include_bytes!("../tests/pages/large/line_1.png"),
    include_bytes!("../tests/pages/large/line_2.png"),
    include_bytes!("../tests/pages/large/line_3.png"),
];
const REFERENCE: &str = include_str!("../tests/pages/large/lines.txt");
/// Where the lines go, as in `tests/page.rs`.
const PLACES: [(usize, usize); 3] = [(200, 300), (3000, 4000), (5200, 7900)];

/// Decodes an 8-bit grey PNG: width, height and the grey bytes.
fn decode(bytes: &[u8]) -> (usize, usize, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .expect("valid PNG");
    let mut buf = vec![0; reader.output_buffer_size().expect("buffer size")];
    let info = reader.next_frame(&mut buf).expect("PNG frame");
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    buf.truncate(info.buffer_size());
    (info.width as usize, info.height as usize, buf)
}

fn normalize(text: &str) -> Vec<char> {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .collect()
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn main() {
    let model = pincel_ocr::Model::from_bytes(MODEL).expect("model loads");
    let mut rgba = vec![255u8; SIDE * SIDE * 4];
    for (bytes, (x0, y0)) in LINES.iter().zip(PLACES) {
        let (w, h, grey) = decode(bytes);
        for y in 0..h {
            for x in 0..w {
                let v = grey[y * w + x];
                let i = ((y0 + y) * SIDE + x0 + x) * 4;
                rgba[i..i + 3].copy_from_slice(&[v, v, v]);
            }
        }
    }
    let result = model
        .read_page(SIDE as u32, SIDE as u32, &rgba, &mut |_| {
            pincel_ocr::Control::Continue
        })
        .expect("large page reads");
    let (got, want) = (normalize(&result.text), normalize(REFERENCE));
    let accuracy = 1.0 - edit_distance(&got, &want) as f64 / want.len() as f64;
    assert!(accuracy >= 0.98, "accuracy {accuracy}: {:?}", result.text);
    println!("accuracy {accuracy:.4}");
}
