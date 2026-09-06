//! A serialization-free description of one effect invocation.
//!
//! Host boundaries (the wasm layer, a native shell) hand over an effect
//! *name* plus a flat `f64` parameter vector; [`EffectSpec::from_params`]
//! validates and types it, [`EffectSpec::run`] executes it. Keeping the
//! wire shape this dumb means neither `pincel-effects` nor the host needs a
//! serde dependency, and the TypeScript side mirrors one table of
//! `(name, parameter order)` pairs.
//!
//! Choice-style parameters (a blur kind, an algorithm) travel as small
//! integer indices; booleans as `0` / `1`. Every constructor clamps or
//! defaults out-of-range values the same way the underlying effect does.

use crate::adjust::{
    BrightnessContrast, ColorBalance, CurveChannel, Curves, Grayscale, GrayscaleMethod,
    HueSaturation, Invert, Levels, LevelsChannel, Posterize, Threshold,
};
use crate::blur::{BoxBlur, GaussianBlur, MotionBlur, RadialBlur, RadialKind};
use crate::distort::{EdgeAlgorithm, EdgeDetect, Emboss, Relief};
use crate::effect::Effect;
use crate::error::EffectError;
use crate::image::EffectImage;
use crate::noise::{AddNoise, NoiseChannels, NoiseType, ReduceNoise};
use crate::sharpen::{Sharpen, UnsharpMask};

/// Every effect and adjustment the crate ships, with typed parameters.
///
/// The doc comment on each variant lists the parameter vector
/// [`EffectSpec::from_params`] expects, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectSpec {
    /// `gaussian_blur`: `[radius]`.
    GaussianBlur { radius: f32 },
    /// `box_blur`: `[width, height]`.
    BoxBlur { width: u32, height: u32 },
    /// `motion_blur`: `[distance, angle_degrees]`.
    MotionBlur { distance: f32, angle: f32 },
    /// `radial_blur`: `[amount, center_x, center_y, kind]` with `kind`
    /// `0` = spin, `1` = zoom.
    RadialBlur {
        amount: f32,
        center_x: f32,
        center_y: f32,
        kind: RadialKind,
    },
    /// `sharpen`: no parameters.
    Sharpen,
    /// `unsharp_mask`: `[amount_percent, radius, threshold]`.
    UnsharpMask {
        amount: f32,
        radius: f32,
        threshold: f32,
    },
    /// `emboss`: `[angle, elevation, relief]`.
    Emboss {
        angle: f32,
        elevation: f32,
        relief: f32,
    },
    /// `edge_detect`: `[algorithm, amount]` with `algorithm` `0` = Sobel,
    /// `1` = Prewitt, `2` = Laplacian.
    EdgeDetect {
        algorithm: EdgeAlgorithm,
        amount: f32,
    },
    /// `relief`: `[angle, amount]`.
    Relief { angle: f32, amount: f32 },
    /// `add_noise`: `[amount, noise_type, channels, seed]` with
    /// `noise_type` `0` = uniform, `1` = gaussian and `channels`
    /// `0` = RGB, `1` = monochromatic.
    AddNoise {
        amount: u32,
        noise_type: NoiseType,
        channels: NoiseChannels,
        seed: u64,
    },
    /// `reduce_noise`: `[radius]`.
    ReduceNoise { radius: u32 },
    /// `brightness_contrast`: `[brightness, contrast, enhanced]`.
    BrightnessContrast {
        brightness: f32,
        contrast: f32,
        enhanced: bool,
    },
    /// `hue_saturation`: `[hue, saturation, lightness, colorize]`.
    HueSaturation {
        hue: f32,
        saturation: f32,
        lightness: f32,
        colorize: bool,
    },
    /// `curves`: `[channel, x0, y0, x1, y1, …]` with `channel`
    /// `0` = composite, `1` = red, `2` = green, `3` = blue, `4` = alpha
    /// and at least one `(x, y)` pair in `0..=255`.
    Curves {
        channel: CurveChannel,
        points: Vec<[f32; 2]>,
    },
    /// `levels`: `[channel, in_black, in_white, gamma, out_black,
    /// out_white]` with `channel` `0` = composite, `1` = red, `2` = green,
    /// `3` = blue.
    Levels {
        channel: LevelsChannel,
        in_black: f32,
        in_white: f32,
        gamma: f32,
        out_black: f32,
        out_white: f32,
    },
    /// `color_balance`: `[s_cr, s_mg, s_yb, m_cr, m_mg, m_yb, h_cr, h_mg,
    /// h_yb, preserve_luminosity]` (shadows / midtones / highlights, each
    /// cyan–red, magenta–green, yellow–blue in `-100..=100`).
    ColorBalance {
        shadows: [f32; 3],
        midtones: [f32; 3],
        highlights: [f32; 3],
        preserve_luminosity: bool,
    },
    /// `invert`: no parameters.
    Invert,
    /// `grayscale`: `[method]` or `[method, r, g, b]` with `method`
    /// `0` = luminosity, `1` = average, `2` = BT.709, `3` = channel mixer
    /// (which uses the three weights).
    Grayscale {
        method: GrayscaleMethod,
        mixer: [f32; 3],
    },
    /// `posterize`: `[levels]`.
    Posterize { levels: u32 },
    /// `threshold`: `[threshold]`.
    Threshold { threshold: u8 },
}

