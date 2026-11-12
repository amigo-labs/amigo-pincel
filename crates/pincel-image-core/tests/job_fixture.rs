//! The input of the Image job digests (`ui/e2e/job-digests.spec.ts`):
//! `tests/fixtures/jobs/doc.snapshot` must be the snapshot of the document
//! built here, so a change to the recipe or to the snapshot format shows up
//! as a failing test instead of digests that silently pin something else.
//!
//! `PINCEL_WRITE_JOB_FIXTURES=1` rewrites the fixture; the digests in
//! `cases.json` are then recorded again with `PINCEL_RECORD_JOB_DIGESTS=1`.

use std::path::PathBuf;

use pincel_image_core::codec::snapshot::to_snapshot_bytes;
use pincel_image_core::document::{Document, ImageBuffer, Layer};
use pincel_image_core::{BlendMode, Rect, SelectionMask};
use uuid::Uuid;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/jobs")
}

/// `00000000-0000-4000-8000-<n as 12 digits>`: fixed ids, so the snapshot
/// bytes are fixed too.
fn id(n: u128) -> Uuid {
    let digits = format!("{n:012}");
    let low = u128::from_str_radix(&digits, 16).expect("decimal digits are hex digits");
    Uuid::from_u128((0x4000_u128 << 64) | (0x8000_u128 << 48) | low)
}

/// A 64×48 document with two raster layers (the top one translucent,
/// multiplied, at 80 %) and an anti-aliased elliptical selection.
fn fixture_document() -> Document {
    let (w, h) = (64u32, 48u32);
    let pixels = |f: &dyn Fn(u32, u32) -> [u8; 4]| {
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&f(x, y));
            }
        }
        ImageBuffer::from_raw(w, h, data).expect("fixture pixels")
    };
    let mut doc = Document::new(w, h).expect("fixture document");
    doc.id = id(1);
    doc.title = "fixture".to_string();
    doc.metadata.created_at = 0;
    doc.metadata.modified_at = 0;
    let mut base = Layer::from_pixels(
        "base",
        pixels(&|x, y| [(x * 4) as u8, (y * 5) as u8, ((x ^ y) * 8) as u8, 255]),
    );
    base.id = id(2);
    let mut paint = Layer::from_pixels(
        "paint",
        pixels(&|x, y| {
            [
                ((x * y) % 256) as u8,
                (200 - x) as u8,
                (y * 3) as u8,
                (((x + y) * 7) % 256) as u8,
            ]
        }),
    )
    .with_opacity(0.8)
    .with_blend_mode(BlendMode::Multiply);
    paint.id = id(3);
    doc.remove_layer(0).ok();
    doc.insert_layer(0, base).expect("base layer");
    doc.insert_layer(1, paint).expect("paint layer");
    while doc.layer_count() > 2 {
        doc.remove_layer(2).expect("extra layer");
    }
    doc.set_active_layer(1).expect("active layer");
    doc.selection = Some(SelectionMask::ellipse_aa(w, h, Rect::new(8, 6, 44, 34)));
    doc
}

#[test]
fn fixture_snapshot_matches_its_recipe() {
    let bytes = to_snapshot_bytes(&fixture_document());
    let path = fixtures().join("doc.snapshot");
    if std::env::var_os("PINCEL_WRITE_JOB_FIXTURES").is_some() {
        std::fs::create_dir_all(fixtures()).expect("fixture dir");
        std::fs::write(&path, &bytes).expect("write fixture");
    }
    let committed = std::fs::read(&path).expect("doc.snapshot");
    assert!(committed == bytes, "doc.snapshot is stale");
}

#[test]
fn id_spells_the_fixed_uuid_pattern() {
    assert_eq!(id(3).to_string(), "00000000-0000-4000-8000-000000000003");
}
