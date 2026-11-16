//! What a layer made from a generated variant records about its generation
//! (docs/specs/comfyui-generation-info.md). Pixel mode keeps it as the text
//! form in the layer's user data, so `.aseprite`, the snapshot and Aseprite
//! itself carry it (Decision Log 2026-11-16 "Generation info"). Image mode
//! keeps the same values on its layers (`pincel-image-core`).

use super::{Layer, UserData};

/// The longest text an `.aseprite` string holds, in bytes (`WORD` length).
pub const MAX_TEXT_BYTES: usize = u16::MAX as usize;

/// The labels of the text form's seven lines, in order.
const LABELS: [&str; 7] = [
    "Prompt",
    "Negative prompt",
    "Model",
    "LoRA",
    "LoRA strength",
    "Strength",
    "Seed",
];

/// The generation that produced a layer (spec Terms, "Generation info").
#[derive(Debug, Clone, PartialEq)]
pub struct GenerationInfo {
    /// The prompt; never empty.
    pub prompt: String,
    /// The negative prompt; empty when none was used.
    pub negative: String,
    /// The checkpoint by the name `gen.models` lists; never empty.
    pub model: String,
    /// The LoRA by its listed name; empty when none was used.
    pub lora: String,
    /// The LoRA's weight, 0–2 (1 when no LoRA was used).
    pub lora_strength: f64,
    /// How much of the input was regenerated, in percent (1–100).
    pub strength: f64,
    /// The seed of this variant.
    pub seed: u64,
}

impl GenerationInfo {
    /// Whether every value is in its range (AC-12): a prompt and a model,
    /// a strength of 1–100 % and a LoRA strength of 0–2.
    pub fn is_valid(&self) -> bool {
        !self.prompt.is_empty()
            && !self.model.is_empty()
            && (1.0..=100.0).contains(&self.strength)
            && (0.0..=2.0).contains(&self.lora_strength)
    }