/// Wire names accepted by [`EffectSpec::from_params`], in menu order.
pub const EFFECT_NAMES: &[&str] = &[
    "gaussian_blur",
    "box_blur",
    "motion_blur",
    "radial_blur",
    "sharpen",
    "unsharp_mask",
    "emboss",
    "edge_detect",
    "relief",
    "add_noise",
    "reduce_noise",
    "brightness_contrast",
    "hue_saturation",
    "curves",
    "levels",
    "color_balance",
    "invert",
    "grayscale",
    "posterize",
    "threshold",
];

fn expect_len(name: &str, params: &[f64], expected: usize) -> Result<(), EffectError> {
    if params.len() == expected {
        Ok(())
    } else {
        Err(EffectError::ParamCount {
            effect: name.to_owned(),
            expected,
            got: params.len(),
        })
    }
}

fn f(v: f64) -> f32 {
    if v.is_finite() { v as f32 } else { 0.0 }
}

fn u(v: f64) -> u32 {
    if v.is_finite() {
        v.round().clamp(0.0, u32::MAX as f64) as u32
    } else {
        0
    }
}

fn b(v: f64) -> bool {
    v.is_finite() && v != 0.0
}

impl EffectSpec {
    /// Build a spec from its wire `name` and parameter vector.
    ///
    /// Errors on an unknown name or a wrong parameter count; values are
    /// otherwise passed to the effect constructors, which clamp.
    pub fn from_params(name: &str, p: &[f64]) -> Result<Self, EffectError> {
        let spec = match name {
            "gaussian_blur" => {
                expect_len(name, p, 1)?;
                Self::GaussianBlur { radius: f(p[0]) }
            }
            "box_blur" => {
                expect_len(name, p, 2)?;
                Self::BoxBlur {
                    width: u(p[0]),
                    height: u(p[1]),
                }
            }
            "motion_blur" => {
                expect_len(name, p, 2)?;
                Self::MotionBlur {
                    distance: f(p[0]),
                    angle: f(p[1]),
                }
            }
            "radial_blur" => {
                expect_len(name, p, 4)?;
                Self::RadialBlur {
                    amount: f(p[0]),
                    center_x: f(p[1]),
                    center_y: f(p[2]),
                    kind: if u(p[3]) == 1 {
                        RadialKind::Zoom
                    } else {
                        RadialKind::Spin
                    },
                }
            }
            "sharpen" => {
                expect_len(name, p, 0)?;
                Self::Sharpen
            }
            "unsharp_mask" => {
                expect_len(name, p, 3)?;
                Self::UnsharpMask {
                    amount: f(p[0]),
                    radius: f(p[1]),
                    threshold: f(p[2]),
                }
            }
            "emboss" => {
                expect_len(name, p, 3)?;
                Self::Emboss {
                    angle: f(p[0]),
                    elevation: f(p[1]),
                    relief: f(p[2]),
                }
            }
            "edge_detect" => {
                expect_len(name, p, 2)?;
                Self::EdgeDetect {
                    algorithm: match u(p[0]) {
                        1 => EdgeAlgorithm::Prewitt,
                        2 => EdgeAlgorithm::Laplacian,
                        _ => EdgeAlgorithm::Sobel,
                    },
                    amount: f(p[1]),
                }
            }
            "relief" => {
                expect_len(name, p, 2)?;
                Self::Relief {
                    angle: f(p[0]),
                    amount: f(p[1]),
                }
            }
            "add_noise" => {
                expect_len(name, p, 4)?;
                Self::AddNoise {
                    amount: u(p[0]),
                    noise_type: if u(p[1]) == 1 {
                        NoiseType::Gaussian
                    } else {
                        NoiseType::Uniform
                    },
                    channels: if u(p[2]) == 1 {
                        NoiseChannels::Monochromatic
                    } else {
                        NoiseChannels::Rgb
                    },
                    seed: u64::from(u(p[3])),
                }
            }
            "reduce_noise" => {
                expect_len(name, p, 1)?;
                Self::ReduceNoise { radius: u(p[0]) }
            }
            "brightness_contrast" => {
                expect_len(name, p, 3)?;
                Self::BrightnessContrast {
                    brightness: f(p[0]),
                    contrast: f(p[1]),
                    enhanced: b(p[2]),
                }
            }
            "hue_saturation" => {
                expect_len(name, p, 4)?;
                Self::HueSaturation {
                    hue: f(p[0]),
                    saturation: f(p[1]),
                    lightness: f(p[2]),
                    colorize: b(p[3]),
                }
            }
            "curves" => {
                if p.len() < 3 || !(p.len() - 1).is_multiple_of(2) {
                    return Err(EffectError::ParamCount {
                        effect: name.to_owned(),
                        expected: 3,
                        got: p.len(),
                    });
                }
                let channel = match u(p[0]) {
                    1 => CurveChannel::Red,
                    2 => CurveChannel::Green,
                    3 => CurveChannel::Blue,
                    4 => CurveChannel::Alpha,
                    _ => CurveChannel::Composite,
                };
                let points = p[1..]
                    .chunks_exact(2)
                    .map(|xy| [f(xy[0]), f(xy[1])])
                    .collect();
                Self::Curves { channel, points }
            }
            "levels" => {
                expect_len(name, p, 6)?;
                Self::Levels {
                    channel: match u(p[0]) {
                        1 => LevelsChannel::Red,
                        2 => LevelsChannel::Green,
                        3 => LevelsChannel::Blue,
                        _ => LevelsChannel::Composite,
                    },
                    in_black: f(p[1]),
                    in_white: f(p[2]),
                    gamma: f(p[3]),
                    out_black: f(p[4]),
                    out_white: f(p[5]),
                }
            }
            "color_balance" => {
                expect_len(name, p, 10)?;
                Self::ColorBalance {
                    shadows: [f(p[0]), f(p[1]), f(p[2])],
                    midtones: [f(p[3]), f(p[4]), f(p[5])],
                    highlights: [f(p[6]), f(p[7]), f(p[8])],
                    preserve_luminosity: b(p[9]),
                }
            }
            "invert" => {
                expect_len(name, p, 0)?;
                Self::Invert
            }
            "grayscale" => {
                if p.len() != 1 && p.len() != 4 {
                    return Err(EffectError::ParamCount {
                        effect: name.to_owned(),
                        expected: 4,
                        got: p.len(),
                    });
                }
                let method = match u(p[0]) {
                    1 => GrayscaleMethod::Average,
                    2 => GrayscaleMethod::Bt709,
                    3 => GrayscaleMethod::ChannelMixer,
                    _ => GrayscaleMethod::Luminosity,
                };
                let mixer = if p.len() == 4 {
                    [f(p[1]), f(p[2]), f(p[3])]
                } else {
                    [0.3, 0.59, 0.11]
                };
                Self::Grayscale { method, mixer }
            }
            "posterize" => {
                expect_len(name, p, 1)?;
                Self::Posterize { levels: u(p[0]) }
            }
            "threshold" => {
                expect_len(name, p, 1)?;
                Self::Threshold {
                    threshold: u(p[0]).min(255) as u8,
                }
            }
            other => return Err(EffectError::UnknownEffect(other.to_owned())),
        };
        Ok(spec)
    }

