//! What a layer made from a generated variant records about its generation
//! (docs/specs/comfyui-generation-info.md). Pixel mode keeps the same values
//! as the text form in a layer's user data (`pincel-core`); the text form is
//! written here too, for the Copy button.

use serde::{Deserialize, Serialize};

/// The generation that produced a layer (Terms, "Generation info").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// The text form (Technical notes): seven `Label: value` lines with
    /// English labels, backslashes and line breaks escaped, LF endings.
    pub fn to_text(&self) -> String {
        format!(
            "Prompt: {}\nNegative prompt: {}\nModel: {}\nLoRA: {}\nLoRA strength: {}\nStrength: {}%\nSeed: {}",
            escape(&self.prompt),
            escape(&self.negative),
            escape(&self.model),
            escape(&self.lora),
            self.lora_strength,
            self.strength,
            self.seed,
        )
    }
}

/// `\` as `\\`, LF as `\n` and CR as `\r`, so a value stays on one line.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// A generation info for tests in this crate.
#[cfg(test)]
pub(crate) fn sample() -> GenerationInfo {
    GenerationInfo {
        prompt: "a red apple\non a \\ table".into(),
        negative: String::new(),
        model: "sd15.safetensors".into(),
        lora: String::new(),
        lora_strength: 1.0,
        strength: 55.5,
        seed: 1234,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> GenerationInfo {
        sample()
    }

    #[test]
    fn to_text_writes_seven_lines_with_escapes() {
        assert_eq!(
            info().to_text(),
            "Prompt: a red apple\\non a \\\\ table\nNegative prompt: \nModel: sd15.safetensors\n\
             LoRA: \nLoRA strength: 1\nStrength: 55.5%\nSeed: 1234"
        );
    }

    #[test]
    fn is_valid_refuses_empty_prompt_and_values_out_of_range() {
        assert!(info().is_valid());
        let cases: [fn(&mut GenerationInfo); 6] = [
            |i| i.prompt.clear(),
            |i| i.model.clear(),
            |i| i.strength = 0.5,
            |i| i.strength = 100.5,
            |i| i.lora_strength = -0.1,
            |i| i.lora_strength = 2.1,
        ];
        for change in cases {
            let mut bad = info();
            change(&mut bad);
            assert!(!bad.is_valid(), "{bad:?}");
        }
    }
}
