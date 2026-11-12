//! One CLI process per answer (docs/specs/ai-chat-panel.md, Technical
//! notes): started in the chat's own folder with the prompt on stdin, its
//! output read into chat events, ended on Stop, cleaned up with the chat.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::claude::{self, Parser, classify};
use super::{ChatEvent, Failure, Hint, Outcome};

/// How long stderr may stay open after the process ended (a grandchild
/// holding the pipe must not block the answer's end).
const READER_GRACE: Duration = Duration::from_secs(1);
/// stderr lines kept for the error text.
const STDERR_LINES: usize = 40;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Where the chat's events go (the webview in the app, a list in tests).
pub trait Sink: Send + Sync {
    /// `event` happened in the answer of `chat`.
    fn event(&self, chat: &str, event: ChatEvent);
}

/// One answer to start.
#[derive(Debug, Clone)]
pub struct Answer {
    /// The chat's id.
    pub chat: String,
    /// The CLI's executable.
    pub program: PathBuf,
    /// The `PATH` the CLI runs with (the login shell's on macOS / Linux);
    /// `None` keeps the app's.
    pub path_env: Option<OsString>,
    /// The user's message with its editor-context block (AC-15).
    pub prompt: String,
    /// The system prompt (AC-18, AC-19).
    pub system_prompt: String,
    /// The model; `None` for the CLI's default (AC-04).
    pub model: Option<String>,
    /// The CLI session to continue (AC-12).
    pub session: Option<String>,
    /// The live endpoint's URL and the chat's token (`mcp-live-desktop`
    /// AC-22).
    pub mcp_url: String,
    /// The chat's bearer token.
    pub mcp_token: String,
}

/// A running answer.
struct Running {
    pid: u32,
    stopped: Arc<AtomicBool>,
}

/// The chats' CLI processes and folders.
pub struct Assistant {
    root: PathBuf,
    sink: Arc<dyn Sink>,
    runs: Arc<Mutex<HashMap<String, Running>>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Assistant {
    /// Chat folders live under `root`, which belongs to the Assistant alone.
    pub fn new(root: PathBuf, sink: Arc<dyn Sink>) -> Self {
        Self {
            root,
            sink,
            runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The folder all chat folders live in.
    #[cfg(test)]
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// The folder of `chat` (its id reduced to a safe file name).
    pub fn chat_dir(&self, chat: &str) -> PathBuf {
        let name: String = chat
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.root.join(name)
    }

    /// Remove the chat folders a crashed run left behind (AC-22).
    pub fn purge(&self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }

    /// An answer of `chat` runs.
    pub fn running(&self, chat: &str) -> bool {
        lock(&self.runs).contains_key(chat)
    }

    /// Start `answer`; its events go to the sink and end with
    /// [`ChatEvent::Done`]. Fails, starting nothing, when the chat's
    /// folder cannot be written or the CLI cannot start (AC-28).
    pub fn start(&self, answer: Answer) -> Result<(), String> {
        if self.running(&answer.chat) {
            return Err("an answer is already running in this chat".to_owned());
        }
        let dir = self.chat_dir(&answer.chat);
        prepare(&self.root, &dir, &answer)
            .map_err(|e| format!("cannot write the chat folder {}: {e}", dir.display()))?;
        let mut command = Command::new(&answer.program);
        command
            .args(claude::args(&answer, &dir))
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(path) = &answer.path_env {
            command.env("PATH", path);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(unix)]
        {
            // Its own process group, so Stop reaches the CLI's children.
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", answer.program.display()))?;
        // The prompt, then end of input: the CLI gets nothing else (AC-27).
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(answer.prompt.as_bytes());
        }
        let stopped = Arc::new(AtomicBool::new(false));
        let (chat, pid) = (answer.chat.clone(), child.id());
        lock(&self.runs).insert(
            chat.clone(),
            Running {
                pid,
                stopped: Arc::clone(&stopped),
            },
        );
        let (sink, runs) = (Arc::clone(&self.sink), Arc::clone(&self.runs));
        let spawned = thread::Builder::new()
            .name("assistant-answer".to_owned())
            .spawn(move || {
                let done = supervise(&mut child, &answer, &*sink, &stopped);
                // The token stays on disk only while an answer runs.
                let _ = std::fs::remove_file(dir.join(claude::MCP_CONFIG));
                lock(&runs).remove(&answer.chat);
                sink.event(&answer.chat, done);
            });
        if let Err(e) = spawned {
            lock(&self.runs).remove(&chat);
            kill_tree(pid);
            return Err(format!("cannot follow the CLI: {e}"));
        }
        Ok(())
    }

    /// End the answer of `chat` (AC-11): the CLI and the processes it
    /// started; the answer then ends as stopped.
    pub fn stop(&self, chat: &str) {
        let pid = lock(&self.runs).get(chat).map(|r| {
            r.stopped.store(true, Ordering::SeqCst);
            r.pid
        });
        if let Some(pid) = pid {
            kill_tree(pid);
        }
    }

    /// Stop every answer and wait up to `wait` for them to end (app exit,
    /// AC-22).
    pub fn stop_all(&self, wait: Duration) {
        let chats: Vec<String> = lock(&self.runs).keys().cloned().collect();
        for chat in &chats {
            self.stop(chat);
        }
        let deadline = Instant::now() + wait;
        while !lock(&self.runs).is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// The chat ended (AC-13, AC-14): stop its answer, remove its folder.
    pub fn end_chat(&self, chat: &str) {
        self.stop(chat);
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.running(chat) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        let _ = std::fs::remove_dir_all(self.chat_dir(chat));
    }
}

/// A private folder: owner-only (0o700 on Unix) whether it is created now
/// or already existed with wider rights, and refused when it is a link or
/// not a folder, so nothing is written through a link someone else planted.
fn private_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)?;
    let meta = std::fs::symlink_metadata(dir)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(std::io::Error::other("it is a link, not a folder"));
    }
    // The mode above applies only to a folder created now.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// A new owner-only file (0o600 on Unix): an older one is removed first,
/// and an existing entry in its place is never written through.
fn private_file(path: &Path, text: &str) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(text.as_bytes())
}

/// The chat folder `dir` under `root` with the answer's MCP config (the
/// chat's token) and system prompt (AC-28 when it fails).
fn prepare(root: &Path, dir: &Path, answer: &Answer) -> std::io::Result<()> {
    private_dir(root)?;
    private_dir(dir)?;
    private_file(&dir.join(claude::MCP_CONFIG), &claude::mcp_config(answer))?;
    private_file(&dir.join(claude::SYSTEM_PROMPT), &answer.system_prompt)
}

/// End process `pid` and its children: on Windows the whole tree (an npm
/// `.cmd` shim runs Node as its child), elsewhere its process group.
fn kill_tree(pid: u32) {
    let mut command = if cfg!(windows) {
        let mut c = Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/T", "/F"]);
        c
    } else {
        let mut c = Command::new("kill");
        c.args(["-KILL", &format!("-{pid}")]);
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Collect stderr's last lines on a thread of its own.
fn follow_stderr(stderr: ChildStderr) -> (Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    let handle = thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
            let line = String::from_utf8_lossy(&buf).trim_end().to_owned();
            buf.clear();
            if line.is_empty() {
                continue;
            }
            let mut l = lock(&sink);
            l.push(line);
            if l.len() > STDERR_LINES {
                l.remove(0);
            }
        }
    });
    (lines, handle)
}

