//! Noise effects (Fineliner spec §11.4): Add Noise and Reduce Noise (median filter).

mod add_noise;
mod reduce_noise;

pub use add_noise::{AddNoise, NoiseChannels, NoiseType};
pub use reduce_noise::ReduceNoise;
