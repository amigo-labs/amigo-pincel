---
id: todo-website-comparison
title: Website — bring the "honest comparison" in line with spec §18
status: todo
size: S–M (one session)
mode: website
depends_on: []
created: 2026-10-28
---

# Website: bring the "honest comparison" in line with spec §18

## Goal

The home page's comparison table covers Pincel's **real** competitive field, as researched in `docs/specs/pincel.md` §18 (snapshot 2026-10-28):
- the pixel-art editors **including Pixelorama**;
- a second, smaller table for **Image mode** against the paint.net-class editors;
- correct cells, including the gaps where Pincel is behind.

The tone stays as `docs/specs/website-spec.md` §4.1.4 demands: *"Honest about where Aseprite is still ahead."*

## Why

Today's table (`website/src/lib/components/ComparisonTable.svelte`, copy in `website/src/lib/i18n/en.ts` / `de.ts`, the `comparison` keys, ≈L93–129) compares only **Pincel, Aseprite and Piskel**. Against §18 that is:

- **Missing Pixelorama.** It is the closest competitor: free (MIT), web + desktop + Android, and since 1.2.2 (2026-09) it **exports `.aseprite`**, with indexed mode, autotiling and PSD import. An "honest comparison" without it isn't credible.
- **Missing the Image-mode field.** There is only the single "layered image editing" row. Paint.NET (Windows-only), Pinta (MIT, cross-platform) and Photopea (web, ads) are the reference points for the second half of the product.
- **Wrong cell.** Aseprite "Open source ✅ (EULA-restricted)". Aseprite is **source-available**: its EULA forbids redistribution, so it is not OSI open source. Show it as "source-available", not ✅.
- **Gaps not shown.** Each needs a row with an honest ❌ or 🟡 for Pincel:
  - pixel-perfect pencil (todo 01);
  - animated GIF / APNG export (todo 06);
  - pen pressure in Pixel mode (todo 02);
  - PSD import (todo 04);
  - indexed painting (§13).
- **Dated wording.** Piskel's "modernization in progress" should be checked against §18.1: there has been no tagged release since 0.15.0 (2018), while master still gets fixes.

## Current state (read these first)

- **`website/src/lib/components/ComparisonTable.svelte`:**
  - `rows: Row[]`, where each row has `feature`, `pincel`, `aseprite` and `piskel` cells;
  - a cell is a mark string (`'✅' | '❌' | '🟡' | 'free' | 'paid'`) or `{ mark, note }`;
  - the note texts come from `comparison.notes`;
  - the columns are hard-coded in the header and in `{#each [row.pincel, row.aseprite, row.piskel]}`.
- **i18n:** `website/src/lib/i18n/en.ts` and `de.ts` (`comparison.rows`, `comparison.notes`, `comparison.footnote`).
- **Used** in `website/src/routes/[[lang=locale]]/+page.svelte` (≈L40), under `home.compareEyebrow` / `compareTitle`.
- **Website spec:** `docs/specs/website-spec.md` §4.1.4 is the table and its intent. Update it in the same change.
- **Data source:** `docs/specs/pincel.md` §18.1–18.4. **Cells must match §18.** Where §18 marks `?` or `*`, re-check before publishing, or leave the row out.

## Design

- **Column model.** Make the table data-driven over a `competitors` array instead of the hard-coded three columns.
  - Phones: the table already sits in a `table-wrap` region. Keep horizontal scroll inside the wrapper, and make the Pincel column sticky.
- **Table 1, "Pixel art"**:
  - columns Pincel, Aseprite, **Pixelorama**, Piskel; optionally LibreSprite or Resprite;
  - rows: today's, plus pixel-perfect, animated export, pen pressure, indexed painting, PSD import.
- **Table 2, "Image editing"**:
  - columns Pincel, Paint.NET, Pinta, Photopea;
  - rows: platforms (Windows / Mac / Linux / web), layers + blend modes, adjustments and effects, selections + magic wand, text, pen pressure, PSD, price / license, offline.
- **Footnote** per table with the snapshot date ("as of 2026-10"), and a link to the sources. A short "Sources" disclosure is enough; it can mirror §18.6.
- **Upkeep.** Add a STATUS.md reminder to re-check the tables when a todo spec that flips a cell lands (01, 02, 04, 06), and at least quarterly. Competitors ship monthly.
- **Tone check:** no "we win everything". Keep the existing footnote's honesty. Lead with where Pincel is different: one app for both, web + desktop, embeddable, free / open.

## Tasks (one session)

1. **`docs(website)`:** update `website-spec.md` §4.1.4 (both tables, sources, upkeep rule).
2. **`feat(website)`:**
   - the data-driven table component;
   - the two tables;
   - the i18n en + de copy (and the languages from `19-more-languages.md` if the website gets them);
   - the Aseprite license cell fixed.
3. **Checks:**
   - `pnpm lint` / `check` / `build` in `website/`;
   - a visual check at 1280 px and 390 px;
   - the Cloudflare preview deploy on the PR.

## Out of scope

- A separate comparison page per competitor ("Pincel vs X" SEO pages). Possible later; keep the tone honest.
- Claims about features that are still todo. Show them as ❌ / 🟡 until they ship.
