//! Bearer tokens and session ids, and the persisted endpoint config
//! (`mcp.json` in the app config dir: token and port, AC-03).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// 64 random bits from the operating system's generator.
///
/// The standard library seeds each thread's `RandomState` keys from the
/// OS (`BCryptGenRandom` / `getrandom` / `arc4random`) once, so a fresh
/// thread hashes with fresh 128-bit secret keys; SipHash under a secret key
/// is a pseudorandom function. One thread per 64 bits keeps every part of a
/// token independent, without a dependency for the RNG.
fn os_random_u64() -> u64 {
    std::thread::spawn(|| {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(0x7069_6e63_656c_6d63);
        h.finish()
    })
    .join()
    .unwrap_or_else(|_| {
        // Spawning failed (no threads left): fall back to this thread's
        // keys, still seeded from the OS.
        let mut h = RandomState::new().build_hasher();
        h.write_u64(0x6d63_7066_616c_6c62);
        h.finish()
    })
}

/// `words` × 64 random bits as lower-case hex.
fn random_hex(words: usize) -> String {
    (0..words)
        .map(|_| format!("{:016x}", os_random_u64()))
        .collect()
}

/// A new 256-bit bearer token (AC-02, AC-20, AC-22).
pub(crate) fn new_token() -> String {
    random_hex(4)
}

/// A new 128-bit session id (`Mcp-Session-Id`, AC-12).
pub(crate) fn new_session_id() -> String {
    random_hex(2)
}

/// Compare a presented token with a known one without an early exit.
pub(crate) fn token_eq(presented: &str, known: &str) -> bool {
    let (a, b) = (presented.as_bytes(), known.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// What survives a restart: the external token and the port.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Config {
    pub token: Option<String>,
    pub port: Option<u16>,
}

impl Config {
    /// The config at `path`; empty when it is missing or unreadable.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Write the config to `path`, readable by the user only where the OS
    /// has such permissions.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_token_is_256_bit_hex_and_never_repeats() {
        let a = new_token();
        let b = new_token();
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
        assert_eq!(new_session_id().len(), 32);
        // Each 64-bit word comes from its own key.
        let words: Vec<&str> = (0..4).map(|i| &a[i * 16..i * 16 + 16]).collect();
        assert!(
            words
                .iter()
                .all(|w| words.iter().filter(|x| *x == w).count() == 1)
        );
    }

    #[test]
    fn token_eq_needs_the_same_bytes() {
        assert!(token_eq("abc", "abc"));
        assert!(!token_eq("abd", "abc"));
        assert!(!token_eq("ab", "abc"));
    }

    #[test]
    fn config_round_trips_and_loads_empty_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("mcp.json");
        assert_eq!(Config::load(&path), Config::default());
        let c = Config {
            token: Some(new_token()),
            port: Some(39217),
        };
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path), c);
        std::fs::write(&path, "garbage").unwrap();
        assert_eq!(Config::load(&path), Config::default());
    }
}
