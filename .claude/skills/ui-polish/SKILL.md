---
name: ui-polish
description: Iteratively improve UI quality by taking screenshots, scoring them across design categories (including the 60/30/10 color rule, 8pt spacing, WCAG contrast, and consistent border radii), and applying targeted fixes until high scores are reached. Use when the user asks to improve UI, polish the interface, hill-climb the design, make the UI better, or run a UI review loop.
---

# UI Polish

Systematically improve the UI of the current application through iterative screenshot-based scoring and targeted fixes.

## Prerequisites

- A browser the agent can drive: the Playwright MCP server (`npx @playwright/mcp@latest`, e.g. `claude mcp add playwright npx @playwright/mcp@latest` in Claude Code) or a Playwright script (`require("playwright")`).
- Prefer a running local dev server. If none is running, start one if possible.

## Workflow

1. **Identify target screens**
   - Focus on the most important 1–3 screens first (e.g. main dashboard, primary form, settings).
   - Ask the user which screens to prioritize if unclear.
   - Include both light and dark mode when relevant.

2. **Capture baseline screenshots**
   - Use the browser (Playwright MCP or a script) to navigate to each target screen.
   - Take full-page screenshots (and key component screenshots if useful).
   - Save them with clear names (e.g. `dashboard-light-round1.png`).
   - Note the current state.

3. **Score the current UI**
   Evaluate the screenshots across these categories (1–10 scale):
   - Typography & visual hierarchy
   - Spacing & layout (8pt grid)
   - Contrast & readability (WCAG)
   - Consistency (radii, borders, icons, colors)
   - Color usage (60/30/10 rule)
   - Overall polish & professional feel

   For each category provide:
   - Score
   - 1–3 concrete, specific improvement suggestions

   When possible, also compare against well-designed reference UIs in the same category.

4. **Apply highest-impact fixes**
   - Prioritize changes that improve multiple categories.
   - Keep changes focused on visual polish.
   - Avoid large functional or architectural changes unless clearly necessary.
   - Make the smallest effective changes.

5. **Iterate**
   - After changes, capture new screenshots.
   - Re-score.
   - Track scores across rounds.
   - Continue until:
     - Average score across categories is ≥ 9/10, **or**
     - Maximum of 6–8 rounds has been reached, **or**
     - Further improvements yield diminishing returns.

6. **Report results**
   Summarize:
   - Screens improved
   - Score progression per category
   - Key changes made
   - Remaining opportunities (if any)

## Rules

- Prefer visual evidence (screenshots) over assumptions.
- Be specific in feedback.
- Prevent scope creep. Stay focused on UI quality.
- If Playwright MCP is unavailable, clearly state the limitation and fall back to code inspection + suggested improvements.
- Stop and report if the app cannot be started or screenshots cannot be taken.

### Color Rule: 60/30/10

- **60%** Dominant color (usually neutral background)
- **30%** Secondary color (supporting surfaces, cards)
- **10%** Accent color (CTAs, highlights, interactive elements)

Flag deviations (too many competing accents, overly saturated backgrounds, accents dominating more than ~10% of visual weight).

### Spacing: 8pt Grid

Prefer an 8pt (or 4pt) spacing system. Use consistent increments such as:

- 4, 8, 12, 16, 24, 32, 40, 48…

Avoid arbitrary values (e.g. 7px, 13px, 19px). Align related elements to the same rhythm.

### Contrast: WCAG

- Body text and important UI text: **minimum 4.5:1** contrast ratio
- Large text (≥18pt regular or ≥14pt bold): **minimum 3:1**
- Interactive elements and icons that convey meaning should also meet contrast requirements

Flag and fix insufficient contrast, especially in light mode or on colored backgrounds.

### Border Radius Consistency

Use a small, consistent set of border radii across the UI. Prefer one of these systems:

- 4px / 8px / 12px
- 6px / 10px / 16px
- or a single radius for most components

Avoid mixing many different radii (e.g. 4, 6, 8, 10, 12, 16 on similar elements).

## Stopping Conditions

Stop the loop when any of these are true:

- Average score ≥ 9/10
- 8 rounds completed
- No meaningful score improvement in the last 2 rounds
- User interrupts
