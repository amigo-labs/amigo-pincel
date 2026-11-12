//! The Codex CLI, Gemini CLI and opencode adapters, the version check and
//! the opencode model list (docs/specs/ai-chat-clis.md), against recorded
//! output and the fake CLI of `tests.rs`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::adapter::{self, Kind, Stream, classify, is_local};
use super::find::CLIS;
use super::probe::{self, Model};
use super::run::Answer;
use super::tests::{answer, done, fake_cli, recorded, script, setup};
use super::{ChatEvent, Hint, Outcome, codex, gemini, opencode};

fn answer_of(kind: Kind, chat: &str) -> Answer {
    Answer {
        kind,
        ..answer(chat, fake_cli())
    }
}

fn has(args: &[String], seq: &[&str]) -> bool {
    args.windows(seq.len()).any(|w| w == seq)
}

fn file<'a>(launch: &'a adapter::Launch, name: &str) -> &'a str {
    launch
        .files
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, text)| text.as_str())
        .unwrap_or_else(|| panic!("no file {name}"))
}

fn lines_of(stream: &mut dyn Stream, lines: &[&str]) -> Vec<ChatEvent> {
    lines
        .iter()
        .flat_map(|l| stream.line(l).expect("readable"))
        .collect()
}

#[test]
fn clis_are_listed_in_the_picker_order_with_install_pages_and_minimum_versions() {
    let ids: Vec<&str> = CLIS.iter().map(|c| c.id).collect();
    assert_eq!(ids, ["claude", "codex", "gemini", "opencode"]);
    let pages: Vec<&str> = CLIS.iter().map(|c| c.install_url).collect();
    assert_eq!(
        pages[1..],
        [
            "https://github.com/openai/codex",
            "https://github.com/google-gemini/gemini-cli",
            "https://opencode.ai",
        ]
    );
    for c in CLIS {
        assert!(c.min_version > [0, 0, 0], "{} has a minimum version", c.id);
    }
}

#[test]
fn codex_launch_offers_only_pincel_and_web_search_and_resumes_the_session() {
    let a = answer_of(Kind::Codex, "c1");
    let launch = adapter::launch(&a, Path::new("chat-dir"));
    let args = &launch.args;
    assert_eq!(args[0], "exec");
    for seq in [
        &["--json"][..],
        &["--ignore-user-config"],
        &["--ignore-rules"],
        &["--skip-git-repo-check"],
        &["--sandbox", "read-only"],
        &["-c", "approval_policy=\"never\""],
        &["-c", "web_search=\"live\""],
        &[
            "-c",
            "mcp_servers.pincel.url=\"http://127.0.0.1:39217/mcp\"",
        ],
        &[
            "-c",
            "mcp_servers.pincel.bearer_token_env_var=\"PINCEL_MCP_TOKEN\"",
        ],
        &["-c", "include_apply_patch_tool=false"],
        &["-c", "features.shell_tool=false"],
        &["-c", "features.unified_exec=false"],
        &["-c", "features.view_image=false"],
        &["-m", "opus"],
    ] {
        assert!(has(args, seq), "{seq:?} missing in {args:?}");
    }
    // The system prompt as one TOML string: no line break in an argument.
    assert!(has(
        args,
        &[
            "-c",
            "developer_instructions=\"Du bist „PixelAssist“.\\nZeile 2\""
        ]
    ));
    assert!(
        args.iter()
            .all(|a| !a.contains('\n') && !a.contains("tok-123"))
    );
    // AC-07: the chat's session, then the prompt from stdin.
    assert_eq!(args[args.len() - 3..], ["resume", "sess-1", "-"]);
    assert_eq!(
        launch.env,
        [("PINCEL_MCP_TOKEN".to_owned(), "tok-123".to_owned())]
    );
    let fresh = Answer {
        session: None,
        model: None,
        ..a
    };
    let args = adapter::launch(&fresh, Path::new("chat-dir")).args;
    assert!(!args.iter().any(|a| a == "resume" || a == "-m"));
    assert_eq!(args.last().map(String::as_str), Some("-"));
    assert_eq!(
        codex::toml_str("a\"b\\c\td\u{1}"),
        "\"a\\\"b\\\\c\\td\\u0001\""
    );
}

