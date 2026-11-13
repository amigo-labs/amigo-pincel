---
status: todo
created: 2026-11-06
size: M
mode: both
design: [pincel.md §15 (2026-11-06 PixelAssist)]
depends_on: [ai-chat-panel]
---

# AI chat: more agent CLIs

## Context

`ai-chat-panel` ships the Assistant panel with one CLI, Claude Code. The owner wants to pick other agent CLIs as well, including one that runs **local models** ("I choose my local LLM in the UI"). This spec adds three adapters: OpenAI Codex CLI (`codex`), Gemini CLI (`gemini`) and opencode (`opencode`), which can drive local models served by Ollama or LM Studio.

Every adapter must meet the panel's contract: Pincel's tools only plus web search (`ai-chat-panel` AC-16), streamed text and tool rows, session resume for follow-ups, and an error message when the CLI fails. The four CLIs are the agent CLIs with MCP support and a non-interactive mode the owner uses or named; opencode is the one that runs local models.

## Scope

### In scope

- Adapters for `codex`, `gemini` and `opencode` meeting `ai-chat-panel` AC-05 – AC-28
- The opencode model list (including local Ollama / LM Studio models) in the model field
- Local vs cloud classification per selected model, which decides the image switch (`ai-chat-panel` AC-17)
- Refusing a CLI whose installed version cannot restrict its tools

### Out of scope

- Further CLIs (aider, goose, …) — because each needs its own adapter; added on demand
- Installing or signing into a CLI from Pincel — because each CLI's installer and login are its own
- A built-in model server — because opencode reaches Ollama / LM Studio, which the user runs

## Acceptance criteria

- [ ] AC-01 (always): The CLI picker offers `codex`, `gemini` and `opencode` in addition to `claude` when they are installed (`ai-chat-panel` AC-03), in the order `claude`, `codex`, `gemini`, `opencode`.
- [ ] AC-02 (event): When the user sends a message with `codex`, `gemini` or `opencode` selected, then the reply streams into the chat, tool calls show as rows, and the answer's changes form one undo step, as with `claude` (`ai-chat-panel` AC-05 – AC-09).
- [ ] AC-03 (always): Pincel starts each of the three CLIs so that its only tools are the chat's Pincel session and web search, as `ai-chat-panel` AC-16 requires; a CLI without web search runs with Pincel's tools only.
- [ ] AC-04 (error): If the installed version of any supported CLI, `claude` included (its `--version` output), is older than the minimum version Pincel records for that CLI — the first version offering the restrictions of AC-03, streamed JSON output and session resume — or cannot be read, then the picker shows it disabled with "Version X is too old — Pincel needs Y or newer" (or "Version unknown"), and no message can be sent with it; while every installed CLI is disabled, the message input is disabled.
- [ ] AC-05 (event): When the user opens the model field with `opencode` selected, then it lists the models opencode is configured for, models served by Ollama or LM Studio marked "local"; if the list cannot be read within 5 s (a hung CLI must not block the panel), the field accepts free text.
- [ ] AC-06 (state): While the selected model is local (served by Ollama or LM Studio), the "Agent may see the image" switch is hidden and the pixel-returning tools are available; with every other model, `ai-chat-panel` AC-17 applies.
- [ ] AC-07 (event): When the user sends a follow-up message with any of the three CLIs, then the CLI continues this chat's session, so the agent has the earlier messages as context.
- [ ] AC-08 (error): If the CLI reports that a local model's server is not reachable (Ollama or LM Studio not running), then the chat shows the CLI's error plus "Start Ollama / LM Studio and try again".
- [ ] AC-09 (state, manual: needs each CLI signed in and a real model): For each of `codex`, `gemini` and `opencode` with a local model, the checks of `ai-chat-panel` AC-24 pass (rectangle, Undo, follow-up recolor, shell request refused).

## Technical notes

Fake CLIs per adapter (recorded JSON streams) cover AC-01 – AC-08 as in `ai-chat-panel`. Flags drift between releases; read each CLI's `--help` when building and record the minimum versions in the adapter.

