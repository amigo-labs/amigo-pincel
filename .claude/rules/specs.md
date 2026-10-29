## Spec-driven features

Features are built spec-driven: first a feature spec `docs/specs/<name>.md` with testable acceptance criteria, then the implementation with tests, then an independent acceptance check. The SessionStart hook lists the open feature specs at the start of every session.

Two kinds of spec live in `docs/specs/`:

| Kind | Example | Owns |
| ---- | ------- | ---- |
| Design spec | `pincel.md`, `canvas.md`, `mcp.md` (`status: draft`, no acceptance criteria) | Architecture, data model, the Decision Log (`pincel.md` §15) |
| Feature spec | `docs/specs/<name>.md` (`status: todo` / `implemented`, `## Acceptance criteria`) | One feature: what it does, how we prove it |

A feature spec references the design-spec sections it builds on and never contradicts them. A feature that needs an architectural decision (cross-crate boundary, file format, new dependency, public API shape) gets a Decision Log entry in `pincel.md` §15 in the same commit as the spec (CLAUDE.md §2.1). The older session plans in `docs/specs/todo/` are legacy; `/spec-extract --migrate <file>` turns one into a feature spec when it becomes next.

## Triage

Classify every new input before touching code:

| Input | Approach |
| ----- | -------- |
| New feature or changed behavior ("I want …", "it should …") | Start `/spec`, no code before the spec. If the user says "just do it", implement directly. |
| Request touches an existing feature spec | Revise that spec in `/spec`'s revision mode, never create a second one |
| Unclear whether small change or feature | Exactly one question (AskUserQuestion): "Capture as spec" / "Implement directly" |
| Bug fix | Directly, always with a regression test that is red before the fix. If the fix changes specified behavior, revise the spec too. |
| Refactoring, tooling, docs, questions | Directly, no spec |
| "Carry on", "build X", "implement the spec" | `/spec-build <name>` |
| "Is X done?", acceptance after manual implementation | `/spec-check <name>` |
| Document existing behavior, or pick up a `docs/specs/todo/` plan | `/spec-extract <area>` / `/spec-extract --migrate <file>` |

Only `/spec-check` sets a spec to `status: implemented` — never by hand.

## Keeping knowledge

Project knowledge belongs in the repository, not on one machine:

- **Pitfalls and non-obvious knowledge** → `## Gotchas` in `CLAUDE.md`. Only what cannot be read from code or git history; no noise.
- **Architecture decisions** → Decision Log in `docs/specs/pincel.md` §15.
- **Feature-local decisions** → `## Decisions` of the feature spec, with the reason.
- **Progress of a feature** → `## Evidence` of the feature spec (it replaces a `STATUS.md` note for spec work).
- **Never** into auto memory under `~/.claude` — it is switched off for this project because it stays on one machine.
