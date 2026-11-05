//! paint.net's Photo menu (docs/specs/effects-artistic-photo.md): Glow, Red
//! Eye Removal, Soften Portrait, Straighten and Vignette. Sharpen and Unsharp
//! Mask, also in the Photo submenu, live in [`crate::sharpen`].
//!
//! All keep the source alpha except Straighten, which rotates it with the
//! image (bilinear on premultiplied pixels, like the resampling
//! distortions).

mod glow;
mod red_eye;
mod soften_portrait;
mod straighten;
mod vignette;

pub use glow::Glow;
pub use red_eye::RedEyeRemoval;
pub use soften_portrait::SoftenPortrait;
pub use straighten::{Straighten, StraightenFill};
pub use vignette::{Vignette, VignetteColor};