#[test]
fn gemini_launch_offers_only_pincel_and_web_search_and_resumes_the_session() {
    let dir = Path::new("chat-dir");
    let launch = adapter::launch(&answer_of(Kind::Gemini, "c1"), dir);
    let args = &launch.args;
    for seq in [
        &["--output-format", "stream-json"][..],
        &["--allowed-mcp-server-names", "pincel"],
        &["--extensions", "none"],
        &["-m", "opus"],
        &["--resume", "sess-1"],
    ] {
        assert!(has(args, seq), "{seq:?} missing in {args:?}");
    }
    assert!(
        args.iter()
            .all(|a| !a.contains('\n') && !a.contains("tok-123"))
    );
    let settings_path = dir.join(gemini::SETTINGS).to_string_lossy().into_owned();
    assert_eq!(
        launch.env,
        [(gemini::SETTINGS_ENV.to_owned(), settings_path)]
    );
    let settings: Value = serde_json::from_str(file(&launch, gemini::SETTINGS)).expect("json");
    assert_eq!(
        settings["mcpServers"],
        json!({ "pincel": {
            "httpUrl": "http://127.0.0.1:39217/mcp",
            "headers": { "Authorization": "Bearer tok-123" },
            "trust": true,
        } })
    );
    assert_eq!(settings["mcp"]["allowed"], json!(["pincel"]));
    assert_eq!(settings["tools"]["core"], json!(["google_web_search"]));
    let excluded = settings["tools"]["exclude"].as_array().expect("exclude");
    for tool in [
        "run_shell_command",
        "read_file",
        "write_file",
        "web_fetch",
        "glob",
    ] {
        assert!(excluded.contains(&json!(tool)), "{tool} not excluded");
    }
    assert_eq!(settings["security"]["folderTrust"]["enabled"], false);
    assert_eq!(
        file(&launch, gemini::CONTEXT),
        "Du bist „PixelAssist“.\nZeile 2"
    );
    assert_eq!(launch.secret, [gemini::SETTINGS]);
}

#[test]
fn opencode_launch_offers_only_pincel_and_web_search_and_resumes_the_session() {
    let dir = Path::new("chat-dir");
    let a = Answer {
        model: Some("ollama/llama3.2".to_owned()),
        ..answer_of(Kind::Opencode, "c1")
    };
    let launch = adapter::launch(&a, dir);
    assert_eq!(
        launch.args,
        [
            "run",
            "--format",
            "json",
            "-m",
            "ollama/llama3.2",
            "--session",
            "sess-1"
        ]
    );
    let config_path = dir.join(opencode::CONFIG).to_string_lossy().into_owned();
    assert_eq!(
        launch.env,
        [
            (opencode::CONFIG_ENV.to_owned(), config_path),
            (opencode::NO_PROJECT_ENV.to_owned(), "1".to_owned()),
        ]
    );
    let text = file(&launch, opencode::CONFIG);
    let config: Value = serde_json::from_str(text).expect("json");
    assert_eq!(config["mcp"]["pincel"]["type"], "remote");
    assert_eq!(config["mcp"]["pincel"]["url"], "http://127.0.0.1:39217/mcp");
    assert_eq!(
        config["mcp"]["pincel"]["headers"]["Authorization"],
        "Bearer tok-123"
    );
    assert_eq!(
        config["permission"],
        json!({ "*": "deny", "pincel_*": "allow", "websearch": "allow" })
    );
    // opencode lets the last matching rule win: the catch-all comes first.
    let at = |key: &str| text.find(key).expect(key);
    assert!(at("\"*\":\"deny\"") < at("\"pincel_*\":\"allow\""));
    assert_eq!(
        config["instructions"],
        json!([dir.join(opencode::SYSTEM_PROMPT).to_string_lossy()])
    );
    assert_eq!(
        file(&launch, opencode::SYSTEM_PROMPT),
        "Du bist „PixelAssist“.\nZeile 2"
    );
    assert_eq!(launch.secret, [opencode::CONFIG]);
    let fresh = Answer {
        session: None,
        model: None,
        ..a
    };
    assert_eq!(
        adapter::launch(&fresh, dir).args,
        ["run", "--format", "json"]
    );
}

