//! Object effects: operate on the shape a layer's opaque pixels form
//! (paint.net's Effects ▸ Object submenu).

mod drop_shadow;
mod outline;

pub use drop_shadow::DropShadow;
pub use outline::{Outline, OutlinePlace, OutlineShape};
