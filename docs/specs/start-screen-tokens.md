---
status: todo
created: 2026-11-26
size: M
mode: shell
design: [pincel.md §9.2, pincel.md §11.3]
---

# Start screen in two columns; sizes and levels as tokens

## Context

Part of the UI refresh (owner request 2026-11-25; backlog "UI refresh: start screen and tokens").

The start screen has two arrangements. On first use the New cards fill the page, with Open… and the samples below them. Once there are recent files, New sits on the left and Open on the right. The utilities — Tour, Settings, About and the language — sit in the title's row and compete with it.

Many components also set radii, text sizes and stacking levels with literal values (`rounded-[2px]`, `text-[11px]`, `z-[46]`, `z-50`). The same level is written as a number in a dozen places, and nothing says which layer lies over which.

Terms:

- **New column**: the New cards and Open from clipboard.
- **Open column**: Open…, its hint, the recent files when there are any, and the samples.
- **Utilities**: Tour, Settings, About (where the OS draws no native Help menu) and the language select.
- **Literal value**: a radius, font size or z-index written as a number in a component's class list.

## Scope

### In scope

- The start screen's columns on first use and when returning
- A footer for the utilities
- Tokens for radii, text sizes and stacking levels that components still write as numbers, at the values they have today

### Out of scope

- New content on the start screen — because the refresh is about arrangement.
- Changing a size or level's value — because the tokens name what exists; a visible change would need its own review.
- Literal spacing values (`max-w-[30rem]`, `min-w-[12rem]`) — because the theme has no spacing tokens to map them to.

## Acceptance criteria

- [ ] AC-01 (state): While the window is 960 px wide or wider, the start screen shows the New column on the left and the Open column on the right, with their top edges level, on first use and when returning.
- [ ] AC-02 (always): The start screen's title row holds the title and the tagline only; the utilities are in a footer below the columns, drawn as quiet controls (ghost buttons, muted text) with the language select at its right end. The utilities keep their names.
- [ ] AC-03 (state): While the start screen's content is shorter than the window, the footer sits at the window's bottom edge; while it is taller, the footer follows the content.
- [ ] AC-04 (state): While the window is narrower than 960 px, the New column comes first and the Open column below it, each the full width.
- [ ] AC-05 (always): No component in `ui/src` writes a radius, font size or z-index as a literal value in its class list (`rounded-[…]`, `text-[…px]` / `text-[…rem]`, `z-[…]`, `z-<number>`); each uses a token. The stacking tokens keep today's values and order.

## Technical notes

Hints, not contracts.

- `ui/src/lib/shell/StartScreen.svelte` (`start-split`, `start-wide`, `newBlock`, `openCard`, `samples`).
- Tokens: `ui/src/app.css` `@theme` (radii as `--radius-pc-*`) and `:root` (text sizes and stacking levels as plain custom properties, used as `text-(length:--pc-text-…)` and `z-(--pc-z-…)`: Tailwind's `--text-*` theme tokens also set a line height, which the literal sizes did not).
- A Node-side spec can read the component sources (no page).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| macOS desktop app | No Tour, Settings or About in the footer (the native menu has them); the language select stays |
| Tour banner shown | It stays under the title, above the columns |
| A New card is open | The New column grows; the Open column keeps its place |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A footer is where users look for Settings and About on a start page | Common on start pages (VS Code, Figma, browsers' new-tab pages); the editors have them in menus | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/start-screen-tokens.spec.ts`: "wide › New on the left, Open on the right, top edges level" | built |        |
| AC-02 | `ui/e2e/start-screen-tokens.spec.ts`: "wide › the title row holds the title only; the utilities are quiet controls in the footer" | built |        |
| AC-03 | `ui/e2e/start-screen-tokens.spec.ts`: "wide › the footer sits at the window’s bottom while the content is shorter" | built |        |
| AC-04 | `ui/e2e/start-screen-tokens.spec.ts`: "narrow › below 960 px New comes first and Open below it, each full width" | built |        |
| AC-05 | `ui/e2e/start-screen-tokens.spec.ts`: "no component writes a radius, font size or z-index as a literal", "the stacking tokens keep today’s values, lowest first" | built |        |

## Decisions

- 2026-11-25 (owner, mock-up): two columns New / Open and a quiet footer; hard-coded radii, text sizes and z-indexes onto the shared tokens.
- 2026-11-26 (spec): the tokens keep today's values; the stacking levels get names by role, lowest first.
- 2026-11-26 (build): 81 literal values in 44 components became tokens with the same values: `--radius-pc-handle` (2 px, the canvas handles), `--pc-text-tiny|label|ui|title` (10 px, 11 px, 13 px, 1.75 rem) and thirteen stacking levels `--pc-z-*` from `raised` (10) to `tooltip` (100). Text sizes are plain custom properties used as `text-(length:--pc-text-…)`, not `--text-*` theme tokens, because those would also set a line height.
- 2026-11-26 (build): the first-use arrangement uses the returning one's `start-split` grid; the first-use "pair" of Open… and the samples is gone, the Open column stacks them. The footer is the page's last flex item with `mt-auto`.
- 2026-11-26 (build): red run — the four page tests failed against the bundle built from the previous source; the two Node-side tests first failed on a path error, and the literal scan, run against the previous commit with the same pattern, finds 81 lines.
- 2026-11-26 (build): `ui/e2e/start-clipboard.spec.ts` (copy-cut-paste AC-34) checked "nothing between the New cards and the clipboard row" as "Open… lies lower than the row". With Open… in the right column at the cards' height (AC-01) that no longer means "between"; the check now accepts Open… below the row or beside it in the other column.
