//! A ComfyUI process Pincel started: `<python> main.py --listen 127.0.0.1
//! --port <port>` through `std::process` (never a shell), with the last
//! output lines kept for the "ComfyUI stopped" status (AC-06, AC-07).

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::install::Installation;

/// How long [`Process::final_log`] waits for the output readers.
const READER_GRACE: Duration = Duration::from_secs(1);

/// Output lines kept for the stopped status (AC-07).
pub const LOG_LINES: usize = 50;

/// Windows: start the console program without a console window.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The last [`LOG_LINES`] lines of a process's stdout and stderr.
#[derive(Debug, Clone, Default)]
pub struct Log(Arc<Mutex<VecDeque<String>>>);

impl Log {
    fn push(&self, line: String) {
        let mut lines = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if lines.len() == LOG_LINES {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    /// The kept lines, oldest first.
    pub fn lines(&self) -> Vec<String> {
        let lines = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        lines.iter().cloned().collect()
    }

    fn follow(&self, stream: impl Read + Send + 'static) -> thread::JoinHandle<()> {
        let log = self.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&buf);
                        log.push(line.trim_end_matches(['\r', '\n']).to_string());
                    }
                }
            }
        })
    }
}

/// A running ComfyUI process this app started.
#[derive(Debug)]
pub struct Process {
    child: Mutex<Child>,
    readers: Mutex<Vec<thread::JoinHandle<()>>>,
    pub log: Log,
    pub pid: u32,
    pub port: u16,
}

impl Process {
    /// Whether the process has ended.
    pub fn exited(&self) -> bool {
        let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
        !matches!(child.try_wait(), Ok(None))
    }

    /// Ends the process and waits for it. On Windows the whole tree: a
    /// venv's `python.exe` is a launcher that runs the interpreter as its
    /// child.
    pub fn kill(&self) {
        let mut child = self.child.lock().unwrap_or_else(PoisonError::into_inner);
        if cfg!(windows) && matches!(child.try_wait(), Ok(None)) {
            kill_pid(self.pid);
        }
        let _ = child.kill();
        let _ = child.wait();
    }

    /// The output lines, after the output readers had up to a second to
    /// drain the pipes (a grandchild holding them open must not block).
    pub fn final_log(&self) -> Vec<String> {
        let readers =
            std::mem::take(&mut *self.readers.lock().unwrap_or_else(PoisonError::into_inner));
        let deadline = Instant::now() + READER_GRACE;
        while readers.iter().any(|r| !r.is_finished()) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        self.log.lines()
    }
}

/// A free TCP port on 127.0.0.1.
pub fn free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Starts `inst` bound to 127.0.0.1 on `port`.
pub fn spawn(inst: &Installation, port: u16) -> std::io::Result<Process> {
    let mut command = Command::new(&inst.python);
    command
        .arg("main.py")
        .args(["--listen", "127.0.0.1", "--port", &port.to_string()])
        .current_dir(&inst.comfy_dir)
        // Python buffers a pipe; the log should show the latest lines.
        .env("PYTHONUNBUFFERED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;
    let log = Log::default();
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        readers.push(log.follow(out));
    }
    if let Some(err) = child.stderr.take() {
        readers.push(log.follow(err));
    }
    Ok(Process {
        pid: child.id(),
        child: Mutex::new(child),
        readers: Mutex::new(readers),
        log,
        port,
    })
}

/// Ends process `pid` (a server started in an earlier run, AC-10).
pub fn kill_pid(pid: u32) {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW);
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = Command::new("kill");
        c.arg(pid.to_string());
        c
    };
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_keeps_the_last_lines() {
        let log = Log::default();
        for i in 0..(LOG_LINES + 7) {
            log.push(format!("line {i}"));
        }
        let lines = log.lines();
        assert_eq!(lines.len(), LOG_LINES);
        assert_eq!(lines[0], "line 7");
        assert_eq!(lines[LOG_LINES - 1], format!("line {}", LOG_LINES + 6));
    }
}
