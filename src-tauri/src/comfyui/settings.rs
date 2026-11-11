//! What the connection remembers between starts, in the app config dir:
//! the server choice (`comfyui.json`, AC-02) and the server Pincel
//! started last (`comfyui-run.json`, AC-10).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Which server Pincel uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerChoice {
    #[default]
    Automatic,
    Existing,
    Custom,
    /// The installation `comfyui-install` made.
    Managed,
}

/// The stored connection settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub server: ServerChoice,
    /// Folder of the existing installation.
    pub existing_dir: Option<String>,
    /// Custom server URL as typed.
    pub custom_url: String,
    /// Folder of the managed installation (set by `comfyui-install`).
    pub managed_dir: Option<String>,
}

/// A server Pincel started: where it listens and its process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub port: u16,
    pub pid: u32,
    /// The installation folder it runs from.
    pub dir: String,
}

/// Reads a JSON file; a missing or damaged one counts as `None`.
pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Writes the whole file or nothing (temporary file, then rename).
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let partial = path.with_extension("json.tmp");
    fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    fs::rename(&partial, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_to_automatic() {
        let tmp = tempfile::tempdir().unwrap();
        let loaded: Settings = read_json(&tmp.path().join("comfyui.json")).unwrap_or_default();
        assert_eq!(loaded.server, ServerChoice::Automatic);
        std::fs::write(tmp.path().join("bad.json"), b"{not json").unwrap();
        let damaged: Option<Settings> = read_json(&tmp.path().join("bad.json"));
        assert_eq!(damaged, None);
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("cfg").join("comfyui.json");
        let settings = Settings {
            server: ServerChoice::Custom,
            existing_dir: Some("D:/ComfyUI_windows_portable".to_string()),
            custom_url: "http://gpu-box:8188".to_string(),
            managed_dir: None,
        };
        write_json(&path, &settings).unwrap();
        assert_eq!(read_json::<Settings>(&path), Some(settings));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"server\": \"custom\""), "{text}");
        assert!(text.contains("\"customUrl\""), "{text}");
    }
}
