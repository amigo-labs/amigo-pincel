//! A stand-in for the Claude Code CLI in the Assistant tests
//! (docs/specs/ai-chat-panel.md, "Tests without a real model").
//!
//! It runs in the chat folder Pincel made (its working directory
//! `<root>/<chat>`) and reads its script from `<root>/<chat>.script`: one
//! step per line, `out <line>` (stdout), `err <line>` (stderr), `sleep <ms>`
//! or `exit <code>`. It records what Pincel handed it beside the folder:
//! `<chat>.args.json` (its arguments), `<chat>.stdin.txt` (the prompt),
//! `<chat>.mcp.json` (the `--mcp-config` file) and `<chat>.system.md` (the
//! `--append-system-prompt-file` file).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn value_after(args: &[String], flag: &str) -> Option<String> {
    let at = args.iter().position(|a| a == flag)?;
    args.get(at + 1).cloned()
}

fn record(path: &Path, bytes: &[u8]) {
    if let Err(e) = std::fs::write(path, bytes) {
        eprintln!("fake claude: cannot record {}: {e}", path.display());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Ok(cwd) = std::env::current_dir() else {
        eprintln!("fake claude: no working directory");
        std::process::exit(3);
    };
    let (Some(root), Some(chat)) = (cwd.parent(), cwd.file_name()) else {
        eprintln!("fake claude: the working directory has no parent");
        std::process::exit(3);
    };
    let chat = chat.to_string_lossy().into_owned();
    let beside = |suffix: &str| -> PathBuf { root.join(format!("{chat}.{suffix}")) };

    let mut stdin = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut stdin);
    record(&beside("stdin.txt"), &stdin);
    let json = format!(
        "[{}]",
        args.iter()
            .map(|a| format!("{a:?}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    record(&beside("args.json"), json.as_bytes());
    for (flag, suffix) in [
        ("--mcp-config", "mcp.json"),
        ("--append-system-prompt-file", "system.md"),
    ] {
        if let Some(file) = value_after(&args, flag) {
            record(&beside(suffix), &std::fs::read(file).unwrap_or_default());
        }
    }

    let script = std::fs::read_to_string(beside("script")).unwrap_or_default();
    let mut out = std::io::stdout();
    for line in script.lines() {
        let (step, rest) = line.split_once(' ').unwrap_or((line, ""));
        match step {
            "out" => {
                let _ = writeln!(out, "{rest}");
                let _ = out.flush();
            }
            "err" => eprintln!("{rest}"),
            "sleep" => std::thread::sleep(Duration::from_millis(rest.parse().unwrap_or(0))),
            "exit" => std::process::exit(rest.parse().unwrap_or(1)),
            _ => {}
        }
    }
}
