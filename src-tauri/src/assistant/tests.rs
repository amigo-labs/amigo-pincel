//! The Assistant's CLI side (docs/specs/ai-chat-panel.md) against a fake
//! CLI (`examples/fake_claude.rs`), which `cargo test` builds; the other
//! CLIs' adapters are tested in `tests_clis.rs` with the same helpers.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::adapter::{self, Kind, Stream};
use super::claude::{self, Parser};
use super::find::find_in;
use super::run::{Answer, Assistant, Sink};
use super::{ChatEvent, Hint, Outcome};

/// Collects the events per chat and wakes waiters on `done`.
#[derive(Default)]
pub(super) struct Events {
    pub(super) seen: Mutex<Vec<(String, ChatEvent)>>,
    changed: Condvar,
}

impl Sink for Events {
    fn event(&self, chat: &str, event: ChatEvent) {
        let mut seen = self.seen.lock().expect("events");
        seen.push((chat.to_owned(), event));
        self.changed.notify_all();
    }
}

impl Events {
    /// The events of `chat` once its answer is done (fails after 10 s).
    pub(super) fn until_done(&self, chat: &str) -> Vec<ChatEvent> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = self.seen.lock().expect("events");
        loop {
            let mine: Vec<ChatEvent> = seen
                .iter()
                .filter(|(c, _)| c == chat)
                .map(|(_, e)| e.clone())
                .collect();
            if mine.iter().any(|e| matches!(e, ChatEvent::Done { .. })) {
                return mine;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "no answer end for {chat}: {mine:?}");
            seen = self.changed.wait_timeout(seen, left).expect("events").0;
        }
    }
}

/// The fake CLI `cargo test` built for this profile. The test binary sits in
/// `<profile>/deps/` or, with cargo's newer build-dir layout, deeper under
/// `<profile>/build/<package>/<hash>/`; examples land in `<profile>/examples/`
/// either way, so the nearest ancestor holding the example wins.
pub(super) fn fake_cli() -> PathBuf {
    let exe = std::env::current_exe().expect("test exe");
    let name = format!("fake_claude{}", std::env::consts::EXE_SUFFIX);
    exe.ancestors()
        .skip(1)
        .map(|dir| dir.join("examples").join(&name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| {
            panic!(
                "examples/{name} not found above {}: run `cargo test -p pincel-tauri`, which builds the examples",
                exe.display()
            )
        })
}

pub(super) fn answer(chat: &str, program: PathBuf) -> Answer {
    Answer {
        chat: chat.to_owned(),
        kind: Kind::Claude,
        program,
        path_env: None,
        prompt: "<editor-context>\ndocument: a.png\n</editor-context>\n\nDraw it".to_owned(),
        system_prompt: "Du bist „PixelAssist“.\nZeile 2".to_owned(),
        model: Some("opus".to_owned()),
        session: Some("sess-1".to_owned()),
        mcp_url: "http://127.0.0.1:39217/mcp".to_owned(),
        mcp_token: "tok-123".to_owned(),
    }
}

pub(super) fn setup() -> (tempfile::TempDir, Arc<Events>, Assistant) {
    let root = tempfile::tempdir().expect("tempdir");
    let events = Arc::new(Events::default());
    let sink: Arc<dyn Sink> = events.clone();
    let assistant = Assistant::new(root.path().join("chats"), sink);
    (root, events, assistant)
}

pub(super) fn script(assistant: &Assistant, chat: &str, lines: &[&str]) {
    std::fs::create_dir_all(assistant.root()).expect("root");
    std::fs::write(
        assistant.root().join(format!("{chat}.script")),
        lines.join("\n"),
    )
    .expect("script");
}

pub(super) fn recorded(assistant: &Assistant, chat: &str, suffix: &str) -> String {
    std::fs::read_to_string(assistant.root().join(format!("{chat}.{suffix}"))).unwrap_or_default()
}

pub(super) fn done(events: &[ChatEvent]) -> (Outcome, Option<(String, Hint)>) {
    match events.last() {
        Some(ChatEvent::Done { outcome, error }) => {
            (*outcome, error.as_ref().map(|f| (f.text.clone(), f.hint)))
        }
        other => panic!("last event is not done: {other:?}"),
    }
}

#[test]
fn find_in_takes_pathext_shims_on_windows_and_executables_elsewhere() {
    let a = tempfile::tempdir().expect("tempdir");
    let b = tempfile::tempdir().expect("tempdir");
    let path = std::env::join_paths([a.path(), b.path()]).expect("path");
    assert_eq!(find_in("claude", &path, Some(".COM;.EXE;.CMD")), None);
    if cfg!(windows) {
        // npm also writes an extensionless shell script, which Windows
        // cannot run; the `.cmd` shim beside it is the CLI.
        std::fs::write(b.path().join("claude"), "#!/bin/sh").expect("sh");
        assert_eq!(find_in("claude", &path, Some(".COM;.EXE;.CMD")), None);
        std::fs::write(b.path().join("claude.cmd"), "@echo off").expect("cmd");
        assert_eq!(
            find_in("claude", &path, Some(".COM;.EXE;.CMD")),
            Some(b.path().join("claude.cmd"))
        );
        std::fs::write(a.path().join("claude.exe"), "").expect("exe");
        assert_eq!(
            find_in("claude", &path, Some(".COM;.EXE;.CMD")),
            Some(a.path().join("claude.exe")),
            "the first PATH entry wins"
        );
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file = b.path().join("claude");
            std::fs::write(&file, "#!/bin/sh").expect("sh");
            assert_eq!(find_in("claude", &path, None), None, "not executable");
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).expect("chmod");
            assert_eq!(find_in("claude", &path, None), Some(file));
        }
    }
}

