//! Finding the supported agent CLIs (docs/specs/ai-chat-panel.md AC-03):
//! where the user's terminal would find them.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use super::adapter::Kind;

/// A supported CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliDef {
    /// Stable id the UI stores (`claude`).
    pub id: &'static str,
    /// Product name shown in the panel.
    pub name: &'static str,
    /// The command the user types.
    pub program: &'static str,
    /// Where to install it.
    pub install_url: &'static str,
    /// Its adapter.
    pub kind: Kind,
    /// The oldest version offering the tool limits, streamed JSON output
    /// and session resume Pincel relies on (ai-chat-clis AC-04).
    pub min_version: [u32; 3],
}

/// The supported CLIs, in the picker's order (ai-chat-clis AC-01). The
/// minimum versions are recorded in docs/specs/ai-chat-clis.md, Decisions.
pub const CLIS: &[CliDef] = &[
    CliDef {
        id: "claude",
        name: "Claude Code",
        program: "claude",
        install_url: "https://docs.claude.com/en/docs/claude-code",
        kind: Kind::Claude,
        min_version: [2, 1, 69],
    },
    CliDef {
        id: "codex",
        name: "Codex CLI",
        program: "codex",
        install_url: "https://github.com/openai/codex",
        kind: Kind::Codex,
        min_version: [0, 122, 0],
    },
    CliDef {
        id: "gemini",
        name: "Gemini CLI",
        program: "gemini",
        install_url: "https://github.com/google-gemini/gemini-cli",
        kind: Kind::Gemini,
        min_version: [0, 34, 0],
    },
    CliDef {
        id: "opencode",
        name: "opencode",
        program: "opencode",
        install_url: "https://opencode.ai",
        kind: Kind::Opencode,
        min_version: [1, 1, 1],
    },
];

/// The CLI with the id `id`.
pub fn cli(id: &str) -> Option<&'static CliDef> {
    CLIS.iter().find(|c| c.id == id)
}

/// How long the login shell may take to print its `PATH`.
const SHELL_WAIT: Duration = Duration::from_secs(3);

/// The `PATH` the user's terminal has. On macOS and Linux an app started
/// from the Dock or a launcher does not inherit it, so it comes from the
/// login shell; on Windows it is the process's own.
pub fn search_path() -> OsString {
    let own = std::env::var_os("PATH").unwrap_or_default();
    if cfg!(windows) {
        return own;
    }
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/sh"));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let printed = Command::new(shell)
            .args(["-l", "-c", "printf %s \"$PATH\""])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok());
        let _ = tx.send(printed);
    });
    match rx.recv_timeout(SHELL_WAIT) {
        Ok(Some(path)) if !path.trim().is_empty() => OsString::from(path.trim()),
        _ => own,
    }
}

/// `program` in the directories of `path`, as a terminal would run it:
/// with one of the extensions in `pathext` (Windows, where npm installs
/// CLIs as `.cmd` shims) or as an executable file (elsewhere, `pathext`
/// `None`).
pub fn find_in(program: &str, path: &OsStr, pathext: Option<&str>) -> Option<PathBuf> {
    let exts: Vec<&str> = pathext
        .map(|p| p.split(';').filter(|e| !e.is_empty()).collect())
        .unwrap_or_default();
    std::env::split_paths(path).find_map(|dir| {
        if pathext.is_some() {
            exts.iter()
                .map(|ext| dir.join(format!("{program}{}", ext.to_ascii_lowercase())))
                .find(|p| p.is_file())
        } else {
            let p = dir.join(program);
            executable(&p).then_some(p)
        }
    })
}

#[cfg(unix)]
fn executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(p: &std::path::Path) -> bool {
    p.is_file()
}

/// `program` where the user's terminal finds it, with that `PATH`.
pub fn locate(program: &str) -> (Option<PathBuf>, OsString) {
    let path = search_path();
    (locate_in(program, &path), path)
}

/// `program` in the `PATH` `path`, with the platform's extensions.
pub fn locate_in(program: &str, path: &OsStr) -> Option<PathBuf> {
    let pathext = cfg!(windows)
        .then(|| std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned()));
    find_in(program, path, pathext.as_deref())
}
