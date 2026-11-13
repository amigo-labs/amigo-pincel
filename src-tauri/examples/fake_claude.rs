//! A stand-in for the agent CLIs in the Assistant tests
//! (docs/specs/ai-chat-panel.md, "Tests without a real model";
//! docs/specs/ai-chat-clis.md): the same program plays Claude Code, Codex
//! CLI, Gemini CLI and opencode, since Pincel starts it by path.
//!
//! An answer: it runs in the chat folder Pincel made (its working
//! directory `<root>/<chat>`) and reads its script from
//! `<root>/<chat>.script`: one step per line, `out <line>` (stdout), `err
//! <line>` (stderr), `sleep <ms>` or `exit <code>`. It records what Pincel
//! handed it beside the folder: `<chat>.args.json` (its arguments),
//! `<chat>.stdin.txt` (the prompt), `<chat>.env.json` (the variables
//! Pincel sets for the CLIs), `<chat>.files/` (a copy of every file in the
//! chat folder), `<chat>.mcp.json` (the `--mcp-config` file) and
//! `<chat>.system.md` (the `--append-system-prompt-file` file).
//!
//! A question (`--version`, `models`): it waits `FAKE_CLI_SLEEP_MS`, then
//! prints `FAKE_CLI_VERSION` / `FAKE_CLI_MODELS` and exits with
//! `FAKE_CLI_EXIT` (0 by default).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The variables the adapters set (ai-chat-clis).
const RECORDED_ENV: &[&str] = &[
    "PINCEL_MCP_TOKEN",
    "GEMINI_CLI_SYSTEM_SETTINGS_PATH",
    "OPENCODE_CONFIG",
    "OPENCODE_DISABLE_PROJECT_CONFIG",
];

fn value_after(args: &[String], flag: &str) -> Option<String> {
    let at = args.iter().position(|a| a == flag)?;
    args.get(at + 1).cloned()
}

fn record(path: &Path, bytes: &[u8]) {
    if let Err(e) = std::fs::write(path, bytes) {
        eprintln!("fake cli: cannot record {}: {e}", path.display());
    }
}

fn json_str(s: &str) -> String {
    format!("{s:?}")
}

/// `--version` or `models`: print what the test asked for.
fn question(what: &str) -> ! {
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    if let Ok(ms) = env("FAKE_CLI_SLEEP_MS").parse::<u64>() {
        std::thread::sleep(Duration::from_millis(ms));
    }
    let text = env(if what == "models" {
        "FAKE_CLI_MODELS"
    } else {
        "FAKE_CLI_VERSION"
    });
    print!("{text}");
    let _ = std::io::stdout().flush();
    std::process::exit(env("FAKE_CLI_EXIT").parse().unwrap_or(0));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first @ ("--version" | "models")) = args.first().map(String::as_str) {
        question(first);
    }
    let Ok(cwd) = std::env::current_dir() else {
        eprintln!("fake cli: no working directory");
        std::process::exit(3);
    };
    let (Some(root), Some(chat)) = (cwd.parent(), cwd.file_name()) else {
        eprintln!("fake cli: the working directory has no parent");
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
            .map(|a| json_str(a))
            .collect::<Vec<_>>()
            .join(",")
    );
    record(&beside("args.json"), json.as_bytes());
    let env = format!(
        "{{{}}}",
        RECORDED_ENV
            .iter()
            .filter_map(|k| std::env::var(k).ok().map(|v| format!(
                "{}:{}",
                json_str(k),
                json_str(&v)
            )))
            .collect::<Vec<_>>()
            .join(",")
    );
    record(&beside("env.json"), env.as_bytes());
    let files = beside("files");
    let _ = std::fs::create_dir_all(&files);
    for entry in std::fs::read_dir(&cwd).into_iter().flatten().flatten() {
        if entry.path().is_file() {
            record(
                &files.join(entry.file_name()),
                &std::fs::read(entry.path()).unwrap_or_default(),
            );
        }
    }
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
