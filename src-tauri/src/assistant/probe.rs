//! Asking an installed CLI about itself (docs/specs/ai-chat-clis.md): its
//! version (AC-04) and, for opencode, its models (AC-05). Each question
//! runs the CLI once with a deadline, so a hung CLI never blocks the
//! panel.

use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::adapter::{Kind, is_local};
use super::opencode::parse_models;
use super::run::kill_tree;

/// How long a CLI may take to answer a question.
pub const DEADLINE: Duration = Duration::from_secs(5);

/// Runs `program` with `args` and returns its stdout, or why not: it could
/// not start, it failed (its stderr), or it did not end within `deadline`
/// (then it and its children are ended). `path` is the `PATH` it runs
/// with; `env` extra variables.
pub fn capture(
    program: &Path,
    args: &[&str],
    path: Option<&OsStr>,
    env: &[(&str, &str)],
    deadline: Duration,
) -> Result<String, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(super::run::CREATE_NO_WINDOW);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", program.display()))?;
    let pipe = |mut r: Box<dyn Read + Send>| {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
        });
        rx
    };
    let out = child.stdout.take().map(|s| pipe(Box::new(s)));
    let err = child.stderr.take().map(|s| pipe(Box::new(s)));
    let end = Instant::now() + deadline;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < end => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                kill_tree(child.id());
                let _ = child.wait();
                return Err(format!("{} did not answer in time", program.display()));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    // A grandchild holding a pipe must not keep us waiting.
    let read = |rx: Option<mpsc::Receiver<String>>| {
        rx.and_then(|rx| rx.recv_timeout(Duration::from_secs(1)).ok())
            .unwrap_or_default()
    };
    let (stdout, stderr) = (read(out), read(err));
    if status.success() {
        Ok(stdout)
    } else if stderr.trim().is_empty() {
        Err(format!("exited with {status}"))
    } else {
        Err(stderr.trim().to_owned())
    }
}

/// The first `major.minor.patch` in a `--version` output (`codex-cli
/// 0.156.1`, `2.1.322 (Claude Code)`, `v1.19.5`).
pub fn parse_version(text: &str) -> Option<[u32; 3]> {
    let bytes = text.as_bytes();
    (0..bytes.len())
        .filter(|&i| {
            bytes[i].is_ascii_digit()
                && (i == 0 || !(bytes[i - 1].is_ascii_digit() || bytes[i - 1] == b'.'))
        })
        .find_map(|i| {
            let rest = &text[i..];
            let token: &str = rest
                .split(|c: char| !(c.is_ascii_digit() || c == '.'))
                .next()
                .unwrap_or_default();
            let mut parts = token.split('.').map(str::parse::<u32>);
            match (parts.next(), parts.next(), parts.next()) {
                (Some(Ok(a)), Some(Ok(b)), Some(Ok(c))) => Some([a, b, c]),
                _ => None,
            }
        })
}

/// `major.minor.patch` as text.
pub fn version_text(v: [u32; 3]) -> String {
    format!("{}.{}.{}", v[0], v[1], v[2])
}

/// The version of the CLI at `program`; `None` when it cannot be read.
pub fn version(program: &Path, path: Option<&OsStr>, env: &[(&str, &str)]) -> Option<[u32; 3]> {
    capture(program, &["--version"], path, env, DEADLINE)
        .ok()
        .as_deref()
        .and_then(parse_version)
}

/// A model in the model field's list (AC-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Model {
    /// The id the CLI takes (`ollama/llama3.2`).
    pub id: String,
    /// Served by Ollama or LM Studio (AC-06).
    pub local: bool,
}

/// The models `opencode models` lists within `deadline`.
pub fn opencode_models(
    program: &Path,
    path: Option<&OsStr>,
    env: &[(&str, &str)],
    deadline: Duration,
) -> Result<Vec<Model>, String> {
    let out = capture(program, &["models"], path, env, deadline)?;
    Ok(parse_models(&out)
        .into_iter()
        .map(|id| Model {
            local: is_local(Kind::Opencode, Some(&id)),
            id,
        })
        .collect())
}
