//! Sharpen effects (Fineliner spec §11.2): Unsharp Mask and a fixed one-step Sharpen.

mod convolution;
mod unsharp_mask;

pub use convolution::Sharpen;
pub use unsharp_mask::UnsharpMask;