#[test]
fn codex_json_becomes_session_text_and_tool_events() {
    let mut p = adapter::stream(Kind::Codex);
    let events = lines_of(
        p.as_mut(),
        &[
            r#"{"type":"thread.started","thread_id":"th-1"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"thinking"}}"#,
            r#"{"type":"item.started","item":{"id":"item_1","type":"mcp_tool_call","server":"pincel","tool":"draw.rect","arguments":{"x0":1},"result":null,"error":null,"status":"in_progress"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_1","type":"mcp_tool_call","server":"pincel","tool":"draw.rect","arguments":{"x0":1},"result":{"content":[{"type":"text","text":"ok"}],"structured_content":null},"error":null,"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_2","type":"mcp_tool_call","server":"pincel","tool":"doc.save","arguments":{},"result":null,"error":{"message":"denied"},"status":"failed"}}"#,
            r#"{"type":"item.started","item":{"id":"item_3","type":"web_search","query":"pixel art"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_3","type":"web_search","query":"pixel art"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_4","type":"agent_message","text":"Drawn."}}"#,
            r#"{"type":"item.completed","item":{"id":"item_5","type":"agent_message","text":"Anything else?"}}"#,
            r#"{"type":"something.new"}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":1}}"#,
        ],
    );
    assert_eq!(
        events,
        [
            ChatEvent::Session { id: "th-1".into() },
            ChatEvent::ToolStart {
                id: "item_1".into(),
                name: "draw.rect".into(),
                args: json!({ "x0": 1 }),
            },
            ChatEvent::ToolEnd {
                id: "item_1".into(),
                ok: true,
                text: "ok".into(),
                images: vec![],
            },
            ChatEvent::ToolStart {
                id: "item_2".into(),
                name: "doc.save".into(),
                args: json!({}),
            },
            ChatEvent::ToolEnd {
                id: "item_2".into(),
                ok: false,
                text: "denied".into(),
                images: vec![],
            },
            ChatEvent::ToolStart {
                id: "item_3".into(),
                name: "web_search".into(),
                args: json!({ "query": "pixel art" }),
            },
            ChatEvent::ToolEnd {
                id: "item_3".into(),
                ok: true,
                text: String::new(),
                images: vec![],
            },
            ChatEvent::Text {
                text: "Drawn.".into()
            },
            ChatEvent::Text {
                text: "\n\nAnything else?".into()
            },
        ]
    );
    assert!(p.finished_ok());
    assert!(p.line("Error: not json").is_err());
    let mut failed = adapter::stream(Kind::Codex);
    lines_of(
        failed.as_mut(),
        &[
            r#"{"type":"error","message":"stream disconnected"}"#,
            r#"{"type":"turn.failed","error":{"message":"unexpected status 401 Unauthorized"}}"#,
        ],
    );
    assert!(!failed.finished_ok());
    assert_eq!(
        failed.error_text(),
        Some("unexpected status 401 Unauthorized")
    );
}

