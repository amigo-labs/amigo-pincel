---
status: todo
created: 2026-11-16
size: L
mode: both
design: [pincel.md §9.3, pincel.md §15 (2026-11-06 PixelAssist), pincel.md §15 (2026-11-06 ComfyUI), pincel.md §15 (2026-11-16 Generation reviewer)]
depends_on: [comfyui-generate, comfyui-generate-pixel, ai-chat-clis]
---

# Generation self-improvement (reviewer agent)

## Context

`comfyui-generate` and `comfyui-generate-pixel` let the user generate variants from a prompt in the Generate panel and apply one. How good the variants are depends on the prompt, and the user learns by trial and error what a model needs. The owner wants the generations to get better over time: a **second agent instance** judges the variants, improves the prompt and runs another round when the result is weak, and what it learns is kept as lessons that later generations use (owner, 2026-11-16).

The reviewer is one of the agent CLIs the Assistant panel already starts (`ai-chat-panel`, `ai-chat-clis`). Pincel still contains no LLM client. The reviewer reaches Pincel only through the live MCP endpoint, in a **review session** of its own. In that session it can only read the review job and submit a result: it has no document tools, no web search, no files and no shell (Decision Log 2026-11-16 "Generation reviewer"). Image data goes to a cloud model only after the user allows it.

## Terms

