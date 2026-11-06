//! The effect catalog the agent exposes: named, ranged parameters per
//! `pincel_effects::EffectSpec` id, converted to the wire parameter vector
//! exactly as the editor's Effects / Adjust dialogs do
//! (`ui/src/lib/effects/catalog.ts`, `toParams`).

use serde_json::{Map, Value};

use crate::AgentError;
use crate::args::parse_hex;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Kind {
    Number {
        min: f64,
        max: f64,
        default: f64,
    },
    Bool {
        default: bool,
    },
    Choice {
        options: &'static [&'static str],
        default: usize,
    },
    /// A colour as `#RRGGBB` / `#RRGGBBAA`; the wire carries it as one
    /// `0xRRGGBBAA` slot (Decision Log 2026-10-29).
    Color {
        default: u32,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Param {
    pub key: &'static str,
    pub kind: Kind,
}

/// How the named values become the wire vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wire {
    /// The parameters in declared order.
    Plain,
    /// `[channel, x0/255, y0/255, …]` at the fixed curve x positions.
    Curves,
    /// `[channel, in_black/255, in_white/255, gamma, out_black/255, out_white/255]`.
    Levels,
}

pub(crate) struct EffectDef {
    pub id: &'static str,
    /// The parameters with the Image editor's defaults (the catalog's).
    pub params: &'static [Param],
    /// Pixel-mode default overrides by parameter (the catalog's
    /// `modeDefaults.pixel`); a choice's value is its option index.
    pub pixel: &'static [(&'static str, f64)],
    wire: Wire,
}

impl EffectDef {
    /// The default of parameter `p` in a document of the given mode, as a
    /// wire number (choice index, 0 / 1 for a flag, packed RGBA).
    pub(crate) fn default_of(&self, p: &Param, image: bool) -> f64 {
        let over = (!image)
            .then(|| self.pixel.iter().find(|(k, _)| *k == p.key))
            .flatten();
        match (over, p.kind) {
            (Some((_, v)), _) => *v,
            (None, Kind::Number { default, .. }) => default,
            (None, Kind::Bool { default }) => f64::from(u8::from(default)),
            (None, Kind::Choice { default, .. }) => default as f64,
            (None, Kind::Color { default }) => f64::from(default),
        }
    }

    /// The same definition with Pixel-mode default overrides.
    const fn with_pixel(self, pixel: &'static [(&'static str, f64)]) -> Self {
        EffectDef { pixel, ..self }
    }
}

const fn num(key: &'static str, min: f64, max: f64, default: f64) -> Param {
    Param {
        key,
        kind: Kind::Number { min, max, default },
    }
}

const fn flag(key: &'static str, default: bool) -> Param {
    Param {
        key,
        kind: Kind::Bool { default },
    }
}

