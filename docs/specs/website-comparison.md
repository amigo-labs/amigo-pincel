---
status: todo
created: 2026-10-28
size: M
mode: website
design: [website-spec.md §4.1.4, pincel.md §18]
---

# Website: honest comparison in line with spec §18

## Context

The home page's comparison table (website-spec §4.1.4) compares only Pincel, Aseprite and Piskel. Against the competitive research in `pincel.md` §18 (snapshot 2026-10-28) it is incomplete and partly wrong:

- **Pixelorama is missing.** It is the closest competitor: free (MIT), web + desktop + Android, and since 1.2.2 it exports `.aseprite`, with indexed mode, autotiling and PSD import. An "honest comparison" without it isn't credible.
- **The Image-mode field is missing.** One row ("layered image editing") stands for half of the product; Paint.NET, Pinta and Photopea are the reference points (§18.3–18.4).
- **The Aseprite license cell is wrong.** It shows "Open source ✅ (EULA-restricted)", but Aseprite is source-available: its EULA forbids redistribution, so it is not OSI open source.
- **Gaps where Pincel is behind are not shown:** pixel-perfect pencil, animated GIF / APNG export, pen pressure in Pixel mode, PSD import, indexed painting.
- **Piskel's "modernization in progress" is dated:** §18.1 records no tagged release since 0.15.0 (2018), while master still gets fixes.

The tone stays as website-spec §4.1.4 demands: "Honest about where Aseprite is still ahead."

Migrated from the legacy plan `24-website-comparison` (removed 2026-10-29, see git history) (2026-10-28).

## Scope

### In scope

- A "Pixel art" table with the columns Pincel, Aseprite, Pixelorama, Piskel
- An "Image editing" table with the columns Pincel, Paint.NET, Pinta, Photopea
- The new gap rows, the corrected Aseprite license cell and the updated Piskel activity cell
- A footnote per table with the snapshot date and a short Sources disclosure mirroring §18.6
- Phone layout: horizontal scroll inside the table wrapper with the Pincel column kept in view
- English and German copy
- Updating website-spec §4.1.4 (both tables, sources, upkeep rule), and extending §18 where a row needs competitor facts §18 doesn't record yet

### Out of scope

- Separate "Pincel vs X" comparison pages — because they are an SEO project of their own; possible later with the same honest tone
- Claims for features that are still todo — because the table must show them as ❌ / 🟡 until they ship
- Website copy in languages other than English and German — because the website's own i18n has only `en` / `de`, and more website languages are excluded by `more-languages`

## Acceptance criteria

- [ ] AC-01 (always, manual: website/ has no automated test harness): The home page shows two comparison tables: "Pixel art" with the columns Pincel, Aseprite, Pixelorama, Piskel, and "Image editing" with the columns Pincel, Paint.NET, Pinta, Photopea.
- [ ] AC-02 (always, manual: website/ has no automated test harness): The "Pixel art" table keeps today's rows and adds the rows pixel-perfect pencil, animated export (GIF / APNG), pen pressure, indexed painting and PSD import.
- [ ] AC-03 (always, manual: website/ has no automated test harness): The "Image editing" table has the rows Windows, Mac, Linux, web, layers + blend modes, adjustments and effects, selections + magic wand, text, pen pressure, PSD, price / license, and offline use.
- [ ] AC-04 (always, manual: checking facts against §18 needs a human): Every cell in both tables matches its entry in `pincel.md` §18; a cell whose §18 entry is marked `?` or `*` is either re-checked against its source (and §18 updated) before publishing, or its row is left out.
- [ ] AC-05 (always, manual: website/ has no automated test harness): Pincel's cells for features it does not ship show ❌ or 🟡 with a note — pixel-perfect pencil ❌, animated export ❌, pen pressure 🟡 "Image mode only" in the Pixel-art table, PSD ❌ in both tables, indexed painting ❌.
- [ ] AC-06 (always, manual: website/ has no automated test harness): Aseprite's license cell reads "source-available" and does not show ✅ or "open source".
- [ ] AC-07 (always, manual: website/ has no automated test harness): Piskel's activity cell says that there has been no tagged release since 0.15.0 (2018) while master still gets fixes, and no longer says "modernization in progress".
- [ ] AC-08 (always, manual: website/ has no automated test harness): Each table has a footnote naming its snapshot ("as of 2026-10") and a Sources disclosure that lists the §18.6 sources for that table's tools; the existing honesty footnote about Aseprite's head start stays.
- [ ] AC-09 (state, manual: website/ has no automated test harness): While the viewport is 390 px wide, each table scrolls horizontally inside its own wrapper, the page itself does not scroll horizontally, and the Pincel column stays visible while the table is scrolled.
- [ ] AC-10 (state, manual: website/ has no automated test harness): While the German site (`/de/`) is shown, every table heading, row label, note, footnote and Sources text is German; on the English site it is English.
- [ ] AC-11 (always): `pnpm lint`, `pnpm check` and `pnpm build` in `website/` pass.
- [ ] AC-12 (always, manual: tone is an editorial judgement): The copy leads with where Pincel is different (one app for both, web + desktop, embeddable, free / open) and claims no category it does not win.

