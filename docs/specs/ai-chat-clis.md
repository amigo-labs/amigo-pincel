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
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |

## Decisions

- 2026-11-06: First CLIs: Claude Code (in `ai-chat-panel`), Codex CLI, Gemini CLI, opencode — the three big vendors' agent CLIs plus one that runs local models (owner).
- 2026-11-06: The model is picked per CLI in the panel; opencode lists its models, local ones marked (owner: "I choose my local LLM in the UI").