    /// Run the described effect over `src`, returning a same-sized buffer.
    pub fn run(&self, src: &EffectImage) -> EffectImage {
        match self {
            Self::GaussianBlur { radius } => GaussianBlur::new(*radius).apply(src),
            Self::BoxBlur { width, height } => BoxBlur::new(*width, *height).apply(src),
            Self::MotionBlur { distance, angle } => MotionBlur::new(*distance, *angle).apply(src),
            Self::RadialBlur {
                amount,
                center_x,
                center_y,
                kind,
            } => RadialBlur::new(*amount, *center_x, *center_y, *kind).apply(src),
            Self::Sharpen => Sharpen.apply(src),
            Self::UnsharpMask {
                amount,
                radius,
                threshold,
            } => UnsharpMask::new(*amount, *radius, *threshold).apply(src),
            Self::Emboss {
                angle,
                elevation,
                relief,
            } => Emboss::new(*angle, *elevation, *relief).apply(src),
            Self::EdgeDetect { algorithm, amount } => {
                EdgeDetect::new(*algorithm, *amount).apply(src)
            }
            Self::Relief { angle, amount } => Relief::new(*angle, *amount).apply(src),
            Self::AddNoise {
                amount,
                noise_type,
                channels,
                seed,
            } => AddNoise::new(*amount, *noise_type, *channels, *seed).apply(src),
            Self::ReduceNoise { radius } => ReduceNoise::new(*radius).apply(src),
            Self::BrightnessContrast {
                brightness,
                contrast,
                enhanced,
            } => {
                let bc = BrightnessContrast::new(*brightness, *contrast);
                let bc = if *enhanced { bc.enhanced() } else { bc };
                bc.apply(src)
            }
            Self::HueSaturation {
                hue,
                saturation,
                lightness,
                colorize,
            } => {
                let hs = HueSaturation::new(*hue, *saturation, *lightness);
                let hs = if *colorize { hs.colorize() } else { hs };
                hs.apply(src)
            }
            Self::Curves { channel, points } => Curves::new(*channel, points.clone()).apply(src),
            Self::Levels {
                channel,
                in_black,
                in_white,
                gamma,
                out_black,
                out_white,
            } => Levels::new(
                *channel, *in_black, *in_white, *gamma, *out_black, *out_white,
            )
            .apply(src),
            Self::ColorBalance {
                shadows,
                midtones,
                highlights,
                preserve_luminosity,
            } => {
                let cb = ColorBalance::new(*shadows, *midtones, *highlights);
                let cb = if *preserve_luminosity {
                    cb.preserve_luminosity()
                } else {
                    cb
                };
                cb.apply(src)
            }
            Self::Invert => Invert.apply(src),
            Self::Grayscale { method, mixer } => match method {
                GrayscaleMethod::ChannelMixer => Grayscale::mixer(*mixer).apply(src),
                m => Grayscale::new(*m).apply(src),
            },
            Self::Posterize { levels } => Posterize::new(*levels).apply(src),
            Self::Threshold { threshold } => Threshold::new(*threshold).apply(src),
        }
    }