#[test]
fn claude_args_allow_only_pincel_and_web_search_and_resume_the_session() {
    let dir = Path::new("chat-dir");
    let a = answer("c1", PathBuf::from("claude"));
    let args = adapter::launch(&a, dir).args;
    let has = |pair: &[&str]| args.windows(pair.len()).any(|w| w == pair);
    assert!(has(&["-p"]));
    assert!(has(&["--output-format", "stream-json", "--verbose"]));
    let mcp = dir.join("mcp.json").to_string_lossy().into_owned();
    assert!(has(&["--mcp-config", &mcp, "--strict-mcp-config"]));
    assert!(has(&["--tools", "WebSearch"]));
    assert!(has(&["--allowedTools", "mcp__pincel__*,WebSearch"]));
    let denied = args
        .iter()
        .position(|a| a == "--disallowedTools")
        .and_then(|i| args.get(i + 1))
        .expect("disallowed tools");
    for tool in [
        "Bash",
        "PowerShell",
        "Read",
        "Edit",
        "Write",
        "WebFetch",
        "Glob",
        "Grep",
    ] {
        assert!(denied.split(',').any(|t| t == tool), "{tool} not denied");
    }
    assert!(has(&["--model", "opus"]));
    assert!(has(&["--resume", "sess-1"]));
    // Neither the prompt nor the system prompt travels as an argument.
    assert!(args.iter().all(|a| !a.contains('\n')));
    let fresh = Answer {
        model: None,
        session: None,
        ..answer("c1", PathBuf::from("claude"))
    };
    let args = adapter::launch(&fresh, dir).args;
    assert!(!args.iter().any(|a| a == "--resume" || a == "--model"));
    let config: Value = serde_json::from_str(&claude::mcp_config(&a)).expect("json");
    assert_eq!(
        config,
        json!({ "mcpServers": { "pincel": {
            "type": "http",
            "url": "http://127.0.0.1:39217/mcp",
            "headers": { "Authorization": "Bearer tok-123" }
        } } })
    );
}

