---
name: turnier
description: Runs a lean design / interaction / code tournament on one target — three challengers built in parallel, scripted gates before any judging, one blind judge with a measured noise margin, and a second round only on a clear win. Use this skill when the user says "/turnier", "mach ein Turnier", "run a tournament", or wants a screen, flow or module rebuilt several ways and the best one kept. Not for small cleanups (use /simplify) or fixed-rule passes (use /qa).
argument-hint: '<compose|humanize|evolve> <target>'
---

# Turnier: $ARGUMENTS

A tournament in the crumora sense: frozen target, gate before score, blind judge, champion falls only to a clear win. It is lean on purpose. The first run on this repo cost about 7 M tokens. Most of that went into rounds whose gains sat inside the judge's noise, and into LLM checks whose findings a script finds before judging. Every rule below exists because of that.

Variants: `compose` judges how a screen looks (renders), `humanize` judges how a flow works (walk traces), `evolve` improves code (source, code rubric). The rubrics and caps are crumora's: `compose` Layout ≤7, `humanize` Error ≤6 / Reach ≤5.

## 1. Triage — no agents

Run a tournament only when the target has several genuinely different directions: a layout, an interaction flow, an API shape. For a small, clean module, run `/simplify` and stop. A noise-sized gain is not worth three builders.

## 2. Freeze the target

Write `<scratchpad>/turnier/<slug>/target.md` before any builder starts:

- 3–8 bullets: what it must do, what must not break.
- The rubric.
- For UI: the content set, including a stress case (longest strings in German, full lists), and the viewports.
- **Every feature-spec AC the target touches** (`grep -n` the files and behaviours in `docs/specs/*.md`), quoted as bullets. A challenger that contradicts an AC fails the gate. To change specified behaviour, revise the spec first (AGENTS.md §2.1), not inside the tournament.

## 3. One round, three challengers

Three ideas, one sentence each, different from each other. Then three builders in parallel (Agent tool, background). Each builder gets:

- the target file, its idea and the champion's path, and its own checkout (`git worktree add --detach`, `ln -s` the main `ui/node_modules` and `crates/*/pkg`, its **own** `CARGO_TARGET_DIR`);
- this constraints block, verbatim:

  > Only `pc-*` tokens and `app.css` classes (`panel-btn`, `panel-btn-primary`, …), the `Icon` component, no raw colours. Every string through `t()`, keys in `en.ts` and `de.ts`. Keep every prop, binding, id, `data-tour` anchor and accessible name e2e uses. No localStorage, no suppressions, no new dependency. Don't edit the main checkout.
- the instruction to reply in **≤120 words**, with artifacts left in files.

Mechanical code moves (evolve fixes, grafts) run with `model: "sonnet"`. Design and interaction ideas use the default model.

## 4. Gate before judge, cheapest first

Builders run steps 1–3 themselves. The orchestrator runs step 4, serialized with `flock`.

1. `node .claude/skills/turnier/scripts/ui-lint.mjs <base>` (UI), or fmt + clippy `-D warnings` (Rust).
2. Build: `pnpm build`, or `cargo test -p <crate>`.
3. UI only: `node .claude/skills/turnier/scripts/sweep.mjs <url> --setup <state.mjs>` with the stress content. The setup file exports `langs` and `states: [{ name, run(page, lang) }]`. Zero console errors.
4. The e2e specs that cover the target (`grep -l` the component's accessible names in `ui/e2e/`): `pnpm exec playwright test <names> --trace off` on a private `PINCEL_E2E_PORT`.

Red gets one repair, then the challenger is out. A spec listed in the AGENTS.md Gotchas as flaky gets one re-run, recorded in `rounds.md`.

## 5. One blind judge call

`node .claude/skills/turnier/scripts/blind.mjs <pool> champion=<dir> c1=<dir> c2=<dir> c3=<dir> --dup champion`

The pool holds renders (`compose`), sanitized traces (`humanize`) or source (`evolve`). The champion appears twice under different labels.

- **Noise margin:** the gap between the champion's two scores, with a floor of 3.
- **Crown:** a challenger is crowned only if it beats the champion by **more than the noise margin**.
- **Judge's reply:** the judge writes the scores in a table and names what wins and loses.
- **Main thread:** reads only the verdict, never the PNGs or full traces.

## 6. Pre-crown only when it can change the outcome

Apply the caps arithmetically, as if every capped criterion missed. Run the LLM checklist (crumora's compose / humanize checklist, as one call) only if that worst case drops the winner to within the noise margin of the champion.

A finding is a **red line** only if the render shows it, e.g. ink overlapping or content unreachable. Touching line boxes, tight gaps or off-scale spacing are caps, not red lines.

## 7. Stop

- **Default:** one round.
- **Second round:** only if the winner's margin is more than **twice** the noise margin and the judge or the scripts name at least two concrete, fixable weaknesses.
- **evolve:** stops as soon as Δ ≤ the noise margin.
- **Hard caps:** 4 rounds (`compose` / `humanize`), 6 rounds (`evolve`).

## 8. Apply and report

Copy the champion's files into the main checkout. Run `/qa`, then the sweep or walk again at every declared viewport.

Report in crumora's five headings:

- What we set out to do
- What we did
- How we did it
- Possible mistakes
- Rounds

Add one line with the summed `subagent_tokens` of every agent in the run.
