//! No input makes loading or recognizing panic
//! (docs/specs/ocr-recognizer.md AC-09). Seeded, so a failure reproduces.

pub mod common;

use common::{DEU, ENG};
use pincel_ocr::Model;

/// A 64-bit linear congruential generator (Knuth's MMIX constants).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn recognize_line_random_images_never_panics() {
    // Image `i` comes from seed `i`, so a failure names its own input. The
    // images are spread over threads only to keep the test fast.
    let model = Model::from_bytes(ENG).expect("eng loads");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for start in 0..threads {
            let model = &model;
            s.spawn(move || {
                for i in (start..1000).step_by(threads) {
                    let mut rng = Rng(0x5eed_0000 + i as u64);
                    let w = 1 + rng.below(512) as u32;
                    let h = 1 + rng.below(128) as u32;
                    let rgba: Vec<u8> = (0..w * h * 4).map(|_| rng.next() as u8).collect();
                    let line = model.recognize_line(w, h, &rgba).expect("valid buffer");
                    assert!(
                        line.words.iter().all(|word| word.confidence <= 100),
                        "image {i}"
                    );
                }
            });
        }
    });
}

#[test]
fn from_bytes_corrupted_models_never_panic() {
    let probe: Vec<u8> = (0..48 * 16)
        .flat_map(|i| {
            if (i % 48) % 6 < 3 && (i / 48) % 12 > 3 {
                [0, 0, 0, 255]
            } else {
                [255; 4]
            }
        })
        .collect();
    let mut rng = Rng(0x5eed_0002);
    for original in [ENG, DEU] {
        for _ in 0..1000 {
            let mut data = original.to_vec();
            // 1-16 distinct positions, each byte changed to a different value.
            let mut changed: Vec<usize> = Vec::new();
            let count = 1 + rng.below(16) as usize;
            while changed.len() < count {
                let at = rng.below(data.len() as u64) as usize;
                if !changed.contains(&at) {
                    data[at] ^= 1 + rng.below(255) as u8;
                    changed.push(at);
                }
            }
            if let Ok(model) = Model::from_bytes(&data) {
                let _ = model.recognize_line(48, 16, &probe);
            }
        }
    }
}
