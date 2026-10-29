## Quality gates

- **Run `/qa` before finishing any coding task** and report the result. After every answer with code changes the Stop hook runs only the fast gates for the areas that changed (rustfmt + clippy for `crates/`, lint + svelte-check for `ui/` and `website/`); tests, builds and e2e run in `/qa` and in CI.
- **Fix the cause, never lower the gates.** No suppressions (`#[allow(…)]`, `#[expect(…)]`, `@ts-ignore`, `@ts-expect-error`, `eslint-disable`), no weakened, skipped or `#[ignore]`d tests (CLAUDE.md §9). The guard hook refuses new suppressions; a justified one is added by the user.
- **Don't loosen the gate configuration**: `.claude/settings.json`, `.claude/hooks/`, `.claude/gates.json` change only on the user's explicit request (editing them asks for permission).
- When the Stop hook blocks, the error is in its message. Where a gate names a `fix` command (`cargo fmt --all`, `pnpm lint --fix`), formatting and lint autofix are allowed; everything else is fixed in the code.
- `STOP_GATE=off` in the environment switches the Stop hook off, e.g. for a bulk migration — only on the user's request.
