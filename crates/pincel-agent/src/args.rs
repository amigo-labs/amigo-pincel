//! Typed access to validated tool arguments, plus the shared argument
//! shapes (colors, regions) and their schema fragments.

use pincel_core::{Palette, Rect, Rgba};
use serde_json::{Map, Value, json};

use crate::AgentError;

/// The arguments of one call, already validated against the tool schema.
pub(crate) struct Args(Map<String, Value>);

impl Args {
    pub fn new(map: Map<String, Value>) -> Self {
        Self(map)
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key).filter(|v| !v.is_null())
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    pub fn req_str(&self, key: &str) -> Result<&str, AgentError> {
        self.str(key)
            .ok_or_else(|| AgentError::tool(format!("missing required argument `{key}`")))
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    pub fn i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::as_i64)
    }

    pub fn req_i32(&self, key: &str) -> Result<i32, AgentError> {
        int_i32(self.get(key), key)
    }

    pub fn u32(&self, key: &str) -> Result<Option<u32>, AgentError> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(Some)
                .ok_or_else(|| AgentError::tool(format!("`{key}` must be a non-negative integer"))),
        }
    }

    pub fn req_u32(&self, key: &str) -> Result<u32, AgentError> {
        self.u32(key)?
            .ok_or_else(|| AgentError::tool(format!("missing required argument `{key}`")))
    }
}

/// An `i32` from a JSON integer, naming `key` on failure.
pub(crate) fn int_i32(v: Option<&Value>, key: &str) -> Result<i32, AgentError> {
    v.and_then(Value::as_i64)
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| AgentError::tool(format!("`{key}` must be an integer")))
}

/// The accepted color forms, quoted in error messages.
pub(crate) const COLOR_FORMS: &str = "\"#RRGGBB\", \"#RRGGBBAA\" or {\"index\": n}";

/// Schema of a color argument.
pub(crate) fn color_schema(description: &str) -> Value {
    json!({
        "description": format!("{description}: {COLOR_FORMS} (a palette index)"),
        "anyOf": [
            { "type": "string" },
            {
                "type": "object",
                "properties": { "index": { "type": "integer", "minimum": 0 } },
                "required": ["index"],
                "additionalProperties": false,
            },
        ],
    })
}

/// Parse a `#RRGGBB` / `#RRGGBBAA` string.
pub(crate) fn parse_hex(s: &str) -> Option<Rgba> {
    let hex = s.strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some(Rgba::from_u32(v << 8 | 0xff)),
        8 => Some(Rgba::from_u32(v)),
        _ => None,
    }
}

/// Resolve a color argument named `name` against `palette`.
pub(crate) fn parse_color(v: &Value, name: &str, palette: &Palette) -> Result<Rgba, AgentError> {
    if let Some(s) = v.as_str() {
        return parse_hex(s).ok_or_else(|| {
            AgentError::tool(format!(
                "argument `{name}`: malformed color {s:?}; accepted forms: {COLOR_FORMS}"
            ))
        });
    }
    if let Some(index) = v.get("index").and_then(Value::as_u64) {
        return usize::try_from(index)
            .ok()
            .and_then(|i| palette.colors.get(i))
            .map(|e| e.rgba)
            .ok_or_else(|| {
                AgentError::tool(format!(
                    "argument `{name}`: palette index {index} is not in the palette ({} entries); accepted forms: {COLOR_FORMS}",
                    palette.colors.len()
                ))
            });
    }
    Err(AgentError::tool(format!(
        "argument `{name}`: malformed color; accepted forms: {COLOR_FORMS}"
    )))
}

/// `#RRGGBBAA` of a color.
pub(crate) fn hex(c: Rgba) -> String {
    format!("#{:08X}", c.to_u32())
}

/// Schema of a `{x, y, width, height}` rectangle.
pub(crate) fn region_schema(description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "properties": {
            "x": { "type": "integer" },
            "y": { "type": "integer" },
            "width": { "type": "integer", "minimum": 1 },
            "height": { "type": "integer", "minimum": 1 },
        },
        "required": ["x", "y", "width", "height"],
        "additionalProperties": false,
    })
}

/// Parse a region argument (validated against [`region_schema`]).
pub(crate) fn parse_region(v: &Value) -> Result<Rect, AgentError> {
    let x = int_i32(v.get("x"), "region.x")?;
    let y = int_i32(v.get("y"), "region.y")?;
    let w = v
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| AgentError::tool("`region.width` must be a positive integer"))?;
    let h = v
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| AgentError::tool("`region.height` must be a positive integer"))?;
    Ok(Rect::new(x, y, w, h))
}

/// `true` when `r` lies completely inside a `w`×`h` canvas.
pub(crate) fn inside_canvas(r: Rect, w: u32, h: u32) -> bool {
    r.x >= 0
        && r.y >= 0
        && i64::from(r.x) + i64::from(r.width) <= i64::from(w)
        && i64::from(r.y) + i64::from(r.height) <= i64::from(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_accepts_rgb_and_rgba() {
        assert_eq!(parse_hex("#ff0000"), Some(Rgba::new(255, 0, 0, 255)));
        assert_eq!(parse_hex("#00FF0080"), Some(Rgba::new(0, 255, 0, 128)));
        assert_eq!(parse_hex("ff0000"), None);
        assert_eq!(parse_hex("#ff00"), None);
        assert_eq!(parse_hex("#gg0000"), None);
    }

    #[test]
    fn parse_color_index_outside_palette_names_argument_and_forms() {
        let err = parse_color(&json!({ "index": 40 }), "color", &Palette::db32()).unwrap_err();
        let AgentError::Tool(msg) = err else {
            panic!("tool error expected");
        };
        assert!(msg.contains("`color`") && msg.contains("40") && msg.contains(COLOR_FORMS));
    }
}