#[test]
fn gemini_stream_json_becomes_session_text_and_tool_events() {
    let mut p = adapter::stream(Kind::Gemini);
    let events = lines_of(
        p.as_mut(),
        &[
            r#"{"type":"init","timestamp":"t","session_id":"g-1","model":"gemini-3-pro"}"#,
            r#"{"type":"message","timestamp":"t","role":"user","content":"Draw it"}"#,
            r#"{"type":"message","timestamp":"t","role":"assistant","content":"On ","delta":true}"#,
            r#"{"type":"message","timestamp":"t","role":"assistant","content":"it.","delta":true}"#,
            r#"{"type":"tool_use","timestamp":"t","tool_name":"draw.rect","tool_id":"t1","parameters":{"x0":1}}"#,
            r#"{"type":"tool_result","timestamp":"t","tool_id":"t1","status":"success","output":"ok"}"#,
            r#"{"type":"tool_use","timestamp":"t","tool_name":"mcp_pincel_doc_info","tool_id":"t2","parameters":{}}"#,
            r#"{"type":"tool_result","timestamp":"t","tool_id":"t2","status":"error","error":{"type":"x","message":"no document"}}"#,
            r#"{"type":"error","timestamp":"t","severity":"warning","message":"slow"}"#,
            r#"{"type":"result","timestamp":"t","status":"success","stats":{}}"#,
        ],
    );
    assert_eq!(
        events,
        [
            ChatEvent::Session { id: "g-1".into() },
            ChatEvent::Text { text: "On ".into() },
            ChatEvent::Text { text: "it.".into() },
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
            ChatEvent::ToolStart {
                id: "t2".into(),
                name: "doc.info".into(),
                args: json!({}),
            },
            ChatEvent::ToolEnd {
                id: "t2".into(),
                ok: false,
                text: "no document".into(),
                images: vec![],
            },
        ]
    );
    assert!(p.finished_ok());
    let mut failed = adapter::stream(Kind::Gemini);
    lines_of(
        failed.as_mut(),
        &[
            r#"{"type":"error","timestamp":"t","severity":"error","message":"quota"}"#,
            r#"{"type":"result","timestamp":"t","status":"error","error":{"type":"api","message":"Model not found: gemini-9"}}"#,
        ],
    );
    assert!(!failed.finished_ok());
    assert_eq!(failed.error_text(), Some("Model not found: gemini-9"));
}

#[test]
fn opencode_json_becomes_session_text_and_tool_events() {
    let mut p = adapter::stream(Kind::Opencode);
    let events = lines_of(
        p.as_mut(),
        &[
            r#"{"type":"step_start","timestamp":1,"sessionID":"ses_1","part":{"type":"step-start"}}"#,
            r#"{"type":"tool_use","timestamp":2,"sessionID":"ses_1","part":{"type":"tool","callID":"call_1","tool":"pincel_draw_rect","state":{"status":"completed","input":{"x0":1},"output":"ok","title":"draw"}}}"#,
            r#"{"type":"tool_use","timestamp":3,"sessionID":"ses_1","part":{"type":"tool","callID":"call_2","tool":"pincel_doc_save","state":{"status":"error","input":{},"error":"denied"}}}"#,
            r#"{"type":"text","timestamp":4,"sessionID":"ses_1","part":{"type":"text","text":"Drawn."}}"#,
            r#"{"type":"step_finish","timestamp":5,"sessionID":"ses_1","part":{"type":"step-finish","reason":"stop"}}"#,
        ],
    );
    assert_eq!(
        events,
        [
            ChatEvent::Session { id: "ses_1".into() },
            ChatEvent::ToolStart {
                id: "call_1".into(),
                name: "draw.rect".into(),
                args: json!({ "x0": 1 }),
            },
            ChatEvent::ToolEnd {
                id: "call_1".into(),
                ok: true,
                text: "ok".into(),
                images: vec![],
            },
            ChatEvent::ToolStart {
                id: "call_2".into(),
                name: "doc.save".into(),
                args: json!({}),
            },
            ChatEvent::ToolEnd {
                id: "call_2".into(),
                ok: false,
                text: "denied".into(),
                images: vec![],
            },
            ChatEvent::Text {
                text: "Drawn.".into()
            },
        ]
    );
    assert!(p.finished_ok());
    let mut failed = adapter::stream(Kind::Opencode);
    lines_of(
        failed.as_mut(),
        &[
            r#"{"type":"error","timestamp":1,"sessionID":"ses_2","error":{"name":"APIError","data":{"message":"Cannot connect to API: connect ECONNREFUSED 127.0.0.1:11434"}}}"#,
        ],
    );
    assert!(!failed.finished_ok());
    assert_eq!(
        failed.error_text(),
        Some("Cannot connect to API: connect ECONNREFUSED 127.0.0.1:11434")
    );
}