#[test]
fn stream_json_becomes_session_text_and_tool_events() {
    let mut p = Parser::default();
    let mut all = Vec::new();
    // A 3×2 PNG's signature and IHDR, base64.
    let png = pincel_agent::base64::encode(&[
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0,
        0, 3, 0, 0, 0, 2, 8, 6, 0, 0, 0,
    ]);
    let lines = [
        r#"{"type":"system","subtype":"init","session_id":"s-9","tools":[]}"#.to_owned(),
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hel"}}}"#.to_owned(),
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"lo"}}}"#.to_owned(),
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"tool_use","id":"t1","name":"mcp__pincel__draw_rect","input":{"x0":1}}]}}"#.to_owned(),
        r#"{"type":"something_new","x":1}"#.to_owned(),
        r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"ok"}],"is_error":false}]}}"#.to_owned(),
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Done."},{"type":"tool_use","id":"t2","name":"mcp__pincel__doc_preview","input":{}}]}}"#.to_owned(),
        format!(r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"t2","content":[{{"type":"image","source":{{"type":"base64","media_type":"image/png","data":"{png}"}}}}],"is_error":true}}]}}}}"#),
        String::new(),
        r#"{"type":"result","subtype":"success","is_error":false,"result":"Done.","session_id":"s-9"}"#.to_owned(),
    ];
    for line in &lines {
        all.extend(p.line(line).expect("readable"));
    }
    assert_eq!(
        all,
        [
            ChatEvent::Session { id: "s-9".into() },
            ChatEvent::Text { text: "Hel".into() },
            ChatEvent::Text { text: "lo".into() },
            ChatEvent::ToolStart {
                id: "t1".into(),
                name: "draw.rect".into(),
                args: json!({ "x0": 1 }),
            },
            ChatEvent::ToolEnd {
                id: "t1".into(),
                ok: true,
                text: "ok".into(),
                images: vec![],
            },
            ChatEvent::Text {
                text: "Done.".into()
            },
            ChatEvent::ToolStart {
                id: "t2".into(),
                name: "doc.preview".into(),
                args: json!({}),
            },
            ChatEvent::ToolEnd {
                id: "t2".into(),
                ok: false,
                text: String::new(),
                images: vec![[3, 2]],
            },
        ]
    );
    assert!(p.finished_ok());
    assert!(p.line("Error: something broke").is_err(), "not JSON");
}

#[test]
fn errors_get_the_hint_of_their_cause() {
    let classify = |text: &str, model: Option<&str>| adapter::classify(text, model, false);
    let model = Some("gpt-9");
    assert_eq!(
        classify("Invalid API key · Please run /login", model),
        Hint::SignIn
    );
    assert_eq!(classify("Error: Not logged in", None), Hint::SignIn);
    assert_eq!(
        classify("API Error: getaddrinfo ENOTFOUND api.anthropic.com", model),
        Hint::Offline
    );
    assert_eq!(
        classify(
            "There's an issue with the selected model (gpt-9). It may not exist",
            model
        ),
        Hint::UnknownModel
    );
    assert_eq!(
        classify("model: gpt-9 not_found_error", model),
        Hint::UnknownModel
    );
    assert_eq!(classify("segfault", model), Hint::Unexpected);
}

#[test]
fn an_answer_streams_events_records_its_inputs_and_finishes() {
    let (_root, events, assistant) = setup();
    script(
        &assistant,
        "c1",
        &[
            r#"out {"type":"system","subtype":"init","session_id":"s-1"}"#,
            r#"out {"type":"assistant","message":{"content":[{"type":"text","text":"Hi"}]}}"#,
            r#"out {"type":"result","subtype":"success","is_error":false,"result":"Hi","session_id":"s-1"}"#,
        ],
    );
    assistant.start(answer("c1", fake_cli())).expect("start");
    let seen = events.until_done("c1");
    assert_eq!(
        seen[..2],
        [
            ChatEvent::Session { id: "s-1".into() },
            ChatEvent::Text { text: "Hi".into() }
        ]
    );
    assert_eq!(done(&seen), (Outcome::Finished, None));
    // AC-12, AC-16: the session, the tool limits and the endpoint reach
    // the CLI; the prompt comes on stdin, which then closes.
    let args: Vec<String> =
        serde_json::from_str(&recorded(&assistant, "c1", "args.json")).expect("args");
    assert_eq!(
        args,
        adapter::launch(&answer("c1", fake_cli()), &assistant.chat_dir("c1")).args
    );
    assert_eq!(
        recorded(&assistant, "c1", "stdin.txt"),
        answer("c1", fake_cli()).prompt
    );
    let config: Value =
        serde_json::from_str(&recorded(&assistant, "c1", "mcp.json")).expect("mcp config");
    assert_eq!(
        config["mcpServers"]["pincel"]["headers"]["Authorization"],
        "Bearer tok-123"
    );
    assert_eq!(
        recorded(&assistant, "c1", "system.md"),
        "Du bist „PixelAssist“.\nZeile 2"
    );
    assert!(!assistant.running("c1"));
    assistant.end_chat("c1");
    assert!(
        !assistant.chat_dir("c1").exists(),
        "the chat folder goes with the chat"
    );
}

