//! The start screen's image sample (docs/specs/first-run-tutorial.md
//! AC-14, AC-16): 3 layers, at most 100 KB.

const POSTCARD: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../ui/public/samples/postcard.ora"
));

#[test]
fn image_sample_has_three_layers() {
    assert!(POSTCARD.len() <= 100 * 1024, "{} bytes", POSTCARD.len());
    let doc = pincel_image_core::codec::ora::from_ora_bytes(POSTCARD).expect("the sample reads");
    assert_eq!(doc.layers().len(), 3);
}