    /// The text form (spec Technical notes): seven `Label: value` lines with
    /// English labels, backslashes and line breaks escaped, LF endings.
    pub fn to_text(&self) -> String {
        let values = [
            escape(&self.prompt),
            escape(&self.negative),
            escape(&self.model),
            escape(&self.lora),
            self.lora_strength.to_string(),
            format!("{}%", self.strength),
            self.seed.to_string(),
        ];
        LABELS
            .iter()
            .zip(values)
            .map(|(label, value)| format!("{label}: {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Reads the text form back: exactly the seven lines in order (LF or
    /// CRLF), every value valid; anything else is none (AC-12).
    pub fn from_text(text: &str) -> Option<Self> {
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.len() != LABELS.len() {
            return None;
        }
        let mut values = Vec::with_capacity(LABELS.len());
        for (line, label) in lines.iter().zip(LABELS) {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let value = line.strip_prefix(label)?.strip_prefix(": ")?;
            values.push(value);
        }
        let number = |v: &str| v.parse::<f64>().ok().filter(|n| n.is_finite());
        let info = Self {
            prompt: unescape(values[0])?,
            negative: unescape(values[1])?,
            model: unescape(values[2])?,
            lora: unescape(values[3])?,
            lora_strength: number(values[4])?,
            strength: number(values[5].strip_suffix('%')?)?,
            seed: values[6].parse().ok()?,
        };
        info.is_valid().then_some(info)
    }
}

impl Layer {
    /// The generation info this layer's user data holds, if it is exactly
    /// the text form (AC-12).
    pub fn generation(&self) -> Option<GenerationInfo> {
        GenerationInfo::from_text(self.user_data.as_ref()?.text.as_deref()?)
    }

    /// The layer with `info` as its user-data text; unchanged when the text
    /// would exceed what an `.aseprite` file holds (spec Edge cases).
    pub fn with_generation(mut self, info: &GenerationInfo) -> Self {
        let text = info.to_text();
        if text.len() <= MAX_TEXT_BYTES {
            let data = self.user_data.get_or_insert_with(UserData::default);
            data.text = Some(text);
        }
        self
    }
}

/// `\` as `\\`, LF as `\n` and CR as `\r`, so a value stays on one line.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Undoes [`escape`]; none for any other backslash sequence.
fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        out.push(match chars.next()? {
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            _ => return None,
        });
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::LayerId;

    fn info() -> GenerationInfo {
        GenerationInfo {
            prompt: "a red apple\non a \\ table, Seed: 5".into(),
            negative: String::new(),
            model: "sd15.safetensors".into(),
            lora: String::new(),
            lora_strength: 1.0,
            strength: 55.5,
            seed: u64::MAX,
        }
    }

    #[test]
    fn to_text_writes_seven_escaped_lines() {
        assert_eq!(
            info().to_text(),
            "Prompt: a red apple\\non a \\\\ table, Seed: 5\nNegative prompt: \n\
             Model: sd15.safetensors\nLoRA: \nLoRA strength: 1\nStrength: 55.5%\n\
             Seed: 18446744073709551615"
        );
    }

    #[test]
    fn from_text_reads_the_text_form_back_also_with_crlf() {
        let text = info().to_text();
        assert_eq!(GenerationInfo::from_text(&text), Some(info()));
        let crlf = text.replace('\n', "\r\n");
        assert_eq!(GenerationInfo::from_text(&crlf), Some(info()));
        let lora = GenerationInfo {
            lora: "pixel-art.safetensors".into(),
            lora_strength: 0.75,
            ..info()
        };
        assert_eq!(GenerationInfo::from_text(&lora.to_text()), Some(lora));
    }

    #[test]
    fn from_text_refuses_anything_but_the_exact_valid_form() {
        let text = info().to_text();
        let bad = [
            String::new(),
            "my own note".to_string(),
            text.replace("\nSeed: 18446744073709551615", ""),
            format!("{text}\nExtra: 1"),
            text.replace("Model:", "Checkpoint:"),
            text.replace("Prompt: a red", "Prompt:a red"),
            text.replace("Prompt: a red apple\\non a \\\\ table, Seed: 5", "Prompt: "),
            text.replace("Model: sd15.safetensors", "Model: "),
            text.replace("Seed: 18446744073709551615", "Seed: 18446744073709551616"),
            text.replace("Seed: 18446744073709551615", "Seed: -1"),
            text.replace("Strength: 55.5%", "Strength: 55.5"),
            text.replace("Strength: 55.5%", "Strength: 0%"),
            text.replace("LoRA strength: 1", "LoRA strength: 2.5"),
            text.replace("LoRA strength: 1", "LoRA strength: NaN"),
            text.replace("\\\\ table", "\\t table"),
        ];
        for t in bad {
            assert_eq!(GenerationInfo::from_text(&t), None, "{t:?}");
        }
    }

    #[test]
    fn with_generation_sets_the_user_data_text_and_keeps_its_color() {
        let mut layer = Layer::image(LayerId::new(1), "Generated: a red apple");
        layer.user_data = Some(UserData {
            color: Some(crate::document::Rgba::new(1, 2, 3, 4)),
            ..UserData::default()
        });
        let layer = layer.with_generation(&info());
        assert_eq!(layer.generation(), Some(info()));
        let data = layer.user_data.expect("user data");
        assert_eq!(data.text, Some(info().to_text()));
        assert_eq!(data.color, Some(crate::document::Rgba::new(1, 2, 3, 4)));
    }

    #[test]
    fn with_generation_skips_a_text_beyond_the_aseprite_limit() {
        let long = GenerationInfo {
            prompt: "x".repeat(MAX_TEXT_BYTES),
            ..info()
        };
        let layer = Layer::image(LayerId::new(1), "g").with_generation(&long);
        assert_eq!(layer.user_data, None);
        assert_eq!(layer.generation(), None);
    }
}
