---
name: qa
description: Runs the project's full QA pass (rustfmt, clippy with -D warnings, cargo test, generated-bindings drift, wasm build, ui and website lint / type-check / build, Playwright e2e) — independent stages in parallel, with exactly the commands CI runs. Use this skill before finishing any coding task, before a commit or PR, and whenever the user wants to know whether "everything is green".
---

# QA

The commands are not here but in `.claude/gates.json` (list `full`). It mirrors `.github/workflows/ci.yml` minus the `pincel-tauri` job (AGENTS.md §10). Green here means green in the pipeline.

## Procedure

1. From the repo root run `node .claude/hooks/gates.mjs full` (Bash with `timeout: 600000`; the wasm build and e2e make it slow, so start it with `run_in_background` and wait for the notification). The script runs every entry of `full`, independent stages at the same time (`needs` in `gates.json`), and prints a table, plus for every red stage its error lines, the `fix` command and the path to the full log.
2. Exit 2 means: `gates.json` is missing or broken. Report it and stop.
3. Fix **all red stages** in the code, then start the run **from scratch** — a fix can break another stage. A skipped stage is not green: it waits for a red one and only runs in the next pass. If the error lines aren't enough, read the log.
4. Where a stage has a `fix` command, it may fix what is auto-fixable (formatting, lint autofix). Start over afterwards anyway.
5. The stage "generated TS bindings current" is red when `cargo test` regenerated `ui/src/modes/image/core/generated/`: the regenerated files belong in the commit — review and include them, don't revert them.

If `.claude/hooks/gates.mjs` is missing, run the entries of `full` **in the given order** one by one, each `command` in its `cwd` (relative to the repo root), stop at the first failure, fix and start over. Don't omit, rephrase or replace a command with an "equivalent" one.

## Forbidden

- Lowering lint levels or suppressing warnings: no `#[allow(…)]` / `#[expect(…)]`, no `@ts-ignore` / `@ts-expect-error` / `@ts-nocheck`, no `eslint-disable`.
- Changing the gate configuration: `.claude/settings.json`, `.claude/hooks/`, `.claude/gates.json`, the CI workflow.
- Deleting, skipping (`#[ignore]`, `test.skip`, `test.fixme`) or weakening tests to make them green.

If a warning really is a false positive after careful review, don't resolve it yourself: name the spot and the reason to the user (AGENTS.md §9).

## Report

Short, as a table (the last run's, without the log column):

| Gate | Command | Result |
| ---- | ------- | ------ |

Plus: what was fixed (file, cause) and what stays open. No "should be green now" — only what the last run actually showed.
