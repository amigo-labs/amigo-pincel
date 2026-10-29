//! The effect catalog the agent exposes: named, ranged parameters per
//! `pincel_effects::EffectSpec` id, converted to the wire parameter vector
//! exactly as the editor's Effects / Adjust dialogs do
//! (`ui/src/lib/effects/catalog.ts`, `toParams`).

use serde_json::{Map, Value};

use crate::AgentError;

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
    pub params: &'static [Param],
    wire: Wire,
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

const fn plain(id: &'static str, params: &'static [Param]) -> EffectDef {
    EffectDef {
        id,
        params,
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
    plain("reduce_noise", &[num("radius", 1.0, 10.0, 1.0)]),
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
];

pub(crate) fn effect_ids() -> Vec<&'static str> {
    EFFECTS.iter().map(|e| e.id).collect()
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
/// every name and range.
pub(crate) fn wire_params(id: &str, params: &Map<String, Value>) -> Result<Vec<f64>, AgentError> {
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
        values.push(value_of(id, p, params.get(p.key))?);
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

fn value_of(id: &str, p: &Param, v: Option<&Value>) -> Result<f64, AgentError> {
    match p.kind {
        Kind::Number { min, max, default } => {
            let Some(v) = v else {
                return Ok(default);
            };
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
        Kind::Bool { default } => match v {
            None => Ok(f64::from(u8::from(default))),
            Some(Value::Bool(b)) => Ok(f64::from(u8::from(*b))),
            Some(_) => Err(AgentError::tool(format!(
                "parameter `{}` of `{id}` must be true or false",
                p.key
            ))),
        },
        Kind::Choice { options, default } => {
            let idx = match v {
                None => Some(default),
                Some(Value::String(s)) => options.iter().position(|o| o == s),
                Some(v) => v
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

    #[test]
    fn catalog_covers_every_effect_in_menu_order() {
        assert_eq!(effect_ids(), EFFECT_NAMES);
    }

    #[test]
    fn wire_params_defaults_build_valid_specs_for_every_effect() {
        for e in EFFECTS {
            let wire = wire_params(e.id, &Map::new()).expect("defaults are in range");
            pincel_effects::EffectSpec::from_params(e.id, &wire).expect("spec accepts defaults");
        }
    }

    #[test]
    fn wire_params_out_of_range_names_parameter_and_range() {
        let err = wire_params("gaussian_blur", &map(json!({ "radius": 300 }))).unwrap_err();
        assert_eq!(
            err,
            AgentError::tool("parameter `radius` of `gaussian_blur` must be 0.1–250, got 300")
        );
    }

    #[test]
    fn wire_params_levels_scales_points_like_the_editor() {
        let w = wire_params("levels", &map(json!({ "in_white": 51, "gamma": 2 }))).expect("ok");
        assert_eq!(w, vec![0.0, 0.0, 0.2, 2.0, 0.0, 1.0]);
    }

    #[test]
    fn wire_params_choice_accepts_name_or_index() {
        let a = wire_params("radial_blur", &map(json!({ "kind": "zoom" }))).expect("ok");
        let b = wire_params("radial_blur", &map(json!({ "kind": 1 }))).expect("ok");
        assert_eq!(a, b);
        assert_eq!(a[3], 1.0);
    }
}