#[test]
fn stop_ends_the_cli_within_two_seconds() {
    // Times the stop alone, not Windows starting `taskkill.exe` beside the
    // ComfyUI tests' Python processes (crate::test_support).
    let _timing = crate::test_support::timing();
    let (_root, events, assistant) = setup();
    script(
        &assistant,
        "c2",
        &[
            r#"out {"type":"system","subtype":"init","session_id":"s-2"}"#,
            "sleep 30000",
        ],
    );
    assistant.start(answer("c2", fake_cli())).expect("start");
    // Wait for the CLI to run.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !events
        .seen
        .lock()
        .expect("events")
        .iter()
        .any(|(_, e)| matches!(e, ChatEvent::Session { .. }))
    {
        assert!(Instant::now() < deadline, "the fake CLI did not start");
        std::thread::sleep(Duration::from_millis(20));
    }
    let asked = Instant::now();
    assistant.stop("c2");
    let seen = events.until_done("c2");
    assert!(
        asked.elapsed() < Duration::from_secs(2),
        "{:?}",
        asked.elapsed()
    );
    assert_eq!(done(&seen), (Outcome::Stopped, None));
    assert!(!assistant.running("c2"));
}

#[test]
fn a_failing_cli_reports_its_error_text_and_hint() {
    let (_root, events, assistant) = setup();
    script(
        &assistant,
        "c3",
        &["err Invalid API key · Please run /login", "exit 1"],
    );
    assistant.start(answer("c3", fake_cli())).expect("start");
    let (outcome, error) = done(&events.until_done("c3"));
    assert_eq!(outcome, Outcome::Failed);
    assert_eq!(
        error,
        Some(("Invalid API key · Please run /login".into(), Hint::SignIn))
    );
}

#[test]
fn an_error_result_fails_with_the_cli_text() {
    let (_root, events, assistant) = setup();
    script(
        &assistant,
        "c4",
        &[
            r#"out {"type":"result","subtype":"error_during_execution","is_error":true,"result":"API Error: Connection error.","session_id":"s"}"#,
            "exit 1",
        ],
    );
    assistant.start(answer("c4", fake_cli())).expect("start");
    let (outcome, error) = done(&events.until_done("c4"));
    assert_eq!(outcome, Outcome::Failed);
    assert_eq!(
        error,
        Some(("API Error: Connection error.".into(), Hint::Offline))
    );
}

#[test]
fn unreadable_output_fails_the_answer() {
    let (_root, events, assistant) = setup();
    script(&assistant, "c5", &["out this is not json", "sleep 30000"]);
    let started = Instant::now();
    assistant.start(answer("c5", fake_cli())).expect("start");
    let (outcome, error) = done(&events.until_done("c5"));
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(outcome, Outcome::Failed);
    assert_eq!(error, Some(("this is not json".into(), Hint::Unexpected)));
}

#[test]
fn a_permission_prompt_fails_the_answer() {
    let (_root, events, assistant) = setup();
    script(
        &assistant,
        "c6",
        &[
            r#"out {"type":"result","subtype":"success","is_error":false,"result":"I need permission","session_id":"s","permission_denials":[{"tool_name":"Bash","tool_use_id":"t","tool_input":{}}]}"#,
        ],
    );
    assistant.start(answer("c6", fake_cli())).expect("start");
    let (outcome, error) = done(&events.until_done("c6"));
    assert_eq!(outcome, Outcome::Failed);
    assert_eq!(error.map(|e| e.1), Some(Hint::Permission));
}