const fn choice(key: &'static str, options: &'static [&'static str]) -> Param {
    Param {
        key,
        kind: Kind::Choice {
            options,
            default: 0,
        },
    }
}

const fn color(key: &'static str, default: u32) -> Param {
    Param {
        key,
        kind: Kind::Color { default },
    }
}

/// A choice whose default is not the first option.
const fn choice_at(key: &'static str, options: &'static [&'static str], default: usize) -> Param {
    Param {
        key,
        kind: Kind::Choice { options, default },
    }
}

/// The edge behaviour of the resampling distortions (effects-distort).
const EDGES: &[&str] = &["clamp", "wrap", "reflect", "transparent"];

const fn plain(id: &'static str, params: &'static [Param]) -> EffectDef {
    EffectDef {
        id,
        params,
        pixel: &[],
        wire: Wire::Plain,
    }
}

const CURVE_XS: [f64; 5] = [0.0, 64.0, 128.0, 192.0, 255.0];

/// Every effect, in menu order (mirrors `EFFECT_NAMES`).
pub(crate) static EFFECTS: &[EffectDef] = &[
    plain("gaussian_blur", &[num("radius", 0.1, 250.0, 2.0)]),
    plain(
        "box_blur",
        &[
            num("width", 1.0, 251.0, 3.0),
            num("height", 1.0, 251.0, 3.0),
        ],
    ),
    plain(
        "motion_blur",
        &[
            num("distance", 1.0, 100.0, 6.0),
            num("angle", 0.0, 360.0, 0.0),
            flag("centered", true),
        ],
    ),
    plain(
        "radial_blur",
        &[
            num("amount", 1.0, 100.0, 10.0),
            num("center_x", 0.0, 1.0, 0.5),
            num("center_y", 0.0, 1.0, 0.5),
            choice("kind", &["spin", "zoom"]),
        ],
    ),
    plain("sharpen", &[]),
    plain(
        "unsharp_mask",
        &[
            num("amount", 1.0, 500.0, 50.0),
            num("radius", 0.1, 50.0, 1.0),
            num("threshold", 0.0, 255.0, 0.0),
        ],
    ),
    plain(
        "emboss",
        &[
            num("angle", 0.0, 360.0, 45.0),
            num("elevation", 0.0, 90.0, 45.0),
            num("relief", 0.1, 10.0, 1.0),
        ],
    ),
    plain(
        "edge_detect",
        &[
            choice("algorithm", &["sobel", "prewitt", "laplacian"]),
            num("amount", 1.0, 400.0, 100.0),
        ],
    ),
    plain(
        "relief",
        &[
            num("angle", 0.0, 360.0, 45.0),
            num("amount", 0.1, 10.0, 1.0),
        ],
    ),
    plain(
        "add_noise",
        &[
            num("amount", 1.0, 100.0, 20.0),
            choice("noise_type", &["uniform", "gaussian"]),
            choice("channels", &["rgb", "monochromatic"]),
            num("seed", 0.0, 9999.0, 1.0),
        ],
    ),
    plain(
        "reduce_noise",
        &[
            num("radius", 1.0, 200.0, 6.0),
            num("strength", 0.0, 100.0, 40.0),
        ],
    ),
    plain(
        "brightness_contrast",
        &[
            num("brightness", -150.0, 150.0, 0.0),
            num("contrast", -150.0, 150.0, 0.0),
            flag("enhanced", false),
        ],
    ),
    plain(
        "hue_saturation",
        &[
            num("hue", -180.0, 180.0, 0.0),
            num("saturation", -100.0, 100.0, 0.0),
            num("lightness", -100.0, 100.0, 0.0),
            flag("colorize", false),
        ],
    ),
    EffectDef {
        id: "curves",
        params: &[
            choice("channel", &["composite", "red", "green", "blue", "alpha"]),
            num("y0", 0.0, 255.0, 0.0),
            num("y64", 0.0, 255.0, 64.0),
            num("y128", 0.0, 255.0, 128.0),
            num("y192", 0.0, 255.0, 192.0),
            num("y255", 0.0, 255.0, 255.0),
        ],
        pixel: &[],
        wire: Wire::Curves,
    },
    EffectDef {
        id: "levels",
        params: &[
            choice("channel", &["composite", "red", "green", "blue"]),
            num("in_black", 0.0, 255.0, 0.0),
            num("in_white", 0.0, 255.0, 255.0),
            num("gamma", 0.1, 10.0, 1.0),
            num("out_black", 0.0, 255.0, 0.0),
            num("out_white", 0.0, 255.0, 255.0),
        ],
        pixel: &[],
        wire: Wire::Levels,
    },
    plain(
        "color_balance",
        &[
            num("s_cr", -100.0, 100.0, 0.0),
            num("s_mg", -100.0, 100.0, 0.0),
            num("s_yb", -100.0, 100.0, 0.0),
            num("m_cr", -100.0, 100.0, 0.0),
            num("m_mg", -100.0, 100.0, 0.0),
            num("m_yb", -100.0, 100.0, 0.0),
            num("h_cr", -100.0, 100.0, 0.0),
            num("h_mg", -100.0, 100.0, 0.0),
            num("h_yb", -100.0, 100.0, 0.0),
            flag("preserve_luminosity", true),
        ],
    ),
    plain("invert", &[]),
    plain(
        "grayscale",
        &[
            choice(
                "method",
                &["luminosity", "average", "bt709", "channel_mixer"],
            ),
            num("r", 0.0, 1.0, 0.3),
            num("g", 0.0, 1.0, 0.59),
            num("b", 0.0, 1.0, 0.11),
        ],
    ),
    plain("posterize", &[num("levels", 2.0, 64.0, 4.0)]),
    plain("threshold", &[num("threshold", 0.0, 255.0, 128.0)]),
    plain("auto_level", &[]),
    plain("exposure", &[num("exposure", -5.0, 5.0, 0.0)]),
    plain(
        "highlights_shadows",
        &[
            num("shadows", -100.0, 100.0, 0.0),
            num("highlights", -100.0, 100.0, 0.0),
        ],
    ),
    plain("invert_alpha", &[]),
    plain("sepia", &[num("intensity", 0.0, 100.0, 100.0)]),
    plain(
        "temperature_tint",
        &[
            num("temperature", -100.0, 100.0, 0.0),
            num("tint", -100.0, 100.0, 0.0),
        ],
    ),
    // docs/specs/effects-blurs.md: paint.net's remaining blurs.
    plain(
        "bokeh",
        &[
            num("radius", 1.0, 100.0, 10.0),
            num("highlights", 0.0, 100.0, 25.0),
        ],
    ),
    plain(
        "fragment",
        &[
            num("fragments", 2.0, 50.0, 4.0),
            num("distance", 0.0, 100.0, 8.0),
            num("rotation", 0.0, 360.0, 0.0),
        ],
    ),
    plain(
        "median",
        &[
            num("radius", 1.0, 200.0, 10.0),
            num("percentile", 0.0, 100.0, 50.0),
        ],
    ),
    plain(
        "sketch_blur",
        &[
            num("radius", 1.0, 100.0, 10.0),
            num("seed", 0.0, 9999.0, 1.0),
        ],
    ),
    plain(
        "surface_blur",
        &[
            num("radius", 1.0, 100.0, 6.0),
            num("threshold", 1.0, 100.0, 25.0),
        ],
    ),
    plain(
        "zoom_blur",
        &[
            num("amount", 1.0, 100.0, 10.0),
            num("center_x", 0.0, 1.0, 0.5),
            num("center_y", 0.0, 1.0, 0.5),
        ],
    ),
    // docs/specs/outline-effect.md: Object ▸ Outline Object.
    plain(
        "outline",
        &[
            choice("place", &["outside", "inside", "both"]),
            Param {
                key: "shape",
                kind: Kind::Choice {
                    options: &["circle", "square"],
                    default: 1,
                },
            },
            num("thickness", 1.0, 8.0, 1.0),
            color("color", 0x0000_00FF),
        ],
    ),
    // docs/specs/effects-distort.md: paint.net's Distort menu.
    plain(
        "bulge",
        &[
            num("amount", -200.0, 100.0, 45.0),
            num("center_x", 0.0, 1.0, 0.5),
            num("center_y", 0.0, 1.0, 0.5),
            choice("edge", EDGES),
        ],
    ),
    plain(
        "crystalize",
        &[
            num("cell_size", 2.0, 250.0, 8.0),
            num("seed", 0.0, 9999.0, 1.0),
        ],
    ),
    plain(
        "dents",
        &[
            num("scale", 1.0, 200.0, 25.0),
            num("refraction", 0.0, 200.0, 50.0),
            num("roughness", 0.0, 100.0, 10.0),
            num("seed", 0.0, 9999.0, 1.0),
            choice("edge", EDGES),
        ],
    ),
    plain(
        "frosted_glass",
        &[
            num("amount", 1.0, 200.0, 10.0),
            num("seed", 0.0, 9999.0, 1.0),
        ],
    ),
    plain(
        "morphology",
        &[
            choice("operation", &["dilate", "erode", "open", "close"]),
            choice("shape", &["square", "circle"]),
            num("radius", 1.0, 100.0, 1.0),
        ],
    ),
    plain(
        "pixelate",
        &[
            num("cell_size", 2.0, 200.0, 8.0),
            choice("color", &["average", "centre"]),
        ],
    )
    // Pixel mode samples the centre pixel, which adds no colour.
    .with_pixel(&[("color", 1.0)]),
    plain(
        "polar_inversion",
        &[
            num("amount", -4.0, 4.0, 1.0),
            num("center_x", 0.0, 1.0, 0.5),
            num("center_y", 0.0, 1.0, 0.5),
            choice_at("edge", EDGES, 1),
        ],
    ),
    plain(
        "tile_reflection",
        &[
            num("tile_size", 2.0, 200.0, 40.0),
            num("angle", 0.0, 360.0, 30.0),
            num("curvature", -100.0, 100.0, 8.0),
            choice("edge", EDGES),
        ],
    ),
    plain(
        "twist",
        &[
            num("amount", -100.0, 100.0, 30.0),
            num("size", 1.0, 200.0, 100.0),
            num("center_x", 0.0, 1.0, 0.5),
            num("center_y", 0.0, 1.0, 0.5),
            choice("edge", EDGES),
        ],
    ),
    // docs/specs/effects-object-stylize-color-noise.md: Object ▸ Drop
    // Shadow (Pixel mode's Blur 0), Stylize ▸ Outline, Color ▸ Quantize.
    plain(
        "drop_shadow",
        &[
            num("offset_x", -100.0, 100.0, 4.0),
            num("offset_y", -100.0, 100.0, 4.0),
            num("blur", 0.0, 50.0, 4.0),
            color("color", 0x0000_00FF),
            num("opacity", 0.0, 100.0, 50.0),
        ],
    )
    // Pixel mode keeps the shadow hard.
    .with_pixel(&[("blur", 0.0)]),
    plain(
        "stylize_outline",
        &[
            num("thickness", 1.0, 200.0, 3.0),
            num("intensity", 0.0, 100.0, 50.0),
        ],
    ),
    plain(
        "quantize",
        &[
            num("colors", 2.0, 256.0, 16.0),
            choice("algorithm", &["median_cut", "octree"]),
            num("dithering", 0.0, 100.0, 0.0),
        ],
    ),
    // docs/specs/effects-artistic-photo.md: paint.net's Artistic and Photo
    // menus.
    plain(
        "ink_sketch",
        &[
            num("ink_outline", 0.0, 99.0, 50.0),
            num("coloring", 0.0, 100.0, 50.0),
        ],
    ),
    plain(
        "oil_painting",
        &[
            num("brush_size", 1.0, 8.0, 3.0),
            num("coarseness", 3.0, 255.0, 50.0),
        ],
    ),
    plain(
        "pencil_sketch",
        &[
            num("tip_size", 1.0, 20.0, 2.0),
            num("color_range", -20.0, 20.0, 0.0),
        ],
    ),
    plain(
        "glow",
        &[
            num("radius", 1.0, 20.0, 6.0),
            num("brightness", -100.0, 100.0, 10.0),
            num("contrast", -100.0, 100.0, 10.0),
        ],
    ),
    plain(
        "red_eye_removal",
        &[
            num("tolerance", 0.0, 100.0, 70.0),
            num("saturation", 0.0, 100.0, 90.0),
        ],
    ),
    plain(
        "soften_portrait",
        &[
            num("softness", 0.0, 10.0, 5.0),
            num("lighting", -20.0, 20.0, 0.0),
            num("warmth", 0.0, 20.0, 10.0),
        ],
    ),
    plain(
        "straighten",
        &[
            num("angle", -45.0, 45.0, 0.0),
            choice("fill", &["zoom_to_fill", "transparent_corners"]),
        ],
    ),
    plain(
        "vignette",
        &[
            num("center_x", 0.0, 1.0, 0.5),
            num("center_y", 0.0, 1.0, 0.5),
            num("radius", 10.0, 400.0, 100.0),
            num("density", 0.0, 100.0, 100.0),
            choice("color", &["black", "white"]),
        ],
    ),
    // docs/specs/effects-render.md: paint.net's Render menu. The editor's
    // colours default to its foreground / background; agents get black /
    // white.
    plain(
        "clouds",
        &[
            num("scale", 2.0, 1000.0, 250.0),
            num("roughness", 0.0, 100.0, 50.0),
            num("seed", 0.0, 9999.0, 1.0),
            color("color1", 0x0000_00FF),
            color("color2", 0xFFFF_FFFF),
            choice("blend", BLENDS),
        ],
    ),
    plain(
        "turbulence",
        &[
            num("scale", 2.0, 1000.0, 200.0),
            num("octaves", 1.0, 8.0, 5.0),
            num("seed", 0.0, 9999.0, 1.0),
            color("color1", 0x0000_00FF),
            color("color2", 0xFFFF_FFFF),
            choice("blend", BLENDS),
        ],
    ),
    plain(
        "julia_fractal",
        &[
            num("factor", 1.0, 10.0, 4.0),
            num("zoom", 1.0, 50.0, 1.0),
            num("angle", 0.0, 360.0, 0.0),
            num("quality", 1.0, 5.0, 2.0),
        ],
    ),
    plain(
        "mandelbrot_fractal",
        &[
            num("factor", 1.0, 10.0, 1.0),
            num("zoom", 1.0, 100.0, 10.0),
            num("angle", 0.0, 360.0, 0.0),
            num("quality", 1.0, 5.0, 2.0),
            flag("invert", false),
        ],
    ),
];

/// The blend modes of Clouds and Turbulence (effects-render).
const BLENDS: &[&str] = &["normal", "multiply", "screen", "overlay"];

pub(crate) fn effect_ids() -> Vec<&'static str> {
    EFFECTS.iter().map(|e| e.id).collect()
}

/// The ids of the editor's Adjust menu (`group: 'adjust'` in
/// `ui/src/lib/effects/catalog.ts`); every other id is an effect.
pub(crate) const ADJUSTMENTS: &[&str] = &[
    "brightness_contrast",
    "hue_saturation",
    "curves",
    "levels",
    "color_balance",
    "invert",
    "grayscale",
    "posterize",
    "threshold",
    "auto_level",
    "exposure",
    "highlights_shadows",
    "invert_alpha",
    "sepia",
    "temperature_tint",
];

/// `true` for an adjustment id (Adjust menu), `false` for an effect.
pub(crate) fn is_adjustment(id: &str) -> bool {
    ADJUSTMENTS.contains(&id)
}

/// One-line parameter summary per effect, for the tool description.
pub(crate) fn catalog_summary() -> String {
    EFFECTS
        .iter()
        .map(|e| {
            let params: Vec<String> = e
                .params
                .iter()
                .map(|p| match p.kind {
                    Kind::Number { min, max, default } => {
                        format!("{} {min}–{max} (default {default})", p.key)
                    }
                    Kind::Bool { default } => format!("{} bool (default {default})", p.key),
                    Kind::Choice { options, .. } => format!("{} {}", p.key, options.join("|")),
                    Kind::Color { default } => {
                        format!("{} #RRGGBB[AA] (default #{default:08X})", p.key)
                    }
                })
                .collect();
            if params.is_empty() {
                e.id.to_owned()
            } else {
                format!("{}: {}", e.id, params.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Build the wire vector for effect `id` from named `params`, checking
/// every name and range; omitted parameters take the defaults of the
/// editor of the document's mode (`image`; mcp-parity-drawing-image
/// AC-09 … AC-11).
pub(crate) fn wire_params(
    id: &str,
    params: &Map<String, Value>,
    image: bool,
) -> Result<Vec<f64>, AgentError> {
    let def = EFFECTS.iter().find(|e| e.id == id).ok_or_else(|| {
        AgentError::tool(format!(
            "unknown effect id `{id}`; known ids: {}",
            effect_ids().join(", ")
        ))
    })?;
    for key in params.keys() {
        if !def.params.iter().any(|p| p.key == key) {
            let names: Vec<&str> = def.params.iter().map(|p| p.key).collect();
            return Err(AgentError::tool(format!(
                "effect `{id}` has no parameter `{key}` (parameters: {})",
                if names.is_empty() {
                    "none".to_owned()
                } else {
                    names.join(", ")
                }
            )));
        }
    }
    let mut values = Vec::with_capacity(def.params.len());
    for p in def.params {
        let value = match params.get(p.key) {
            Some(v) => value_of(id, p, v)?,
            None => def.default_of(p, image),
        };
        values.push(value);
    }
    Ok(match def.wire {
        Wire::Plain => values,
        Wire::Curves => {
            let mut out = vec![values[0]];
            for (x, y) in CURVE_XS.iter().zip(&values[1..]) {
                out.push(x / 255.0);
                out.push(y / 255.0);
            }
            out
        }
        Wire::Levels => vec![
            values[0],
            values[1] / 255.0,
            values[2] / 255.0,
            values[3],
            values[4] / 255.0,
            values[5] / 255.0,
        ],
    })
}

fn value_of(id: &str, p: &Param, v: &Value) -> Result<f64, AgentError> {
    match p.kind {
        Kind::Number { min, max, .. } => {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| {
                AgentError::tool(format!(
                    "parameter `{}` of `{id}` must be a number {min}–{max}",
                    p.key
                ))
            })?;
            if n < min || n > max {
                return Err(AgentError::tool(format!(
                    "parameter `{}` of `{id}` must be {min}–{max}, got {n}",
                    p.key
                )));
            }
            Ok(n)
        }
        Kind::Bool { .. } => match v {
            Value::Bool(b) => Ok(f64::from(u8::from(*b))),
            _ => Err(AgentError::tool(format!(
                "parameter `{}` of `{id}` must be true or false",
                p.key
            ))),
        },
        Kind::Choice { options, .. } => {
            let idx = match v {
                Value::String(s) => options.iter().position(|o| o == s),
                v => v
                    .as_u64()
                    .and_then(|i| usize::try_from(i).ok())
                    .filter(|&i| i < options.len()),
            };
            idx.map(|i| i as f64).ok_or_else(|| {
                AgentError::tool(format!(
                    "parameter `{}` of `{id}` must be one of: {}",
                    p.key,
                    options.join(", ")
                ))
            })
        }
        Kind::Color { .. } => {
            let rgba = match v {
                Value::String(s) => parse_hex(s).map(|c| c.to_u32()),
                _ => None,
            };
            rgba.map(f64::from).ok_or_else(|| {
                AgentError::tool(format!(
                    "parameter `{}` of `{id}` must be a colour #RRGGBB or #RRGGBBAA",
                    p.key
                ))
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pincel_effects::EFFECT_NAMES;
    use serde_json::json;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap_or_default()
    }

    /// The default wire vector of every effect per mode, colour slots
    /// `null` (mcp-parity-drawing-image AC-11). `ui/e2e/effect-defaults.spec.ts`
    /// checks the editor's catalog against the committed copy.
    fn default_wires() -> String {
        let mut out = serde_json::Map::new();
        for e in EFFECTS {
            let mut modes = serde_json::Map::new();
            for (mode, image) in [("pixel", false), ("image", true)] {
                let wire = wire_params(e.id, &Map::new(), image).expect("defaults");
                let plain = e.wire == Wire::Plain;
                let slots: Vec<Value> = wire
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let colour = plain
                            && e.params
                                .get(i)
                                .is_some_and(|p| matches!(p.kind, Kind::Color { .. }));
                        if colour { Value::Null } else { json!(v) }
                    })
                    .collect();
                modes.insert(mode.to_owned(), Value::Array(slots));
            }
            out.insert(e.id.to_owned(), Value::Object(modes));
        }
        let mut text = serde_json::to_string_pretty(&Value::Object(out)).expect("json");
        text.push('\n');
        text
    }

    #[test]
    fn effect_defaults_file_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/effect-defaults.json");
        let want = default_wires();
        let have = std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        if have != want {
            std::fs::write(path, &want).expect("write effect-defaults.json");
            panic!("{path} was stale and has been regenerated; commit it");
        }
    }

    #[test]
    fn pixel_and_image_defaults_differ_where_the_editors_do() {
        let pixelate = |image| wire_params("pixelate", &Map::new(), image).expect("ok");
        assert_eq!(pixelate(true), vec![8.0, 0.0], "Image: average colour");
        assert_eq!(pixelate(false), vec![8.0, 1.0], "Pixel: centre pixel");
        let blur = |image| wire_params("drop_shadow", &Map::new(), image).expect("ok")[2];
        assert_eq!((blur(true), blur(false)), (4.0, 0.0));
    }

    #[test]
    fn catalog_covers_every_effect_in_menu_order() {
        assert_eq!(effect_ids(), EFFECT_NAMES);
    }

    #[test]
    fn wire_params_defaults_build_valid_specs_for_every_effect() {
        for e in EFFECTS {
            let wire = wire_params(e.id, &Map::new(), true).expect("defaults are in range");
            pincel_effects::EffectSpec::from_params(e.id, &wire).expect("spec accepts defaults");
        }
    }

    #[test]
    fn wire_params_out_of_range_names_parameter_and_range() {
        let err = wire_params("gaussian_blur", &map(json!({ "radius": 300 })), true).unwrap_err();
        assert_eq!(
            err,
            AgentError::tool("parameter `radius` of `gaussian_blur` must be 0.1–250, got 300")
        );
    }

    #[test]
    fn wire_params_levels_scales_points_like_the_editor() {
        let w =
            wire_params("levels", &map(json!({ "in_white": 51, "gamma": 2 })), true).expect("ok");
        assert_eq!(w, vec![0.0, 0.0, 0.2, 2.0, 0.0, 1.0]);
    }

    #[test]
    fn wire_params_choice_accepts_name_or_index() {
        let a = wire_params("radial_blur", &map(json!({ "kind": "zoom" })), true).expect("ok");
        let b = wire_params("radial_blur", &map(json!({ "kind": 1 })), true).expect("ok");
        assert_eq!(a, b);
        assert_eq!(a[3], 1.0);
    }

    /// outline-effect: the colour parameter takes `#RRGGBB[AA]` and travels
    /// as one `0xRRGGBBAA` slot, like the editor's colour control.
    #[test]
    fn wire_params_outline_colour_is_one_rrggbbaa_slot() {
        let d = wire_params("outline", &Map::new(), true).expect("defaults");
        assert_eq!(d, vec![0.0, 1.0, 1.0, f64::from(0x0000_00FF_u32)]);
        let w = wire_params(
            "outline",
            &map(
                json!({ "place": "both", "shape": "circle", "thickness": 3, "color": "#FF801080" }),
            ),
            true,
        )
        .expect("ok");
        assert_eq!(w, vec![2.0, 0.0, 3.0, f64::from(0xFF80_1080_u32)]);
        let opaque = wire_params("outline", &map(json!({ "color": "#ff8010" })), true).expect("ok");
        assert_eq!(opaque[3], f64::from(0xFF80_10FF_u32));
        for bad in [json!("red"), json!("#ff80"), json!(255)] {
            let err = wire_params("outline", &map(json!({ "color": bad })), true).unwrap_err();
            assert_eq!(
                err,
                AgentError::tool(
                    "parameter `color` of `outline` must be a colour #RRGGBB or #RRGGBBAA"
                )
            );
        }
        assert!(wire_params("outline", &map(json!({ "thickness": 9 })), true).is_err());
    }
}
