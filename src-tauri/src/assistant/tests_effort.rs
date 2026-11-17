//! The effort option (docs/specs/ai-chat-clis.md AC-10 – AC-15): which CLI
//! offers it from which version, how each adapter passes it, and which
//! text may reach a command line.

use std::path::Path;

use super::adapter::{self, Kind};
use super::effort;
use super::find::{self, CLIS};
use super::run::Answer;
use super::tests::{answer, fake_cli};
use super::{CliStatus, cli_status};

fn launch_with(kind: Kind, effort: Option<&str>) -> Vec<String> {
    let a = Answer {
        kind,
        effort: effort.map(str::to_owned),
        ..answer("c-1", fake_cli())
    };
    adapter::launch(&a, Path::new(".")).args
}

fn has(args: &[String], seq: &[&str]) -> bool {
    args.windows(seq.len()).any(|w| w == seq)
}

fn status_of(id: &str, version: [u32; 3]) -> CliStatus {
    cli_status(find::cli(id).expect("known CLI"), true, Some(version))
}

#[test]
fn launch_with_an_effort_passes_it_in_each_clis_option() {
    assert!(has(
        &launch_with(Kind::Claude, Some("xhigh")),
        &["--effort", "xhigh"]
    ));
    assert!(has(
        &launch_with(Kind::Codex, Some("minimal")),
        &["-c", "model_reasoning_effort=\"minimal\""]
    ));
    assert!(has(
        &launch_with(Kind::Opencode, Some("max")),
        &["--variant", "max"]
    ));
}

#[test]
fn launch_with_default_effort_passes_no_effort_option() {
    let effort_args = |args: &[String]| {
        args.iter()
            .any(|a| a == "--effort" || a == "--variant" || a.starts_with("model_reasoning_effort"))
    };
    for kind in [Kind::Claude, Kind::Codex, Kind::Gemini, Kind::Opencode] {
        assert!(!effort_args(&launch_with(kind, None)), "{kind:?}");
    }
    // Gemini CLI has no effort option: an effort never reaches it.
    let gemini = launch_with(Kind::Gemini, Some("high"));
    assert!(!effort_args(&gemini));
    assert!(!gemini.iter().any(|a| a == "high"));
}

#[test]
fn cli_status_offers_each_clis_levels_from_its_effort_version() {
    let claude = status_of("claude", [2, 1, 72]);
    assert!(claude.effort);
    assert_eq!(
        claude.effort_levels,
        ["low", "medium", "high", "xhigh", "max"]
    );
    let codex = status_of("codex", [0, 122, 0]);
    assert!(codex.effort);
    assert_eq!(
        codex.effort_levels,
        ["minimal", "low", "medium", "high", "xhigh"]
    );
    // Free text: offered, no levels.
    let opencode = status_of("opencode", [1, 1, 1]);
    assert!(opencode.effort);
    assert!(opencode.effort_levels.is_empty());
    assert!(!status_of("gemini", [0, 68, 0]).effort);
}

#[test]
fn cli_status_of_a_version_before_the_effort_option_offers_none() {
    // Supported (AC-04) but older than Claude Code's `--effort`.
    let old = status_of("claude", [2, 1, 70]);
    assert!(old.supported);
    assert!(!old.effort);
    let unknown = cli_status(find::cli("claude").expect("claude"), true, None);
    assert!(!unknown.effort);
    // The other CLIs had their option before their minimum version.
    for c in CLIS.iter().filter(|c| c.id != "claude" && c.id != "gemini") {
        assert!(cli_status(c, true, Some(c.min_version)).effort, "{}", c.id);
    }
}

#[test]
fn of_request_takes_variant_names_and_refuses_other_text() {
    assert_eq!(effort::of_request(None), Ok(None));
    assert_eq!(effort::of_request(Some("  ".to_owned())), Ok(None));
    assert_eq!(
        effort::of_request(Some(" high ".to_owned())),
        Ok(Some("high".to_owned()))
    );
    assert_eq!(
        effort::of_request(Some("x-1_A".to_owned())),
        Ok(Some("x-1_A".to_owned()))
    );
    for bad in [
        "a b",
        "hi\"gh",
        "hi\ngh",
        "--model=x",
        &"a".repeat(33),
        "höch",
    ] {
        assert!(
            effort::of_request(Some(bad.to_owned())).is_err(),
            "{bad:?} must be refused"
        );
    }
    assert!(effort::valid(&"a".repeat(32)));
}