/// One finished answer per CLI: the recorded output streams, and the CLI
/// gets its launch's arguments, environment and files, the prompt on
/// stdin; the token file goes when the answer ends.
#[test]
fn every_cli_runs_an_answer_with_its_launch_and_streams_it() {
    let cases: [(Kind, &[&str], &str); 3] = [
        (
            Kind::Codex,
            &[
                r#"out {"type":"thread.started","thread_id":"th-1"}"#,
                r#"out {"type":"item.completed","item":{"id":"i","type":"agent_message","text":"Hi"}}"#,
                r#"out {"type":"turn.completed","usage":{}}"#,
            ],
            "th-1",
        ),
        (
            Kind::Gemini,
            &[
                r#"out {"type":"init","timestamp":"t","session_id":"g-1","model":"m"}"#,
                r#"out {"type":"message","timestamp":"t","role":"assistant","content":"Hi","delta":true}"#,
                r#"out {"type":"result","timestamp":"t","status":"success"}"#,
            ],
            "g-1",
        ),
        (
            Kind::Opencode,
            &[
                r#"out {"type":"text","timestamp":1,"sessionID":"ses_1","part":{"type":"text","text":"Hi"}}"#,
                r#"out {"type":"step_finish","timestamp":2,"sessionID":"ses_1","part":{"type":"step-finish","reason":"stop"}}"#,
            ],
            "ses_1",
        ),
    ];
    for (kind, lines, session) in cases {
        let (_root, events, assistant) = setup();
        let chat = format!("{kind:?}").to_lowercase();
        script(&assistant, &chat, lines);
        let a = answer_of(kind, &chat);
        assistant.start(a.clone()).expect("start");
        let seen = events.until_done(&chat);
        assert_eq!(
            seen[..2],
            [
                ChatEvent::Session { id: session.into() },
                ChatEvent::Text { text: "Hi".into() }
            ],
            "{kind:?}"
        );
        assert_eq!(done(&seen), (Outcome::Finished, None), "{kind:?}");
        let launch = adapter::launch(&a, &assistant.chat_dir(&chat));
        let args: Vec<String> =
            serde_json::from_str(&recorded(&assistant, &chat, "args.json")).expect("args");
        assert_eq!(args, launch.args, "{kind:?}");
        assert_eq!(recorded(&assistant, &chat, "stdin.txt"), a.prompt);
        let env: Value =
            serde_json::from_str(&recorded(&assistant, &chat, "env.json")).expect("env");
        for (k, v) in &launch.env {
            assert_eq!(env[k], json!(v), "{kind:?} {k}");
        }
        for (name, text) in &launch.files {
            assert_eq!(
                &recorded(&assistant, &chat, &format!("files/{name}")),
                text,
                "{kind:?} {name}"
            );
        }
        for name in &launch.secret {
            assert!(
                !assistant.chat_dir(&chat).join(name).exists(),
                "{kind:?}: {name} outlived the answer"
            );
        }
    }
}