/// Read the answer's output to its end; returns its [`ChatEvent::Done`].
fn supervise(
    child: &mut Child,
    answer: &Answer,
    sink: &dyn Sink,
    stopped: &AtomicBool,
) -> ChatEvent {
    let stderr = child.stderr.take().map(follow_stderr);
    let mut parser = Parser::default();
    let mut unreadable: Option<String> = None;
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        let mut buf = Vec::new();
        while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
            let line = String::from_utf8_lossy(&buf).into_owned();
            buf.clear();
            match parser.line(&line) {
                Ok(events) => {
                    for e in events {
                        sink.event(&answer.chat, e);
                    }
                }
                Err(()) => {
                    // Output Pincel cannot read ends the answer (AC-20).
                    unreadable = Some(line.trim().to_owned());
                    kill_tree(child.id());
                    // Drain, so the process is not blocked on a full pipe.
                    let _ = reader.read_to_end(&mut Vec::new());
                    break;
                }
            }
        }
    }
    let status = child.wait();
    let stderr_text = stderr.map_or_else(String::new, |(lines, handle)| {
        let deadline = Instant::now() + READER_GRACE;
        while !handle.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        lock(&lines).join("\n")
    });
    let fail = |text: String, hint: Hint| ChatEvent::Done {
        outcome: Outcome::Failed,
        error: Some(Failure { text, hint }),
    };
    if stopped.load(Ordering::SeqCst) {
        return ChatEvent::Done {
            outcome: Outcome::Stopped,
            error: None,
        };
    }
    let model = answer.model.as_deref();
    if let Some(line) = unreadable {
        let hint = classify(&line, model);
        return fail(line, hint);
    }
    if parser.denied() {
        let text = parser.error_text().map_or(stderr_text, str::to_owned);
        return fail(text, Hint::Permission);
    }
    let exited_ok = status.as_ref().is_ok_and(|s| s.success());
    if exited_ok && parser.finished_ok() {
        return ChatEvent::Done {
            outcome: Outcome::Finished,
            error: None,
        };
    }
    let text = parser
        .error_text()
        .filter(|t| !t.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| (!stderr_text.trim().is_empty()).then_some(stderr_text))
        .unwrap_or_else(|| match &status {
            Ok(s) => format!("exited with {s}"),
            Err(e) => e.to_string(),
        });
    let hint = classify(&text, model);
    fail(text, hint)
}