- **codex:** `codex exec --json`, MCP server and tool approval via `-c` config overrides; its built-in shell and file tools must be switched off — a read-only sandbox alone still allows shell commands and file reads, so a version that cannot switch them off is too old (AC-04); resume per its session id.
- **gemini:** `gemini -p … --output-format stream-json`, MCP server and allowed tools via a settings file in the chat's temp directory; resume per its session id.
- **opencode:** `opencode run --format json`, MCP server, permissions and model via a config file in the chat's temp directory; `opencode models` for the list; provider prefix (`ollama/`, `lmstudio/`) marks local models.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Local model without tool-calling support | The CLI's error is shown; when it says the model cannot call tools, the hint reads "This model cannot call tools — pick another" |
| opencode configured with a cloud provider (e.g. `anthropic/…`) | Counts as cloud: image switch shown, off by default |
| Two CLIs installed, last choice uninstalled | The picker selects the first CLI found; the stored choice is kept for when it returns |
| Install pages | Codex CLI `https://github.com/openai/codex`, Gemini CLI `https://github.com/google-gemini/gemini-cli`, opencode `https://opencode.ai` (shown by `ai-chat-panel` AC-03) |
| `opencode models` takes long or fails | The model field falls back to free text with the CLI's default as placeholder |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A model is local exactly when its opencode provider is Ollama or LM Studio | Those are the local servers the owner named; other providers are remote APIs | A self-hosted remote Ollama counts as local |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `src-tauri/src/assistant/tests_clis.rs` `clis_are_listed_in_the_picker_order_with_install_pages_and_minimum_versions`; `ui/e2e/assistant-clis.spec.ts` "AC-01 AC-04 the picker lists the CLIs in order; …" | built  |        |
| AC-02 | `src-tauri/src/assistant/tests_clis.rs` `codex_json_becomes_session_text_and_tool_events`, `gemini_stream_json_becomes_session_text_and_tool_events`, `opencode_json_becomes_session_text_and_tool_events`, `every_cli_runs_an_answer_with_its_launch_and_streams_it`; `ui/e2e/assistant-clis.spec.ts` "AC-02 AC-07 a codex / gemini / opencode answer streams, …" | built  |        |
| AC-03 | `src-tauri/src/assistant/tests_clis.rs` `codex_launch_offers_only_pincel_and_web_search_and_resumes_the_session`, `gemini_launch_…`, `opencode_launch_…`, `every_cli_runs_an_answer_with_its_launch_and_streams_it` | built  |        |
| AC-04 | `src-tauri/src/assistant/tests_clis.rs` `versions_are_read_from_each_clis_version_output`, `clis_are_listed_in_the_picker_order_with_install_pages_and_minimum_versions`; `ui/e2e/assistant-clis.spec.ts` "AC-01 AC-04 …" | built  |        |
| AC-05 | `src-tauri/src/assistant/tests_clis.rs` `opencode_models_are_listed_with_local_ones_marked`, `a_hung_cli_is_ended_at_the_deadline`; `ui/e2e/assistant-clis.spec.ts` "AC-05 AC-06 opencode lists its models, …", "AC-05 a model list that fails or takes over 5 s …" | built  |        |
| AC-06 | `ui/e2e/assistant-clis.spec.ts` "AC-05 AC-06 …"; `src-tauri/src/assistant/tests_clis.rs` `local_model_failures_name_the_model_server_or_missing_tool_calls` (`is_local`) | built  |        |
| AC-07 | `src-tauri/src/assistant/tests_clis.rs` the three `…_launch_offers_only_pincel_and_web_search_and_resumes_the_session`; `ui/e2e/assistant-clis.spec.ts` "AC-02 AC-07 …" | built  |        |
| AC-08 | `src-tauri/src/assistant/tests_clis.rs` `local_model_failures_name_the_model_server_or_missing_tool_calls`; `ui/e2e/assistant-clis.spec.ts` "AC-08 a local model whose server is down, …" | built  |        |
| AC-09 |      | open   |        |

## Decisions