    /// Human-readable name, suitable for a menu entry or an undo label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::GaussianBlur { .. } => "Gaussian Blur",
            Self::BoxBlur { .. } => "Box Blur",
            Self::MotionBlur { .. } => "Motion Blur",
            Self::RadialBlur { .. } => "Radial Blur",
            Self::Sharpen => "Sharpen",
            Self::UnsharpMask { .. } => "Unsharp Mask",
            Self::Emboss { .. } => "Emboss",
            Self::EdgeDetect { .. } => "Edge Detect",
            Self::Relief { .. } => "Relief",
            Self::AddNoise { .. } => "Add Noise",
            Self::ReduceNoise { .. } => "Reduce Noise",
            Self::BrightnessContrast { .. } => "Brightness / Contrast",
            Self::HueSaturation { .. } => "Hue / Saturation",
            Self::Curves { .. } => "Curves",
            Self::Levels { .. } => "Levels",
            Self::ColorBalance { .. } => "Color Balance",
            Self::Invert => "Invert Colors",
            Self::Grayscale { .. } => "Grayscale",
            Self::Posterize { .. } => "Posterize",
            Self::Threshold { .. } => "Threshold",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> EffectImage {
        let data = px.repeat((w * h) as usize);
        EffectImage::from_rgba8(w, h, data).unwrap()
    }

