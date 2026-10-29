//! Read-only MCP resources: the agent guide (`pincel://guide`), the effect
//! catalog (`pincel://effects`) and the built-in palette presets as
//! `pincel://palettes/{id}`.

use pincel_core::{Palette, PalettePreset};
use serde_json::{Value, json};

use crate::AgentError;
use crate::args::hex;
use crate::pixel::effects::{EFFECTS, Kind, is_adjustment};

const PALETTE_PREFIX: &str = "pincel://palettes/";
const GUIDE_URI: &str = "pincel://guide";
const EFFECTS_URI: &str = "pincel://effects";
/// The usage guide for agents, both modes (AC-17).
const GUIDE: &str = include_str!("guide.md");

/// The MCP description of one resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDef {
    /// `pincel://…` URI.
    pub uri: String,
    /// Short name.
    pub name: String,
    /// What the resource holds.
    pub description: String,
    /// MIME type of [`read_resource`]'s text.
    pub mime_type: &'static str,
}

/// Display name of a preset.
pub(crate) fn preset_title(p: PalettePreset) -> &'static str {
    match p {
        PalettePreset::Db32 => "DawnBringer 32",
        PalettePreset::Db16 => "DawnBringer 16",
        PalettePreset::Pico8 => "PICO-8",
        PalettePreset::GameBoy => "Game Boy",
        PalettePreset::Nes => "NES",
    }
}

/// Every resource: the agent guide, the effect catalog and one per palette
/// preset.
pub fn list_resources() -> Vec<ResourceDef> {
    let mut list = vec![
        ResourceDef {
            uri: GUIDE_URI.to_owned(),
            name: "guide".to_owned(),
            description: "How to use Pincel as an agent: modes and their tools, workflow (plan → draw → preview → export), coordinates, colors, undo, preview and common pitfalls".to_owned(),
            mime_type: "text/markdown",
        },
        ResourceDef {
            uri: EFFECTS_URI.to_owned(),
            name: "effects".to_owned(),
            description: "Every effect and adjustment id effect.apply / adjust.apply accept, with its parameters, ranges and defaults".to_owned(),
            mime_type: "application/json",
        },
    ];
    list.extend(PalettePreset::ALL.iter().map(|&p| ResourceDef {
        uri: format!("{PALETTE_PREFIX}{}", p.name()),
        name: format!("palette {}", p.name()),
        description: format!(
            "{} palette preset ({} colors) as #RRGGBBAA strings",
            preset_title(p),
            Palette::preset(p).len()
        ),
        mime_type: "application/json",
    }));
    list
}

/// The text of the resource at `uri` (its MIME type is in
/// [`list_resources`]).
pub fn read_resource(uri: &str) -> Result<String, AgentError> {
    match uri {
        GUIDE_URI => return Ok(GUIDE.to_owned()),
        EFFECTS_URI => return Ok(effects_json().to_string()),
        _ => {}
    }
    let preset = uri
        .strip_prefix(PALETTE_PREFIX)
        .and_then(PalettePreset::from_name)
        .ok_or_else(|| AgentError::tool(format!("unknown resource `{uri}`")))?;
    Ok(palette_json(preset).to_string())
}

/// The effect catalog, generated from the table the tools validate against
/// (AC-18).
fn effects_json() -> Value {
    let effects: Vec<Value> = EFFECTS
        .iter()
        .map(|e| {
            let params: Vec<Value> = e
                .params
                .iter()
                .map(|p| match p.kind {
                    Kind::Number { min, max, default } => json!({
                        "name": p.key, "type": "number", "min": min, "max": max, "default": default,
                    }),
                    Kind::Bool { default } => {
                        json!({ "name": p.key, "type": "boolean", "default": default })
                    }
                    Kind::Choice { options, default } => json!({
                        "name": p.key, "type": "choice", "options": options, "default": options[default],
                    }),
                })
                .collect();
            json!({
                "id": e.id,
                "kind": if is_adjustment(e.id) { "adjustment" } else { "effect" },
                "tool": if is_adjustment(e.id) { "adjust.apply or effect.apply" } else { "effect.apply" },
                "params": params,
            })
        })
        .collect();
    json!({ "effects": effects })
}