#[test]
fn local_model_failures_name_the_model_server_or_missing_tool_calls() {
    assert!(is_local(Kind::Opencode, Some("ollama/llama3.2")));
    assert!(is_local(Kind::Opencode, Some("lmstudio/qwen3")));
    assert!(!is_local(Kind::Opencode, Some("anthropic/claude-sonnet-5")));
    assert!(!is_local(Kind::Opencode, None));
    assert!(!is_local(Kind::Codex, Some("ollama/llama3.2")));
    let refused = "Cannot connect to API: connect ECONNREFUSED 127.0.0.1:11434";
    assert_eq!(
        classify(refused, Some("ollama/llama3.2"), true),
        Hint::LocalServer
    );
    assert_eq!(
        classify(refused, Some("openai/gpt-5"), false),
        Hint::Offline
    );
    assert_eq!(
        classify(
            "registry.ollama.ai/library/gemma:2b does not support tools",
            Some("ollama/gemma:2b"),
            true
        ),
        Hint::NoTools
    );
    assert_eq!(
        classify("unexpected status 401 Unauthorized", None, false),
        Hint::SignIn
    );
    // End to end: opencode with an Ollama model whose server is down.
    let (_root, events, assistant) = setup();
    script(
        &assistant,
        "l1",
        &[
            r#"out {"type":"error","timestamp":1,"sessionID":"ses_1","error":{"name":"APIError","data":{"message":"Cannot connect to API: connect ECONNREFUSED 127.0.0.1:11434"}}}"#,
            "exit 1",
        ],
    );
    let a = Answer {
        model: Some("ollama/llama3.2".to_owned()),
        ..answer_of(Kind::Opencode, "l1")
    };
    assistant.start(a).expect("start");
    assert_eq!(
        done(&events.until_done("l1")),
        (
            Outcome::Failed,
            Some((refused.to_owned(), Hint::LocalServer))
        )
    );
}

#[test]
fn versions_are_read_from_each_clis_version_output() {
    assert_eq!(
        probe::parse_version("2.1.322 (Claude Code)"),
        Some([2, 1, 322])
    );
    assert_eq!(
        probe::parse_version("codex-cli 0.156.1\n"),
        Some([0, 156, 1])
    );
    assert_eq!(probe::parse_version("0.68.0"), Some([0, 68, 0]));
    assert_eq!(probe::parse_version("v1.19.5"), Some([1, 19, 5]));
    assert_eq!(probe::parse_version("version unknown 3.4"), None);
    assert_eq!(probe::version_text([0, 122, 0]), "0.122.0");
    let fake: PathBuf = fake_cli();
    assert_eq!(
        probe::version(&fake, None, &[("FAKE_CLI_VERSION", "codex-cli 0.121.9")]),
        Some([0, 121, 9])
    );
    assert_eq!(
        probe::version(&fake, None, &[("FAKE_CLI_VERSION", "garbage")]),
        None
    );
    assert_eq!(
        probe::version(
            &fake,
            None,
            &[("FAKE_CLI_VERSION", "1.0.0"), ("FAKE_CLI_EXIT", "2")]
        ),
        None,
        "a failing --version is unknown"
    );
}

#[test]
fn opencode_models_are_listed_with_local_ones_marked() {
    let fake = fake_cli();
    let listed =
        "anthropic/claude-sonnet-5\nollama/llama3.2\n\nWarning: cache stale\nlmstudio/qwen3-8b\n";
    let models =
        probe::opencode_models(&fake, None, &[("FAKE_CLI_MODELS", listed)], probe::DEADLINE)
            .expect("models");
    let model = |id: &str, local: bool| Model {
        id: id.to_owned(),
        local,
    };
    assert_eq!(
        models,
        [
            model("anthropic/claude-sonnet-5", false),
            model("ollama/llama3.2", true),
            model("lmstudio/qwen3-8b", true),
        ]
    );
    assert!(
        probe::opencode_models(&fake, None, &[("FAKE_CLI_EXIT", "1")], probe::DEADLINE).is_err()
    );
}

#[test]
fn a_hung_cli_is_ended_at_the_deadline() {
    // Times the deadline alone (crate::test_support).
    let _timing = crate::test_support::timing();
    let started = Instant::now();
    let err = probe::opencode_models(
        &fake_cli(),
        None,
        &[("FAKE_CLI_SLEEP_MS", "30000")],
        Duration::from_millis(300),
    )
    .expect_err("hung");
    assert!(err.contains("in time"), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "{:?}",
        started.elapsed()
    );
}