- 2026-11-06: First CLIs: Claude Code (in `ai-chat-panel`), Codex CLI, Gemini CLI, opencode — the three big vendors' agent CLIs plus one that runs local models (owner).
- 2026-11-06: The model is picked per CLI in the panel; opencode lists its models, local ones marked (owner: "I choose my local LLM in the UI").
- 2026-11-12 (spec-build): Flags as built: Codex CLI checked against `codex exec --help` 0.156.1 on the build machine; Gemini CLI (0.68) and opencode (1.19.5) were not installed there, so their flags and output come from public sources: Gemini CLI's `packages/core/src/output/types.ts` and configuration reference, opencode's `run.ts` and V1 config schema at tag `v1.19.5`. Each adapter's module doc names them; AC-09 checks them with the real CLIs.
- 2026-11-12 (spec-build): Minimum versions (AC-04), the oldest release whose notes show every flag the adapter uses: Claude Code 2.1.69 (oldest changelog entry naming `--append-system-prompt-file`; `--tools` worked in print mode before 2.1.0), Codex CLI 0.122.0 (adds `--ignore-user-config` and `--ignore-rules`), Gemini CLI 0.34.0 (oldest release notes in reach naming `--resume` and `stream-json`: a conservative bound, older releases may work), opencode 1.1.1 (the permission rules that replaced `tools`). The send itself does not ask the CLI's version again — the panel disables a CLI that is too old, and asking would start the CLI twice per message.
- 2026-11-12 (spec-build): Codex CLI runs `exec --json --ignore-user-config --ignore-rules --skip-git-repo-check --sandbox read-only` with `-c` overrides: approval `never`, web search `live`, the server `pincel` with the token in the environment variable `PINCEL_MCP_TOKEN` (`bearer_token_env_var`, never an argument or a file), a 600 s tool timeout (the default is 60 s, a call may wait for a dialog), `include_apply_patch_tool=false` and every built-in feature that adds a tool switched off (shell, unified exec, view image, browser, computer use, image generation, sub-agents, apps, plugins, memories, …). The system prompt is `developer_instructions` as one TOML string, so no argument carries a line break through a `.cmd` shim. Assumption: a version that lacks one of the listed features ignores its `features.<name>=false`.
- 2026-11-12 (spec-build): Gemini CLI gets its settings through `GEMINI_CLI_SYSTEM_SETTINGS_PATH` — system settings override the user's and the workspace's and are not subject to folder trust, which is switched off: the server `pincel` (`httpUrl`, the token header, `trust: true` so no confirmation is asked), `mcp.allowed`, `tools.core: [google_web_search]` plus an exclude list of the shell, file, fetch, memory and sub-agent tools; arguments `--allowed-mcp-server-names pincel --extensions none`. The prompt goes to stdin (headless mode on a non-TTY stdin) instead of `-p`, and the system prompt to the chat folder's `GEMINI.md` context file — `GEMINI_SYSTEM_MD` would replace Gemini CLI's own tool instructions. Assumption: `tools.core` limits built-in tools only, not MCP tools.
- 2026-11-12 (spec-build): opencode gets a config file through `OPENCODE_CONFIG` with project configs off (`OPENCODE_DISABLE_PROJECT_CONFIG`): the remote server `pincel` with a 600 s timeout (the default is 5 s), the permissions `{"*": "deny", "pincel_*": "allow", "websearch": "allow"}` (last match wins) and the system prompt file as `instructions`. MCP servers of the user's global config still connect, but their tools are denied — opencode has no switch to leave them out. opencode 2 (`opencode2`, V2 config) is a separate program and not supported. Its JSON shows a tool only once it finished, so the tool row's start and end arrive together.
- 2026-11-12 (spec-build): "Local" (AC-06) is decided in both the UI and the Rust side from the opencode provider prefix `ollama/` or `lmstudio/`; a CLI's default model counts as cloud. A local chat sends with image access on, and the endpoint now takes a chat's image access from every answer's start instead of only the first. The local-server hint (AC-08) applies only to a local model; the same connection error with a cloud model keeps "No internet connection".
- 2026-11-12 (spec-build): `assistant_models` lists models for opencode only (the other CLIs have no listing command; their field stays free text); the UI also gives up after 5 s, so a list that arrives later is not shown until the field or panel opens again.
- 2026-11-12 (spec-build): Red runs. Rust: the tests of `tests_clis.rs` were written alongside the adapters; red shown by mutation (dropping `--ignore-user-config`, opencode's `*` deny, Gemini CLI's tool limits, the local-server hint and Codex CLI's minimum version): 5 of 12 failed for exactly those reasons. E2e: against a bundle built from the previous UI, 3 of 7 tests of `ui/e2e/assistant-clis.spec.ts` failed (picker labels and disabled entries, model list, hints); the three per-CLI AC-02 / AC-07 tests and the free-text fallback passed at once, because the chat flow does not depend on the CLI and the model field was free text before — they guard that the new CLIs flow through unchanged.
