---
name: spec-extract
description: Reverse engineering — writes feature specs from existing code, records the current state of the code in a spec, or migrates a legacy session plan from docs/specs/todo/ into a feature spec.
argument-hint: '<area or feature-name> | --analyze [<spec-name>] | --migrate <docs/specs/todo/file.md>'
---

# Spec Extract

You analyse existing code or legacy plans and turn them into feature specs in the format of `/spec`. Three modes:

| Call | Mode | Function |
| ---- | ---- | -------- |
| `/spec-extract <area>` | Reverse engineering | Derive a spec from code |
| `/spec-extract --analyze [<name>]` | Current state | Enrich an existing spec with the state of the code |
| `/spec-extract --migrate <file>` | Migration | Turn a plan from `docs/specs/todo/` into a feature spec |

The template and the AC rules are those of `/spec` (`.claude/skills/spec/SKILL.md`) — read them first.

---

## Mode: reverse engineering (`<area>`)

### 1. Explore the code

Use Explore agents to find all code for `$ARGUMENTS`:

- Document model, commands and their undo (`pincel-core`, `pincel-image-core`, `pincel-effects`)
- Codecs and round-trip tests (`codec/`, `aseprite-writer`)
- wasm bindings (`pincel-wasm`, `pincel-image-wasm`) and the UI adapters
- Svelte components, stores, menus, shortcuts, i18n keys
- Tests: inline `#[cfg(test)]`, `tests/`, `ui/e2e/`
- Design-spec sections in `docs/specs/pincel.md` that describe the area

### 2. Reconstruct

From the code derive:

- **What it does** — summarize the feature from the implementation
- **Acceptance criteria** — testable ACs in the five forms from the actual behavior (not what it should do)
- **Technical notes** — existing document-model fields, file-format chunks, wasm API, UI patterns
- **Edge cases** — error handling, guard clauses, validations
- **Evidence** — per AC the existing test that checks exactly this behavior (test name + file). Status `built` if such a test exists, else `open`.

### 3. Present + refine

Show the user:

- "Here is what the code does for `$ARGUMENTS`"
- "These ACs are covered by tests: …"
- "These have no tests: …"
- "These gaps / inconsistencies (also against the design spec) I found: …"

Ask: "Shall I save it like this, or do you want to add planned improvements as new ACs?"

### 4. Write the spec

Write to `docs/specs/$0.md` with the template from `/spec`. If that name belongs to a design spec, pick `$0-<aspect>`.

- Checkboxes stay `[ ]`, status `todo` — even if everything seems covered. Only `/spec-check` decides on `implemented`.
- Tell the user: if all ACs are covered, `/spec-check $0` is enough for acceptance; if tests are missing, `/spec-build $0` closes the gaps.

---

## Mode: current state (`--analyze`)

### 1. Load specs

- Scan `docs/specs/` for feature specs with `status: todo`
- With a name: only that spec
- Without a name and several: process them one after another

### 2. Analyse the code

Per spec:

1. Identify affected areas from Context and Technical notes
2. Read the code and analyse the current state
3. Match ACs against the code — are implementation approaches visible?
4. Identify relevant commands, types, wasm exports, components

### 3. Write the current state

Update or add the `## Current state` section in the spec:

```markdown
## Current state

> Last updated: {YYYY-MM-DD}

### Existing implementation

- `crates/pincel-core/src/command/…` — `SetPixel` command with undo exists

### AC coverage (estimated)

| AC    | Status      | Reason                              |
| ----- | ----------- | ----------------------------------- |
| AC-01 | Partial     | Command exists, no wasm export yet  |
| AC-02 | Not started | No code                             |
```

**An existing `## Current state` is replaced completely** — the code is the truth.

---

## Mode: migration (`--migrate <file>`)

The plans in `docs/specs/todo/` (front matter `id: todo-…`, sections Goal / Why / Current state / Design / Tasks / Out of scope) predate feature specs. Migrate one at a time, when it becomes next — never all at once.

1. **Name:** derive `<name>` from the file name without its number (`09-pixel-grid.md` → `pixel-grid`); if it collides with a design spec, add an aspect.
2. **Context** from Goal and Why. **Scope** from Goal and Out of scope (every out-of-scope item needs a "— because"; ask if the plan gives none).
3. **Acceptance criteria** in the five forms with stable IDs (`AC-01` …) from Goal, Design and the tests the plan lists. Requirements that don't translate unambiguously into an AC go under `## Open questions` — don't guess.
4. **Technical notes** from Current state and Design (paths, line hints, document-model and format notes) — they help `spec-build`, so keep them, marked as hints that drift.
5. **Decisions:** recommendations and "stop and ask" points of the plan. Settled ones go to `## Decisions` with their reason; unsettled ones (new dependency, Decision Log entry still to write) to `## Open questions`.
6. The task list and commit plan are dropped — `spec-build` groups blocks itself; git keeps the history.
7. Front matter: `status: todo` (or `implemented` only if the plan says `done` — still unchecked boxes, `/spec-check` confirms), `created` of the plan, `size` and `mode` from the plan, `design:` from its §-references.
8. Run the clarification pass of `/spec` (step 4) on the result.
9. Delete the old file (git keeps the history) and in `docs/specs/todo/README.md` point its table row to the new spec.

---

## Commit

Whether you commit the written, migrated or deleted files without asking is decided by the **commit mode** in Context (`SPEC_AUTOCOMMIT`, per developer):

- **on** (default — also when the Context line is missing or says anything other than `ask`): `spec-build` only starts on a clean working tree and reads the committed spec — so commit yourself:
  1. Only the affected files, by path: `git add docs/specs/<name>.md` (for migration also the deleted plan and `docs/specs/todo/README.md`, by path) — never `git add -A`, no other files.
  2. Commit `docs(spec): extract <name>`, `docs(spec): analyze <name>` or `docs(spec): migrate <name> to a feature spec`.
  3. On `main` (protected, CLAUDE.md §8) first switch to `feat/<name>`, where `spec-build` continues.
- **ask**: ask via AskUserQuestion before the commit — do not end the turn without this question. The question names the files and the commit message; the options are "Commit now" and "Continue without commit":
  - "Commit now": commit as with **on**.
  - "Continue without commit": you don't commit. Name the `git add` command with exactly these paths and the commit message, and point out that `spec-build` only starts once the specs are committed.

In both modes no `git push` — pushing and merging is the user's job.

## Rules

- **Document what IS, not what SHOULD be** — reverse engineering is a snapshot of the current state
- **No spec changes** except `## Current state` in analyze mode
- **Don't change the front-matter status** of an existing spec — only `/spec-check` sets `implemented`
- **Take types and code verbatim** — don't paraphrase
- Specs are **English**; talk to the user in their language

## Context

Existing specs:
!`grep -L '^## Acceptance criteria' docs/specs/*.md 2>/dev/null | sed 's/$/ (design spec)/'; grep -l '^## Acceptance criteria' docs/specs/*.md 2>/dev/null || echo "No feature specs yet"`

Legacy plans:
!`ls docs/specs/todo/*.md 2>/dev/null | grep -v README || echo "none"`

Commit mode:
!`node .claude/hooks/commit-mode.mjs 2>/dev/null || echo "on"`
