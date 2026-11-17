//! The effort a CLI's answer runs with (docs/specs/ai-chat-clis.md
//! AC-10 – AC-15): which CLIs take one on their command line, from which
//! version, with which levels, and which text may reach the command line.

/// How a CLI takes an effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffortDef {
    /// No effort option (Gemini CLI).
    None,
    /// One of `levels`, from version `since` on.
    Levels {
        /// The oldest version with the option.
        since: [u32; 3],
        /// The levels, in the field's order.
        levels: &'static [&'static str],
    },
    /// Free text (opencode's model variants), from version `since` on.
    Text {
        /// The oldest version with the option.
        since: [u32; 3],
    },
}

impl EffortDef {
    /// Whether a CLI of `version` takes an effort.
    pub fn offered(self, version: Option<[u32; 3]>) -> bool {
        match self {
            EffortDef::None => false,
            EffortDef::Levels { since, .. } | EffortDef::Text { since } => {
                version.is_some_and(|v| v >= since)
            }
        }
    }

    /// The levels the field lists; empty for free text.
    pub fn levels(self) -> &'static [&'static str] {
        match self {
            EffortDef::Levels { levels, .. } => levels,
            EffortDef::None | EffortDef::Text { .. } => &[],
        }
    }
}

/// The longest effort text that may reach a command line (AC-15).
pub const MAX_LEN: usize = 32;

/// `true` when `effort` has a variant name's shape: 1 to 32 ASCII
/// letters, digits, `-` or `_` (AC-15).
pub fn valid(effort: &str) -> bool {
    (1..=MAX_LEN).contains(&effort.len())
        && effort
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The effort of a send request: trimmed, empty meaning the CLI's
/// default; an effort of any other shape is refused, so no text but a
/// variant name reaches a command line (AC-15).
pub fn of_request(effort: Option<String>) -> Result<Option<String>, String> {
    match effort.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(e) if valid(e) => Ok(Some(e.to_owned())),
        Some(_) => Err("invalid effort: use letters, digits, - or _".to_owned()),
    }
}
