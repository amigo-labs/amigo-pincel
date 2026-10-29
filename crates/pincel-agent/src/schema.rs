//! A small JSON Schema subset: building the hand-written tool schemas and
//! validating call arguments against them.
//!
//! Supported keywords: `type` (`object`, `array`, `string`, `integer`,
//! `number`, `boolean`), `properties`, `required`, `additionalProperties:
//! false`, `items`, `enum`, `minimum`, `maximum`, `minItems`, `maxItems` and
//! `anyOf`. That covers every schema the tools declare; an unknown keyword
//! is ignored, as JSON Schema prescribes. See docs/specs/mcp-headless-pixel.md
//! (Technical notes, "Schemas").

use serde_json::{Map, Value, json};

/// An object schema with `properties`, `required` and no extra properties.
pub(crate) fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

/// Validate `value` against `schema`; the error names the offending
/// argument path (e.g. `pixels[2].x`) and what was expected.
pub(crate) fn validate(schema: &Value, value: &Value) -> Result<(), String> {
    check(schema, value, "")
}

fn label(path: &str) -> &str {
    if path.is_empty() { "arguments" } else { path }
}

fn child(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

fn type_matches(ty: &str, value: &Value) -> bool {
    match ty {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

fn check(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    let Some(schema) = schema.as_object() else {
        return Ok(());
    };
    if let Some(any) = schema.get("anyOf").and_then(Value::as_array) {
        let mut first_err = None;
        for alt in any {
            match check(alt, value, path) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
        }
        let hint = schema
            .get("description")
            .and_then(Value::as_str)
            .map(|d| format!(" ({d})"))
            .unwrap_or_default();
        return Err(format!(
            "{} does not match any accepted form{hint}",
            label(path)
        ));
    }
    if let Some(ty) = schema.get("type").and_then(Value::as_str)
        && !type_matches(ty, value)
    {
        return Err(format!("{} must be of type {ty}", label(path)));
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array)
        && !options.contains(value)
    {
        let names: Vec<String> = options
            .iter()
            .map(|o| o.as_str().map_or_else(|| o.to_string(), str::to_owned))
            .collect();
        return Err(format!(
            "{} must be one of: {} (got {value})",
            label(path),
            names.join(", ")
        ));
    }
    if let Some(n) = value.as_f64() {
        let min = schema.get("minimum").and_then(Value::as_f64);
        let max = schema.get("maximum").and_then(Value::as_f64);
        let below = min.is_some_and(|m| n < m);
        let above = max.is_some_and(|m| n > m);
        if below || above {
            return Err(match (min, max) {
                (Some(lo), Some(hi)) => {
                    format!("{} must be {}–{}", label(path), fmt_num(lo), fmt_num(hi))
                }
                (Some(lo), None) => format!("{} must be at least {}", label(path), fmt_num(lo)),
                (None, Some(hi)) => format!("{} must be at most {}", label(path), fmt_num(hi)),
                (None, None) => unreachable!("below or above implies a bound"),
            });
        }
    }
    if let Some(items) = value.as_array() {
        if let Some(min) = schema.get("minItems").and_then(Value::as_u64)
            && (items.len() as u64) < min
        {
            return Err(format!("{} needs at least {min} items", label(path)));
        }
        if let Some(max) = schema.get("maxItems").and_then(Value::as_u64)
            && (items.len() as u64) > max
        {
            return Err(format!("{} takes at most {max} items", label(path)));
        }
        if let Some(item_schema) = schema.get("items") {
            for (i, item) in items.iter().enumerate() {
                check(item_schema, item, &format!("{}[{i}]", label(path)))?;
            }
        }
    }
    if let Some(obj) = value.as_object() {
        check_object(schema, obj, path)?;
    }
    Ok(())
}

fn check_object(
    schema: &Map<String, Value>,
    obj: &Map<String, Value>,
    path: &str,
) -> Result<(), String> {
    let empty = Map::new();
    let props = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for key in required.iter().filter_map(Value::as_str) {
            if !obj.contains_key(key) {
                return Err(format!("missing required argument `{}`", child(path, key)));
            }
        }
    }
    let closed = schema.get("additionalProperties") == Some(&Value::Bool(false));
    for (key, v) in obj {
        match props.get(key) {
            Some(prop) => check(prop, v, &child(path, key))?,
            None if closed => {
                let known: Vec<&str> = props.keys().map(String::as_str).collect();
                return Err(format!(
                    "unknown argument `{}` (accepted: {})",
                    child(path, key),
                    known.join(", ")
                ));
            }
            None => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> Value {
        object(
            json!({
                "width": { "type": "integer", "minimum": 1, "maximum": 4096 },
                "mode": { "type": "string", "enum": ["pixel"] },
                "pixels": {
                    "type": "array",
                    "items": object(json!({ "x": { "type": "integer" } }), &["x"]),
                },
                "color": {
                    "anyOf": [{ "type": "string" }, object(json!({ "index": { "type": "integer" } }), &["index"])],
                },
            }),
            &["width"],
        )
    }

    #[test]
    fn validate_accepts_matching_arguments() {
        let args =
            json!({ "width": 8, "mode": "pixel", "pixels": [{ "x": 1 }], "color": { "index": 2 } });
        assert_eq!(validate(&schema(), &args), Ok(()));
    }

    #[test]
    fn validate_range_names_argument_and_bounds() {
        let err = validate(&schema(), &json!({ "width": 5000 })).unwrap_err();
        assert_eq!(err, "width must be 1–4096");
    }

    #[test]
    fn validate_nested_type_error_names_path() {
        let err =
            validate(&schema(), &json!({ "width": 1, "pixels": [{ "x": "a" }] })).unwrap_err();
        assert_eq!(err, "pixels[0].x must be of type integer");
    }

    #[test]
    fn validate_missing_and_unknown_arguments_are_named() {
        assert_eq!(
            validate(&schema(), &json!({})).unwrap_err(),
            "missing required argument `width`"
        );
        assert!(
            validate(&schema(), &json!({ "width": 1, "bogus": 1 }))
                .unwrap_err()
                .starts_with("unknown argument `bogus`")
        );
    }

    #[test]
    fn validate_enum_and_any_of_report_failures() {
        assert_eq!(
            validate(&schema(), &json!({ "width": 1, "mode": "image" })).unwrap_err(),
            "mode must be one of: pixel (got \"image\")"
        );
        assert!(validate(&schema(), &json!({ "width": 1, "color": 3 })).is_err());
    }
}