| Term | Meaning |
| ---- | ------- |
| Reviewer | The agent CLI and model chosen for review in the Generate panel, started by Pincel for each review step |
| Review step | One start of the reviewer for one task: a **revision** (prompt revision before round 1), a **judgement** (scoring one round) or a **lesson step** (new lesson list) |
| Round | One generation of the chosen number of variants (`comfyui-generate` AC-05 – AC-07) inside a reviewed generation |
| Reviewed generation | Everything from one click on Generate with Review on: an optional revision, then one or more rounds, until the loop ends (AC-08, AC-10, AC-11) |
| Review settings | Review switch, reviewer CLI and model, threshold and extra rounds — fixed for a reviewed generation when Generate is clicked, like the target box |
| Verdict | What a judgement submits: for each variant of the round a score from 1 to 10 and a reason of at most 200 characters, plus a revised prompt (1–2000 characters) and negative prompt (0–2000 characters) |
| Lesson | A text of 1–200 characters (outer whitespace trimmed) about what helps or hurts a prompt for a model family, e.g. "Add 'flat colors, no gradients' for pixel-art results"; it keeps the date it was last changed and whether the user added or edited it. Two lessons are **equal** when their texts match ignoring letter case |
| Model family | As in `comfyui-generate`: SD 1.5, SDXL or Flux — every checkpoint has exactly one (catalog entry or the user's "Model type", default SDXL) |
| Shown variant | A variant as it would be applied: fitted to the target box, and in Pixel mode scaled and palette-mapped (`comfyui-generate-pixel`) |
| Best score | The highest score among the scored variants of a round |

## Scope

### In scope

- Review controls in the Generate panel of both editors (desktop app only — because the reviewer is a CLI process and the Generate panel itself is desktop only)
- Prompt revision from stored lessons before round 1
- Scoring each round's variants and repeating with a revised prompt until the threshold or the round limit is reached
- Scores and reasons on the variant strip; variants of every round stay selectable
- User feedback per variant (thumbs up / down) and the applied or dropped result as learning signals
- Lessons per model family, written by the reviewer, kept across restarts, viewable and editable in a Lessons dialog
- The review session on the live endpoint with its own tools and token
- Strings in every UI locale

### Out of scope

- Reviewing `gen.image` calls from the chat agent — because the chat agent is an agent itself and can check its result with `doc.preview`; a reviewer inside it would nest two agent loops and slow every call (owner, 2026-11-16)
- Reviewing chat answers or other document edits — because the owner asked for generations; the Generate panel's variants are the clear object to judge
- Changing model, LoRA, strength, variant count or target box between rounds — because only the prompt is a safe, explainable lever; other parameters change cost and the edited area behind the user's back
- Training or fine-tuning a model from the feedback — because Pincel runs no training; lessons improve prompts, not weights
- Sharing or syncing lessons between machines — because lessons are local app data like other preferences; export can come later
- Editing the reviewer's instructions — because the result format the submit tool checks depends on them, and an edited text could break every review step; a later spec can open them
- A built-in LLM client or API keys — because the reviewer is a CLI the user installed (Decision Log 2026-11-06 "PixelAssist")

## Acceptance criteria

- [ ] AC-01 (always): In the desktop app, the Generate panel shows a "Review" switch (off by default), a reviewer CLI picker listing the installed supported CLIs, a model field with "Default" as placeholder, a score threshold (1–10, default 7) and extra rounds (0–5, default 2); all of them persist across restarts, the model per CLI. If the remembered CLI is no longer installed, the picker shows the first installed one.
- [ ] AC-02 (state): While no supported CLI is installed, the Review switch is off and disabled and shows the hint "Install an agent CLI (Claude Code, Codex CLI, Gemini CLI or opencode) to use review".
- [ ] AC-03 (event): When the user turns Review on, picks another reviewer CLI or model while it is on, or clicks Generate with Review on, and the reviewer model is classified as cloud (`ai-chat-clis`; unknown counts as cloud) without image sending allowed for that CLI and model, then the editor shows the notice "Variants and the input image are sent to the model's provider" with Allow / Cancel; Cancel turns Review off and starts no generation; Allow keeps Review on (and continues the Generate) and is remembered for that CLI and model choice — "Default" counting as its own choice — across restarts.
- [ ] AC-04 (state): While image sending is allowed for the selected cloud reviewer, the panel shows "Images are sent to the provider" with a "Revoke" button; Revoke forgets the permission for that CLI and model and turns Review off, and every running revision or judgement with that reviewer ends; their loops end as in AC-11.
- [ ] AC-05 (state): While Review is off, generating behaves exactly as `comfyui-generate` and `comfyui-generate-pixel` specify: no reviewer starts, no score or thumbs control is shown, and no lesson step starts for that generation.
- [ ] AC-06 (event): When the user clicks Generate with Review on and at least one lesson exists for the chosen model's family, then a revision runs first: the reviewer receives the prompt, the negative prompt, the model family and those lessons and submits a revised prompt and negative prompt; round 1 generates with them, the panel shows the revised prompt below the prompt field while round 1 runs, and the prompt field keeps the user's text. With no lesson for the family, round 1 uses the user's prompt and no revision runs.
- [ ] AC-07 (event): When a round's variants have arrived with Review on, then a judgement runs: the reviewer receives every shown variant of the round, the input and mask if the generation has them, the user's prompt and the round's prompt, and submits a verdict; each thumbnail then shows its score, and selecting a thumbnail shows its reason and the prompt of its round.
- [ ] AC-08 (event): When a round's best score is below the threshold and extra rounds remain, then the next round generates with the verdict's revised prompt and negative prompt and otherwise the same target box, mask, input, model, LoRA, strengths, variant count and seed setting (a set seed stays the same, an empty one is random per round); the panel shows "Round N of M" from round 1 on while the loop runs (also when M is 1), where M is 1 plus the extra rounds; it disappears when the loop ends.
- [ ] AC-09 (event): When a round's best score reaches the threshold, or the last allowed round has been judged, then the loop ends: the strip holds the variants of all rounds sorted by score (highest first; on a tie the earlier round first, because it is closer to the user's own prompt; unscored variants last), and the highest is selected and previewed unless the user selected a variant during the loop, which stays selected. Apply and Discard work as in `comfyui-generate` AC-12 / AC-13.
- [ ] AC-10 (state): While a reviewed generation runs, the strip already shows the variants of finished rounds with their scores, and the user can select, preview and apply one; Apply or Discard then ends the loop, cancelling a running round as in `comfyui-generate` AC-09 and ending a running review step.
- [ ] AC-11 (event): When the user clicks Cancel during a reviewed generation, then the running round is cancelled as in `comfyui-generate` AC-09, the running review step ends, the loop ends, and the variants of finished rounds stay usable with their scores.
- [ ] AC-12 (error): If a revision or judgement fails — the CLI cannot start, exits (with or without an error) before a valid submission, or no valid submission arrives within 3 minutes — or the server fails a round (`comfyui-generate` AC-15), then the loop ends, the panel shows "Review failed: " or the generation error plus the reason, variants already generated stay usable (unscored where no verdict arrived), and Generate works again.
- [ ] AC-13 (error): If the reviewer submits a result that breaks the rules of its step — a verdict that misses a variant of the round, names a variant the round does not have, scores outside 1–10, gives a reason over 200 characters or an empty or over-long revised prompt; a revision with an empty or over-long prompt; a lesson list breaking AC-18; or a result of another step's kind — then the submission fails with `isError: true` and a message naming the problem, nothing of it is used, and the reviewer may submit again within the step's time limit.
- [ ] AC-14 (always): A review session lists and accepts only the tools for reading its own review job and for submitting its step's result, and the session's token stops working when its review step ends; Pincel starts the reviewer CLI with the restrictions of `ai-chat-panel` AC-16 and without web search, so it has no other MCP server, no shell, no file access and no web access (evidence: the session's tool list and each adapter's start arguments).
- [ ] AC-15 (event): When the user marks a thumbnail of a reviewed generation with thumbs up or thumbs down, then the mark is shown on it and can be toggled off or switched; marks stay with the variant until the strip is dropped.
- [ ] AC-16 (event): When the user applies a variant of a reviewed generation, or drops its variants by Discard or by clicking Generate again, and at least one of its rounds was judged, then a lesson step runs in the background with that reviewed generation's reviewer — the panel stays usable, a new generation may start, and closing the tab does not stop it — and gives the reviewer, without any image, the model family, the family's current lessons, each round's prompt, scores and reasons, the thumbs marks and which variant was applied (or that all were dropped); the reviewer submits the family's new lesson list, which replaces the stored one; an entry equal to a stored lesson keeps that lesson's date and user mark, any other entry is dated now and not user-marked.
- [ ] AC-17 (state): While a lesson step runs for a model family, a further lesson step for the same family waits and starts after it, with the then current lessons; lesson steps for different families run independently.
- [ ] AC-18 (always): A model family holds at most 30 lessons; a lesson list over 30 entries, with an empty lesson, a lesson over 200 characters or two equal lessons is refused (AC-13); lessons the user added or edited appear word for word in every accepted list.
- [ ] AC-19 (error): If a lesson step fails, times out after 3 minutes, or the user added, edited or deleted a lesson of that family after the step started, then the stored lessons stay unchanged and the Generate panel of the tab the step came from shows "Lessons not updated: " plus the reason until that tab's next Generate.
- [ ] AC-20 (event): When the user opens Lessons… from the Generate panel, then a dialog lists the lessons per model family with their date of last change and lets the user add, edit and delete single lessons and delete all lessons of a family; Save is disabled for an empty text, a text over 200 characters (a counter shows the length) or a text equal to another lesson of the family, Add is disabled at 30 lessons, and every change persists across restarts and is used by the next review step.
- [ ] AC-21 (error): If saving lessons fails (after a lesson step or a change in the dialog), then the stored lessons stay as they were and the message "Lessons could not be saved: " plus the reason appears where the change came from (the panel as in AC-19, or the dialog, which keeps the edit open). If the stored lessons cannot be read, then every family starts without lessons, the Lessons dialog shows "Stored lessons could not be read and were reset", and the next change stores a fresh list.
- [ ] AC-22 (event): When the user switches tab during a reviewed generation, then it continues and its rounds, scores and marks stay with their tab as in `comfyui-generate` AC-14; when the tab closes, its running round, revision or judgement is cancelled and no new lesson step starts for its variants, because closing a tab is no judgement of the variants (the user may close it for any reason); a lesson step it started earlier continues (AC-16).
- [ ] AC-23 (event): When the app quits while review steps run or wait (AC-17), then their reviewer processes end, waiting steps do not start, and their results are dropped; stored lessons stay as they were.
- [ ] AC-24 (always): The review controls, the Lessons dialog and their messages are translated in every UI locale.
- [ ] AC-25 (state, manual: needs a GPU, a model and a signed-in CLI): With SD 1.5 and Claude Code as reviewer, threshold 10 and 2 extra rounds, "pixel art knight with a sword" runs three rounds unless a variant scores 10, every thumbnail shows a score and selecting it shows a reason, and "Round N of 3" is visible; after Apply the Lessons dialog shows the list the lesson step returned (or the AC-19 notice with its reason), and with at least one lesson stored the next Generate shows a revised prompt before round 1.

