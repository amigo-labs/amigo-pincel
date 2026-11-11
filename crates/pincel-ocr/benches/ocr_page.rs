//! `ocr-page-screenshot` (docs/specs/ocr-page.md AC-13): reading the
//! benchmark screenshot (the light 1920 × 1080 UI screenshot of the English
//! corpus) with `eng`. The budget is 3 s on the reference machine
//! (pincel.md §4.5), checked by hand in a native release build. Not part of
//! the PR gate.

use criterion::{Criterion, criterion_group, criterion_main};
use pincel_ocr::{Control, Model};

/// Decodes the corpus page (RGB) to RGBA.
fn screenshot() -> (u32, u32, Vec<u8>) {
    let bytes = include_bytes!("../tests/pages/eng/ui_light.png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(&bytes[..]))
        .read_info()
        .expect("valid PNG");
    let mut buf = vec![0; reader.output_buffer_size().expect("buffer size")];
    let info = reader.next_frame(&mut buf).expect("PNG frame");
    assert_eq!(info.color_type, png::ColorType::Rgb);
    let rgba = buf[..info.buffer_size()]
        .chunks(3)
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    (info.width, info.height, rgba)
}

fn bench(c: &mut Criterion) {
    let model = Model::from_bytes(include_bytes!("../models/eng.traineddata")).expect("model");
    let (width, height, rgba) = screenshot();
    let mut group = c.benchmark_group("ocr-page");
    group.sample_size(10);
    group.bench_function("ocr-page-screenshot", |b| {
        b.iter(|| {
            model
                .read_page(width, height, &rgba, &mut |_| Control::Continue)
                .expect("page reads")
        });
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
