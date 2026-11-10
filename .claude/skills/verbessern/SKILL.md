---
name: verbessern
description: Improves one screen, flow or module in three steps — scripts measure the current version into a defect list, one builder fixes it (or builds one named idea) behind the same scripts as gate, and one blind pairwise comparison of old vs new in both orders decides whether the change is kept. Use this skill when the user says "/verbessern", "mach das besser", "verbessere den Screen / den Ablauf / das Modul", "/turnier", "mach ein Turnier", or wants a target redone and only kept if it is really better. Not for small cleanups (use /simplify) or fixed-rule passes (use /qa).
argument-hint: '<screen|flow|module> [idea]'
---

# Verbessern: $ARGUMENTS

Measure, change once, compare once. This replaces a crumora-style tournament. The tournament run on this repo cost about 7 M tokens across ~45 agents. Its value came from round 1 and from findings a script makes. Later rounds sorted near-equal versions inside the judge's noise. This skill keeps the useful parts and needs about two agents.

## 1. Measure (no agents)

Run the scripts on the current version and write the defect list to `<scratchpad>/verbessern/<slug>/defects.md`, every item with a number.

| Target | Run |
| --- | --- |
| Any UI | `node .claude/skills/verbessern/scripts/ui-lint.mjs <base>` — raw colours, en/de key parity |
| Screen | `pnpm build && pnpm preview --port <p>`, then `node .claude/skills/verbessern/scripts/sweep.mjs http://localhost:<p>/app/ --setup <state.mjs>`. The setup file exports `langs` and `states: [{ name, run(page, lang) }]`, which bring the page into its stress state: longest German strings, full lists. |
| Flow | A short Playwright script that walks the person's 3–5 tasks: steps, worst latency, keyboard-only, 360 px touch, wrong input. Reuse `ui/e2e/helpers.ts`. |
| Module | fmt, clippy `-D warnings`, the crate's tests, plus a measurement where a claim needs one (allocations, time). |
| Always | The e2e specs that cover the target (`grep -l` its accessible names in `ui/e2e/`). Also the feature-spec ACs it touches (`grep -n` in `docs/specs/*.md`); quote them in `defects.md` as constraints. |

If the list is empty and the user named no idea: report that and stop. Nothing to improve is a result.

## 2. Change once (one builder)

One agent (Agent tool) in its own checkout:

- **Checkout:** `git worktree add --detach`, `ln -s` the main `ui/node_modules` and `crates/*/pkg`, and its own `CARGO_TARGET_DIR`.
- **Task:** fix `defects.md`, or build the user's idea. The constraints below go in verbatim:

  > Only `pc-*` tokens and `app.css` classes, the `Icon` component, no raw colours. Every string through `t()`, keys in `en.ts` and `de.ts`. Keep every prop, binding, id, `data-tour` anchor and accessible name e2e uses. Don't contradict the quoted spec ACs. No localStorage, no suppressions, no new dependency. Don't edit the main checkout.
- **Gate:** step 1's scripts and specs, all green. A red result gets one repair, then the builder stops and reports.
- **Report:** ≤120 words; artifacts stay in files.

Mechanical code changes run with `model: "sonnet"`.

Two builders only when the user explicitly wants two directions compared. Each gets its own idea, and the better one goes to step 3.

## 3. Compare once (one judge)

`node .claude/skills/verbessern/scripts/blind.mjs <pool> old=<dir> new=<dir>`

The pool holds renders (screen), sanitized walk traces (flow) or source plus measurements (module). The key file stays outside the pool.

One judge agent compares the two versions **twice in the same call**: first A against B, then B against A as a fresh look. Each comparison uses the target's criteria and names something specific in the render, trace or code.

- **Keep the new version** only if it wins **both** comparisons.
- **A split or a loss** means the old version stays, and the report names what the new one did better, for a later attempt.

The main thread reads the verdict only, never PNGs or full traces.

## 4. Apply and report

When the new version won:

- Copy its files.
- Run `/qa`.
- Run step 1's scripts once more on the result.
- Commit with the before/after numbers from `defects.md` in the message.

Report in five short sections:

- what was measured
- what changed
- the judge's two verdicts
- open defects
- the summed `subagent_tokens` of all agents

A second attempt is a new run with a fresh defect list, started on purpose, never automatically.