    #[test]
    fn from_params_every_name_parses_with_its_documented_arity() {
        let cases: &[(&str, &[f64])] = &[
            ("gaussian_blur", &[2.0]),
            ("box_blur", &[3.0, 3.0]),
            ("motion_blur", &[5.0, 45.0]),
            ("radial_blur", &[10.0, 0.5, 0.5, 1.0]),
            ("sharpen", &[]),
            ("unsharp_mask", &[50.0, 1.0, 0.0]),
            ("emboss", &[45.0, 45.0, 1.0]),
            ("edge_detect", &[2.0, 100.0]),
            ("relief", &[45.0, 1.0]),
            ("add_noise", &[10.0, 1.0, 1.0, 7.0]),
            ("reduce_noise", &[1.0]),
            ("brightness_contrast", &[10.0, 10.0, 1.0]),
            ("hue_saturation", &[10.0, 0.0, 0.0, 0.0]),
            ("curves", &[0.0, 0.0, 0.0, 255.0, 255.0]),
            ("levels", &[0.0, 0.0, 255.0, 1.0, 0.0, 255.0]),
            (
                "color_balance",
                &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            ),
            ("invert", &[]),
            ("grayscale", &[0.0]),
            ("grayscale", &[3.0, 0.5, 0.25, 0.25]),
            ("posterize", &[4.0]),
            ("threshold", &[128.0]),
        ];
        for (name, params) in cases {
            let spec =
                EffectSpec::from_params(name, params).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                EFFECT_NAMES.contains(name),
                "{name} missing from EFFECT_NAMES"
            );
            assert!(!spec.label().is_empty());
            // Every spec must run on a small image without panicking.
            let out = spec.run(&solid(4, 4, [200, 100, 50, 255]));
            assert_eq!((out.width(), out.height()), (4, 4));
        }
    }

    #[test]
    fn from_params_unknown_name_is_an_error() {
        assert_eq!(
            EffectSpec::from_params("sepia", &[]),
            Err(EffectError::UnknownEffect("sepia".into()))
        );
    }

    #[test]
    fn from_params_wrong_arity_is_an_error() {
        assert!(matches!(
            EffectSpec::from_params("gaussian_blur", &[]),
            Err(EffectError::ParamCount {
                expected: 1,
                got: 0,
                ..
            })
        ));
        assert!(matches!(
            EffectSpec::from_params("curves", &[0.0, 1.0]),
            Err(EffectError::ParamCount { .. })
        ));
    }

    #[test]
    fn run_invert_flips_rgb_and_keeps_alpha() {
        let spec = EffectSpec::from_params("invert", &[]).unwrap();
        let out = spec.run(&solid(1, 1, [10, 20, 30, 200]));
        assert_eq!(out.data(), &[245, 235, 225, 200]);
    }

    #[test]
    fn every_effect_name_is_accepted() {
        for name in EFFECT_NAMES {
            if let Err(err) = EffectSpec::from_params(name, &[]) {
                assert!(
                    !matches!(err, EffectError::UnknownEffect(_)),
                    "{name} rejected as unknown"
                );
            }
        }
    }
}
