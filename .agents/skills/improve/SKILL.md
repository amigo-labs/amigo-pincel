---
name: improve
description: Improves one screen, flow or code module in three steps — scripts measure the current version into a numbered defect list, one builder fixes it (or builds one named idea) behind the same scripts as its gate, and one blind pairwise comparison of old against new, in both orders, decides whether the change is kept. Use when the user says "/improve", "make this better", "improve this screen / flow / module", "/verbessern", "mach das besser", or wants a target redone and kept only if it is really better. A cheap alternative to the compose / humanize / evolve tournaments; not for small cleanups (simplify) or fixed-rule passes (qa, lint).
argument-hint: '<screen|flow|module> [idea]'
---

# Improve: $ARGUMENTS

Measure, change once, compare once.

**Why this shape.** A crumora-style tournament (`compose`, `humanize`, `evolve`) on one repository cost about 7 M tokens across ~45 agents. Its value came from round 1 and from findings a script makes. Later rounds sorted near-equal versions inside the judge's noise (±7 points between passes). This skill keeps the useful part and needs about two agents per run.

**Agent notes:**

- `$ARGUMENTS` is what the user passed: the target (a file, a screen, a flow) and optionally one idea to build.
- "Subagent without chat history" means a fresh agent that sees only the brief you give it (Claude Code: the Agent tool, `general-purpose`). Without subagents, build in the main thread. Judge before you reread your own diff, from the pool only.
- Long commands (build, e2e) may run for minutes: give them a long timeout.

**Project specifics** come from `AGENTS.md`, section `## Spec workflow profile`:

- **Improve** — how to serve the UI, the project's own checks, extra builder constraints, where stress setups live.
- **Conventions**
- **Tests**

Without an **Improve** key, ask the user once how to serve and check the target. Then suggest adding the key.

## 1. Measure (no agents)

Run the checks on the current version. Write the defect list to `<scratch>/improve/<slug>/defects.md`, every item with a number (a count, a width, a latency, a test name).

| Target | Run |
| --- | --- |
| Any | The project checks the **Improve** key names (a style or token lint, translation-key parity, …). |
| Screen | Serve the UI as **Improve** says. Then run `node <skill-dir>/scripts/sweep.mjs <url> --setup <state.mjs>`, where `<skill-dir>` is this skill's folder (`.agents/skills/improve` in a repository that vendors it). The setup file exports `langs` and `states: [{ name, run(page, lang) }]`, which bring the page into its stress state: longest translations, full lists, open panels. The sweep reports overlapping text, horizontal scroll and an unreachable top for every width from 320 to 1440 px. |
| Flow | A short browser script (Playwright) that walks the person's 3–5 tasks. It records steps, the worst feedback latency (measured, not estimated), keyboard-only use, a 360 px touch viewport and wrong input. Reuse the project's e2e helpers (**Tests**). |
| Module | Formatter, linter with warnings as errors, the module's tests. Add a measurement where a claim needs one: allocations, time, size. |
| Always | The tests that cover the target: search the test folders (**Tests**) for its file name, its accessible names or its public functions. Also the acceptance criteria of feature specs it touches: search the specs for its name, and quote them in `defects.md` as constraints. |

If the list is empty and the user named no idea: report that and stop. Nothing to improve is a result.

## 2. Change once (one builder)

One subagent without chat history, in its own checkout:

- **Checkout:** `git worktree add --detach <dir>`.
  - Link installed dependencies and build output from the main checkout instead of reinstalling them.
  - Give a compiled language its own build directory (e.g. a separate `CARGO_TARGET_DIR`). Two checkouts sharing one directory can run each other's binaries.
- **Task:** fix `defects.md`, or build the user's idea.
- **Constraints**, quoted verbatim in the brief:
  - the profile's **Conventions** and the **Improve** key's constraints;
  - keep every public name, prop, id and accessible name the tests use;
  - don't contradict the quoted acceptance criteria;
  - no new dependency, no suppressions;
  - don't edit the main checkout.
- **Gate:** step 1's checks and tests, all green. A red result gets one repair; after that the builder stops and reports.
- **Report:** at most 120 words; artifacts stay in files.

Run mechanical code changes on a smaller, faster model where the agent offers one (Claude Code: `model: "sonnet"`).

Use two builders only when the user explicitly wants two directions compared. Each gets its own idea, and the better one goes on to step 3.

## 3. Compare once (one judge)

Build the pool:

```bash
node <skill-dir>/scripts/blind.mjs <pool> old=<dir> new=<dir>
```

- The pool holds renders (screen), sanitized walk traces (flow) or source plus measurements (module), under shuffled labels.
- The key file is written beside the pool, not in it. Never hand it to the judge.
- `blind.mjs` blanks every folder name wherever it appears in the text, so give the version folders names that are not words, e.g. `v7f2` and `v1c9`. A folder named `new` would also blank "New" in "New image".

One subagent without chat history compares the two versions **twice in the same call**: first A against B, then B against A as a fresh look. Each verdict uses the target's criteria and names something specific in the render, trace or code:

- layout, hierarchy and states for a screen;
- steps, feedback and errors for a flow;
- clarity, measured cost and test coverage for a module.

- **Keep the new version** only if it wins **both** comparisons.
- **A split or a loss** keeps the old version. The report names what the new one did better, for a later attempt.

The main thread reads only the verdict, never the screenshots or full traces.

## 4. Apply and report

When the new version won:

- Copy its files into the main checkout.
- Run the full gates (the `qa` skill where it is installed).
- Run step 1's checks once more on the result.
- Commit with the before/after numbers from `defects.md` in the message.

Report in five short sections:

- what was measured
- what changed
- the judge's two verdicts
- open defects
- the summed token use of all subagents

A second attempt is a new run with a fresh defect list, started on purpose, never automatically.