## Technical notes

Hints from the legacy plan and a 2026-10-29 code check; paths drift — grep for the names.

- **Component** `website/src/lib/components/ComparisonTable.svelte`: `rows: Row[]` with `feature`, `pincel`, `aseprite`, `piskel`; a cell is `'✅' | '❌' | '🟡' | 'free' | 'paid'` or `{ mark, note }` with note keys from `comparison.notes`; the three columns are hard-coded in the header and in `{#each [row.pincel, row.aseprite, row.piskel]}`. Plan: make it data-driven over a `competitors` array and render it twice.
- **Copy** `website/src/lib/i18n/en.ts` / `de.ts`, `comparison` keys (`rows`, `notes`, `footnote`, `label`). Used in `website/src/routes/[[lang=locale]]/+page.svelte` under `home.compareEyebrow` / `compareTitle`.
- **Wrapper**: the table already sits in a `.table-wrap` region with `overflow-x: auto`; the Pincel column needs `position: sticky`.
- **Design spec**: website-spec §4.1.4 holds today's single table; update it in the same change.
- **Data source**: `pincel.md` §18.1–18.4, legend ✓ / ✗ / ~ / ? and `*` (prior knowledge, not re-checked). §18 does not cover animated export at all, so that row's competitor cells need a source and a §18 addition first.
- **Upkeep (plan)**: re-check the tables when a feature spec that flips a Pincel cell lands (`pixel-pencil`, pixel pressure, PSD import, animated export) and at least quarterly — competitors ship monthly. The plan put this as a `STATUS.md` reminder; it goes into website-spec §4.1.4 instead (see Decisions).
- **Checks** from the plan: `pnpm lint` / `check` / `build` in `website/`, a visual check at 1280 px and 390 px, the Cloudflare preview deploy on the PR.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A competitor cell is `?` in §18 (e.g. Pixelorama slices) | Re-check and update §18, or leave the row out (AC-04) |
| A feature spec flips a Pincel cell after this ships (e.g. pixel-perfect pencil lands) | The cell changes in that feature's change or the next upkeep pass; the snapshot date in the footnote is updated with it |
| A long German note in a narrow cell at 390 px | Wraps inside the cell; the table scrolls inside its wrapper, not the page (AC-09) |
| A row exists in one locale's copy but not the other | `pnpm check` fails, because `de` is typed against `en` (AC-11) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The §18 snapshot (2026-10-28) is still accurate enough to publish for the cells not marked `?` / `*` | It was researched one day before this spec | Medium: competitors ship monthly; the footnote's date makes the age visible |

## Open questions

- `website/` has no automated test harness (no Playwright or Vitest in `website/package.json`), so AC-01 … AC-10 are manual. Add Playwright as a `website/` dev dependency (needs approval, CLAUDE.md §9) so they become e2e tests, or accept manual evidence?
- Should the "Pixel art" table also get a LibreSprite or Resprite column? The plan calls it optional.
- Pincel cannot open or save indexed or grayscale `.aseprite` files today (`pincel-core` read and write adapters return `CodecError::UnsupportedColorMode`), yet §18.2 lists Pincel's Indexed cell as "~ read / write". Should the "Aseprite file format (read + write)" row show Pincel as 🟡 with an "RGBA only" note, and §18.2 be corrected in the same change?

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

## Decisions

- Cells must trace to §18; unverified (`?` / `*`) entries are re-checked or their row is dropped — an "honest comparison" with guessed cells would undermine its own claim (plan).
- Gaps are shown as ❌ / 🟡 for Pincel rather than hidden — the website spec demands honesty about where others are ahead (plan, website-spec §4.1.4).
- Aseprite is "source-available", not open source — its EULA forbids redistribution, so it is not OSI open source (plan).
- The upkeep rule (re-check on cell-flipping feature specs and quarterly) lives in website-spec §4.1.4, not in `STATUS.md` — feature progress now lives in feature specs (`.claude/rules/specs.md`), and the rule is about the page's design, which website-spec owns.
- Size M (plan said S–M): two tables, a component rework, two locales and a §18 / website-spec update fit one session.