## Technical notes

- Review session: a sibling of the chat session (`mcp-live-desktop` AC-22) with its own token per review step. Its tools are host tools of the desktop endpoint, like `gen.*`, and not part of `pincel-agent`. Names follow the existing `<group>.<verb>` scheme, e.g. `review.job` (job data, images as MCP image content) and `review.submit` (verdict, revision or lesson list, by the step's kind). A structured submit tool works the same way for every CLI adapter, unlike parsing answer text.
- Lessons and prompts reach the reviewer inside its job as quoted data, not as part of its instructions; the reviewer can do nothing beyond submitting its step's result anyway (AC-14).
- The reviewer's instructions (role, scale, what a good revision looks like) ship in English.
- Lesson storage: local app data next to the other generation preferences, keyed by model family.
- Starting the reviewer reuses the `ai-chat-clis` adapters (tool restriction, model choice, local or cloud classification).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Extra rounds 0 | One round, judged; no further round; a lesson step runs after Apply / Discard (AC-16) |
| Round 1 already reaches the threshold | Loop ends after round 1 (AC-09) |
| A round returns fewer variants than requested (`comfyui-generate` AC-15) | The judgement covers the variants that arrived; with none, the loop ends with the generation error (AC-12) |
| User changes threshold, rounds, reviewer or turns Review off during a reviewed generation | The running reviewed generation keeps its review settings; the change applies from the next Generate. Revoke is the exception (AC-04) |
| CLI uninstalled while a review step runs | The step finishes or fails on its own (AC-12, AC-19) |
| Two tabs generate with review for the same family | Each has its own judgements and tokens; their lesson steps run one after the other (AC-17) |
| User deletes a lesson while a lesson step for its family runs | The step's result is not stored (AC-19), so the deleted lesson cannot come back |
| Loop ended by Cancel or a review failure, then the user applies a variant | A lesson step runs if at least one round was judged (AC-16) |
| Pixel mode with an indexed sprite | The reviewer sees the palette-mapped shown variant, which is what Apply would add |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Default threshold 7, 2 extra rounds, 3-minute limit per review step | Owner chose "threshold + max rounds" with these proposed defaults; three minutes covers a slow local model judging 8 images | A slow local vision model may need longer; the limit is a constant that is easy to raise |
| 30 lessons per family, 200 characters each | Keeps the reviewer's input small and the dialog readable | Too few for many styles; a later revision can add per-style sets |
| Image permission per CLI and model, kept across restarts | Owner, 2026-11-16: generating happens often, asking on every click would make review unusable; the chat asks per chat because a chat is one long session | The user may forget a permission; the panel shows it with Revoke (AC-04) |
| The reviewer model must accept images | Scoring needs to see the variants | A text-only model fails the step with the CLI's error (AC-12) |

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
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |
| AC-23 |      | open   |        |
| AC-24 |      | open   |        |
| AC-25 |      | open   |        |

## Decisions

- 2026-11-16: The reviewer is a second agent CLI instance with its own read-only review session, judges only Generate-panel variants, loops with "threshold + max rounds", revises the first prompt from lessons, learns from agent scores plus the user's choice and thumbs, and keeps lessons app-wide per model family (owner).
- 2026-11-16: Image permission for a cloud reviewer is kept per CLI and model across restarts, revocable in the panel (owner).
- 2026-11-16: Second clarification pass: Revoke stops running review steps; lesson steps get no images, run with the generation's reviewer and survive a tab close; lessons are matched by text (ignoring case) to keep date and user mark.
- 2026-11-16: Clarification pass: the loop ends at the threshold, the round limit, Cancel or a failure; lesson steps follow the user's Apply / Discard / Generate again, not the loop's end; a lesson step whose family the user changed meanwhile is dropped instead of merged — merging an AI-written list with user edits has no unambiguous result.
