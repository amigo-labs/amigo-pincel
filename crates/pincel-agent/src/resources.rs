//! Read-only MCP resources: the built-in palette presets as
//! `pincel://palettes/{id}`.

use pincel_core::{Palette, PalettePreset};
use serde_json::{Value, json};

use crate::AgentError;
use crate::args::hex;

const PALETTE_PREFIX: &str = "pincel://palettes/";

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

/// Every resource, one per palette preset.
pub fn list_resources() -> Vec<ResourceDef> {
    PalettePreset::ALL
        .iter()
        .map(|&p| ResourceDef {
            uri: format!("{PALETTE_PREFIX}{}", p.name()),
            name: format!("palette {}", p.name()),
            description: format!(
                "{} palette preset ({} colors) as #RRGGBBAA strings",
                preset_title(p),
                Palette::preset(p).len()
            ),
            mime_type: "application/json",
        })
        .collect()
}

/// The JSON text of the resource at `uri`.
pub fn read_resource(uri: &str) -> Result<String, AgentError> {
    let preset = uri
        .strip_prefix(PALETTE_PREFIX)
        .and_then(PalettePreset::from_name)
        .ok_or_else(|| AgentError::tool(format!("unknown resource `{uri}`")))?;
    Ok(palette_json(preset).to_string())
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
        assert_eq!(list.len(), PalettePreset::ALL.len());
        assert!(list.iter().any(|r| r.uri == "pincel://palettes/pico8"));
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