fn palette_json(p: PalettePreset) -> Value {
    let colors: Vec<String> = Palette::preset(p)
        .colors
        .iter()
        .map(|e| hex(e.rgba))
        .collect();
    json!({ "id": p.name(), "name": preset_title(p), "colors": colors })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_resources_has_one_uri_per_preset() {
        let list = list_resources();
        let palettes = list
            .iter()
            .filter(|r| r.uri.starts_with(PALETTE_PREFIX))
            .count();
        assert_eq!(palettes, PalettePreset::ALL.len());
        assert!(list.iter().any(|r| r.uri == "pincel://palettes/pico8"));
    }

    #[test]
    fn list_resources_has_guide_and_effects() {
        let list = list_resources();
        let find = |uri: &str| list.iter().find(|r| r.uri == uri).expect(uri);
        assert_eq!(find("pincel://guide").mime_type, "text/markdown");
        assert_eq!(find("pincel://effects").mime_type, "application/json");
    }

    #[test]
    fn read_resource_guide_covers_workflow_and_both_modes() {
        let guide = read_resource("pincel://guide").expect("guide");
        for heading in [
            "## Workflow",
            "## Coordinates",
            "## Colors",
            "## Undo",
            "## Preview",
            "## Common pitfalls",
        ] {
            assert!(guide.contains(heading), "missing {heading}");
        }
        for word in ["plan", "draw", "doc.preview", "export", "Pixel", "Image"] {
            assert!(guide.contains(word), "missing {word}");
        }
        // Every tool the guide names in backticks exists.
        let tools: Vec<&str> = crate::list_tools().iter().map(|t| t.name).collect();
        for part in guide.split('`').skip(1).step_by(2) {
            let looks_like_tool = part.contains('.')
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')
                && !part.starts_with('.');
            if looks_like_tool && !part.ends_with(".ora") {
                assert!(tools.contains(&part), "guide names unknown tool `{part}`");
            }
        }
    }

    #[test]
    fn effects_resource_lists_exactly_the_catalog_with_ranges() {
        let text = read_resource("pincel://effects").expect("effects");
        let v: Value = serde_json::from_str(&text).expect("json");
        let entries = v["effects"].as_array().expect("effects array");
        let ids: Vec<&str> = entries.iter().filter_map(|e| e["id"].as_str()).collect();
        assert_eq!(ids, pincel_effects::EFFECT_NAMES);
        let blur = &entries[0];
        assert_eq!(blur["id"], "gaussian_blur");
        assert_eq!(blur["kind"], "effect");
        assert_eq!(
            blur["params"][0],
            json!({ "name": "radius", "type": "number", "min": 0.1, "max": 250.0, "default": 2.0 })
        );
        let levels = entries
            .iter()
            .find(|e| e["id"] == "levels")
            .expect("levels");
        assert_eq!(levels["kind"], "adjustment");
        assert_eq!(
            levels["params"][0]["options"],
            json!(["composite", "red", "green", "blue"])
        );
        // Generated from the catalog: every parameter is there.
        for (entry, def) in entries.iter().zip(crate::pixel::effects::EFFECTS) {
            assert_eq!(
                entry["params"].as_array().map(Vec::len),
                Some(def.params.len()),
                "{}",
                def.id
            );
        }
    }

    #[test]
    fn read_resource_returns_the_preset_colors() {
        let text = read_resource("pincel://palettes/gameboy").expect("read");
        let v: Value = serde_json::from_str(&text).expect("json");
        let expected: Vec<String> = Palette::preset(PalettePreset::GameBoy)
            .colors
            .iter()
            .map(|e| hex(e.rgba))
            .collect();
        assert_eq!(v["colors"], json!(expected));
        assert!(read_resource("pincel://palettes/vga").is_err());
    }
}
