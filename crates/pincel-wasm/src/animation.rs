//! Animated export surface (APNG, GIF). See `docs/specs/pincel.md` §7.5
//! and `docs/specs/animated-export.md`.
//!
//! `exportAnimation` encodes the animation; `animationInfo` describes it
//! for the export dialog (frame count, default loop count, GIF notices)
//! without encoding. Formats cross the boundary as `"apng"` / `"gif"`; a
//! loop count of `0` plays forever.

use pincel_core::{AnimationFormat, AnimationOptions, animation_info, export_animation};
use wasm_bindgen::prelude::*;

use crate::Document;

/// What the animation export dialog shows before exporting.
#[wasm_bindgen]
pub struct AnimationInfo {
    frame_count: u32,
    default_loops: u16,
    gif_approximated: bool,
    semi_transparent: bool,
}

#[wasm_bindgen]
impl AnimationInfo {
    /// Number of frames the export contains (pingpong counts the way back).
    #[wasm_bindgen(getter, js_name = frameCount)]
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Default play count: the tag's repeat count when non-zero, else `0`
    /// (forever).
    #[wasm_bindgen(getter, js_name = defaultLoops)]
    pub fn default_loops(&self) -> u16 {
        self.default_loops
    }

    /// A frame does not fit a 256-entry GIF colour table, so a GIF export
    /// approximates its colours.
    #[wasm_bindgen(getter, js_name = gifApproximated)]
    pub fn gif_approximated(&self) -> bool {
        self.gif_approximated
    }

    /// A frame has partially transparent pixels, which GIF reduces to
    /// on / off.
    #[wasm_bindgen(getter, js_name = semiTransparent)]
    pub fn semi_transparent(&self) -> bool {
        self.semi_transparent
    }
}

#[wasm_bindgen]
impl Document {
    /// Encode the animation as APNG or GIF bytes.
    ///
    /// Mirrors `pincel_core::export_animation`. `format` is `"apng"` or
    /// `"gif"`; `tag`, when given, exports that tag in its direction —
    /// pass `undefined` / `null` for every frame forward. `scale` is the
    /// integer nearest-neighbour factor `1..=10`; `loops` the total play
    /// count, `0` for forever (at most 65535). The returned `Box<[u8]>` is
    /// a fresh `Uint8Array` copy. Errors surface the `pincel-core`
    /// message: an unknown format or tag, a scale outside 1–10, an output
    /// edge above 16384 px, a frameless sequence, or a sprite the renderer
    /// refuses (an indexed sprite).
    #[wasm_bindgen(js_name = exportAnimation)]
    pub fn export_animation(
        &self,
        format: &str,
        tag: Option<String>,
        scale: u32,
        loops: u32,
    ) -> Result<Box<[u8]>, String> {
        let format = match format {
            "apng" => AnimationFormat::Apng,
            "gif" => AnimationFormat::Gif,
            other => return Err(format!("unknown animation format `{other}` (apng, gif)")),
        };
        let loops = u16::try_from(loops)
            .map_err(|_| format!("loop count {loops} is above the limit of 65535"))?;
        let mut opts = AnimationOptions::new(format)
            .with_scale(scale)
            .with_loops(loops);
        opts.tag = tag;
        let bytes = export_animation(&self.sprite, &self.cels, &opts)
            .map_err(|e| format!("failed to export animation: {e}"))?;
        Ok(bytes.into_boxed_slice())
    }

    /// Describe the animation `tag` selects (`undefined` / `null` = every
    /// frame) for the export dialog. Errors as
    /// [`Document::export_animation`] for the tag and the renderer.
    #[wasm_bindgen(js_name = animationInfo)]
    pub fn animation_info(&self, tag: Option<String>) -> Result<AnimationInfo, String> {
        let info = animation_info(&self.sprite, &self.cels, tag.as_deref())
            .map_err(|e| format!("failed to describe animation: {e}"))?;
        Ok(AnimationInfo {
            frame_count: info.frame_count,
            default_loops: info.default_loops,
            gif_approximated: info.gif_approximated,
            semi_transparent: info.semi_transparent,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2×2 document with three frames and a pingpong tag over all three
    /// that repeats twice.
    fn three_frames() -> Document {
        let mut doc = Document::new(2, 2).expect("doc");
        doc.add_frame(100).expect("frame");
        doc.add_frame(100).expect("frame");
        let i = doc.add_tag("walk", 0, 2, 0xff00_00ff).expect("tag");
        doc.set_tag_direction(i, "pingpong").expect("direction");
        doc.sprite.tags[i as usize].repeats = 2;
        doc
    }

    fn actl_frames(png: &[u8]) -> u32 {
        let at = png
            .windows(4)
            .position(|w| w == b"acTL")
            .expect("acTL chunk");
        u32::from_be_bytes([png[at + 4], png[at + 5], png[at + 6], png[at + 7]])
    }

    #[test]
    fn export_animation_apng_carries_actl_with_frame_count() {
        let doc = three_frames();
        let all = doc.export_animation("apng", None, 1, 0).expect("apng");
        assert_eq!(&all[1..4], b"PNG");
        assert_eq!(actl_frames(&all), 3);
        let walk = doc
            .export_animation("apng", Some("walk".into()), 2, 0)
            .expect("apng");
        assert_eq!(actl_frames(&walk), 4);
    }

    #[test]
    fn export_animation_gif_starts_with_gif89a_and_netscape() {
        let doc = three_frames();
        let bytes = doc.export_animation("gif", None, 1, 0).expect("gif");
        assert_eq!(&bytes[..6], b"GIF89a");
        assert!(bytes.windows(11).any(|w| w == b"NETSCAPE2.0"));
    }

    #[test]
    fn export_animation_bad_arguments_return_err() {
        let doc = three_frames();
        let err = doc
            .export_animation("apng", None, 0, 0)
            .expect_err("scale 0");
        assert!(err.contains("1–10"), "{err}");
        assert!(doc.export_animation("apng", None, 11, 0).is_err());
        let err = doc
            .export_animation("gif", Some("nope".into()), 1, 0)
            .expect_err("unknown tag");
        assert!(err.contains("nope"), "{err}");
        assert!(doc.export_animation("webp", None, 1, 0).is_err());
        assert!(doc.export_animation("gif", None, 1, 65_536).is_err());
    }

    #[test]
    fn export_animation_over_edge_limit_names_the_limit() {
        let doc = Document::new(1639, 1).expect("doc");
        let err = doc
            .export_animation("gif", None, 10, 0)
            .expect_err("too large");
        assert!(err.contains("16384"), "{err}");
    }

    #[test]
    fn animation_info_reports_count_loops_and_notices() {
        let doc = three_frames();
        let all = doc.animation_info(None).expect("info");
        assert_eq!((all.frame_count(), all.default_loops()), (3, 0));
        assert!(!all.gif_approximated() && !all.semi_transparent());
        let walk = doc.animation_info(Some("walk".into())).expect("info");
        assert_eq!((walk.frame_count(), walk.default_loops()), (4, 2));
        assert!(doc.animation_info(Some("nope".into())).is_err());
    }
}
