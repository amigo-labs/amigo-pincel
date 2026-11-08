//! The LSTM unicharset: Tesseract's text-format `UNICHARSET`
//! (`src/ccutil/unicharset.cpp`, `load_via_fgets`), reduced to what the
//! recognizer needs — each entry's text and whether its script separates
//! words with spaces.

use crate::error::LoadError;

/// Tesseract's `MAX_NUM_CLASSES`.
const MAX_NUM_CLASSES: usize = 32767;

/// Scripts that do not separate words with spaces
/// (`UNICHARSET::IsSpaceDelimited`).
const NON_SPACE_SCRIPTS: [&str; 5] = ["Han", "Thai", "Hangul", "Hiragana", "Katakana"];

/// One entry of the unicharset.
#[derive(Debug, Clone)]
struct Entry {
    text: String,
    space_delimited: bool,
}

/// The characters a model can output, indexed by unichar id.
#[derive(Debug, Clone)]
pub(crate) struct Unicharset {
    entries: Vec<Entry>,
}

impl Unicharset {
    /// Parses the text format: a count line, then one line per entry.
    pub(crate) fn parse(data: &[u8]) -> Result<Self, LoadError> {
        let text = std::str::from_utf8(data).map_err(|_| LoadError::CharsetUnreadable)?;
        let mut lines = text.lines();
        let size: usize = lines
            .next()
            .and_then(|l| l.split_whitespace().next())
            .and_then(|t| t.parse().ok())
            .ok_or(LoadError::CharsetUnreadable)?;
        if size == 0 || size > MAX_NUM_CLASSES {
            return Err(LoadError::CharsetUnreadable);
        }
        let mut entries: Vec<Entry> = Vec::with_capacity(size.min(data.len()));
        for _ in 0..size {
            let line = lines.next().ok_or(LoadError::CharsetUnreadable)?;
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() < 2 || u32::from_str_radix(tokens[1], 16).is_err() {
                return Err(LoadError::CharsetUnreadable);
            }
            let text = if tokens[0] == "NULL" { " " } else { tokens[0] };
            // The script follows the comma-separated metrics when present.
            let script = match tokens.get(2) {
                Some(t) if t.contains(',') => tokens.get(3).copied(),
                other => other.copied(),
            }
            .unwrap_or("NULL");
            if entries.iter().any(|e| e.text == text) {
                return Err(LoadError::CharsetUnreadable);
            }
            entries.push(Entry {
                text: text.to_owned(),
                space_delimited: !NON_SPACE_SCRIPTS.contains(&script),
            });
        }
        Ok(Self { entries })
    }

    /// Number of entries, including the special ones (space, `Joined`, …).
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// The text of an entry.
    pub(crate) fn text(&self, id: usize) -> &str {
        &self.entries[id].text
    }

    /// Whether the entry's script separates words with spaces.
    pub(crate) fn is_space_delimited(&self, id: usize) -> bool {
        self.entries.get(id).is_none_or(|e| e.space_delimited)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_reads_null_as_space_and_script() {
        let data = b"3\nNULL 0 Common 0\nJoined 7 0,255,0,255,0,0,0,0,0,0 Latin 1 0 1 Joined\n\xe4\xb8\x80 1 0,255,0,255,0,0,0,0,0,0 Han 2 0 2 x\n";
        let u = Unicharset::parse(data).expect("valid unicharset");
        assert_eq!(u.len(), 3);
        assert_eq!(u.text(0), " ");
        assert!(u.is_space_delimited(1));
        assert!(!u.is_space_delimited(2));
    }

    #[test]
    fn parse_short_file_returns_charset_unreadable() {
        assert_eq!(
            Unicharset::parse(b"5\nNULL 0 Common 0\n").err(),
            Some(LoadError::CharsetUnreadable)
        );
    }
}