#[test]
fn a_chat_folder_that_cannot_be_written_starts_nothing() {
    let root = tempfile::tempdir().expect("tempdir");
    let blocker = root.path().join("chats");
    std::fs::write(&blocker, "a file where the folder should be").expect("file");
    let events = Arc::new(Events::default());
    let sink: Arc<dyn Sink> = events.clone();
    let assistant = Assistant::new(blocker, sink);
    let err = assistant
        .start(answer("c7", fake_cli()))
        .expect_err("no folder");
    assert!(err.contains("folder"), "{err}");
    assert!(!assistant.running("c7"));
    assert!(events.seen.lock().expect("events").is_empty());
}

#[test]
fn purge_removes_folders_left_by_a_crashed_run() {
    let (_root, _events, assistant) = setup();
    let left = assistant.chat_dir("old");
    std::fs::create_dir_all(&left).expect("dir");
    std::fs::write(left.join("mcp.json"), "{}").expect("file");
    assistant.purge();
    assert!(!left.exists());
}

/// Links `link` to the folder `target`: a symlink, or on Windows a
/// junction, which needs no elevation.
fn link_dir(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).expect("symlink");
    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .status()
            .expect("mklink");
        assert!(status.success(), "mklink /J failed");
    }
}

#[test]
fn a_linked_chat_folder_is_refused_and_nothing_is_written_through_it() {
    let (root, events, assistant) = setup();
    let elsewhere = root.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("dir");
    std::fs::create_dir_all(assistant.root()).expect("root");
    link_dir(&elsewhere, &assistant.chat_dir("c8"));
    let err = assistant
        .start(answer("c8", fake_cli()))
        .expect_err("a linked folder");
    assert!(err.contains("folder"), "{err}");
    assert_eq!(
        std::fs::read_dir(&elsewhere).expect("dir").count(),
        0,
        "nothing written through the link"
    );
    assert!(!assistant.running("c8"));
    assert!(events.seen.lock().expect("events").is_empty());
}

#[test]
fn the_chat_token_file_goes_when_the_answer_ends_and_comes_back_with_the_next() {
    let (_root, events, assistant) = setup();
    let lines = [
        r#"out {"type":"result","subtype":"success","is_error":false,"result":"","session_id":"s"}"#,
    ];
    script(&assistant, "c9", &lines);
    let config = assistant.chat_dir("c9").join(claude::MCP_CONFIG);
    assistant.start(answer("c9", fake_cli())).expect("start");
    assert_eq!(done(&events.until_done("c9")), (Outcome::Finished, None));
    assert!(
        !config.exists(),
        "the token stays on disk only while an answer runs"
    );
    events.seen.lock().expect("events").clear();
    assistant
        .start(answer("c9", fake_cli()))
        .expect("second answer");
    assert_eq!(done(&events.until_done("c9")), (Outcome::Finished, None));
    assert!(recorded(&assistant, "c9", "mcp.json").contains("tok-123"));
}

#[cfg(unix)]
#[test]
fn the_chat_folder_and_its_token_file_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let (_root, events, assistant) = setup();
    // The root already exists with the usual 0o755 (here: from `script`):
    // it is made private too, not only a folder created now.
    script(&assistant, "c10", &["sleep 30000"]);
    assistant.start(answer("c10", fake_cli())).expect("start");
    let mode = |p: &Path| std::fs::metadata(p).expect("meta").permissions().mode() & 0o777;
    assert_eq!(mode(assistant.root()), 0o700);
    assert_eq!(mode(&assistant.chat_dir("c10")), 0o700);
    assert_eq!(
        mode(&assistant.chat_dir("c10").join(claude::MCP_CONFIG)),
        0o600
    );
    assistant.stop("c10");
    events.until_done("c10");
}

#[test]
fn chat_folders_live_in_the_users_app_data_not_the_shared_temp_dir() {
    let data = Path::new("C:/Users/u/AppData/Local/pincel");
    assert_eq!(
        super::chats_root(Some(data.to_path_buf())),
        data.join("assistant")
    );
    let fallback = super::chats_root(None);
    assert_ne!(fallback, std::env::temp_dir().join("pincel-assistant"));
    assert_ne!(
        fallback,
        super::chats_root(None),
        "a fallback is unique per start"
    );
}
