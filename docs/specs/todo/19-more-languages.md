---
id: todo-more-languages
title: More UI languages — Spanish, Japanese, French, Portuguese (Brazil)
status: todo
size: L (one M session for the infrastructure, then one S session per language)
mode: both (shell, both editors, native macOS menu)
depends_on: []
created: 2026-10-28
---

# More UI languages: es, ja, fr, pt-BR

## Goal

The UI ships in **Spanish (`es`)**, **Japanese (`ja`)**, **French (`fr`)** and **Portuguese, Brazil (`pt-BR`)**, in addition to English and German. Each is selectable on the start screen and in View ▸ Language, and is chosen automatically from the browser or OS language.

## Why

The user wants these four. Pixel art and indie games are strong in Japan, Brazil and the Spanish- and French-speaking world. The i18n system was built to make this cheap: it uses typed dictionaries, and a missing key fails `pnpm check`.

## Current state (read these first)

**Dictionaries.** `ui/src/lib/i18n/`
- `en.ts` is the source of truth for the keys (≈690 lines).
- `de.ts` is typed against it.
- `index.svelte.ts` holds:
  - `type Locale = 'en' | 'de'`, plus `LOCALES` (the picker labels, each in its own language);
  - `DICTS`;
  - `browserLocale()`, which takes the **base language** from `navigator.languages`;
  - `isLocale()`;
  - `applyLocale()`, which also sets `<html lang>`;
  - `t(key, params)` with `{name}` placeholders. **There are no plural rules.**
- `locale.ts`:
  - `initLocale` restores the IndexedDB pref `language`;
  - `syncNativeMenu` calls the Tauri command `set_menu_language`;
  - `languageMenu()`.

**Native macOS menu.** `src-tauri/src/main.rs` has a `Labels` table per language with `labels_for(lang)` (≈L143–180) and a test `labels_for_german_and_fallback` (≈L542). The native menu exists on macOS only (Decision Log 2026-10-27).

**UI font.** `ui/src/app.css` (≈L35) uses the `system-ui` stack, which renders Japanese on every target OS. Nothing to ship.

**Text tool font.** Both modes use one bundled font, Liberation Sans (OFL), loaded in `ui/src/lib/text/font`. It has **no CJK glyphs**, so Japanese text drawn on the canvas would be missing. That is a separate concern; see "Out of scope".

**e2e.** `ui/e2e/i18n.spec.ts` covers the German browser default, the switch persisting across a reload, and View ▸ Language.

## Design

### Infrastructure (once, first session)

**Locale ids.** `Locale = 'en' | 'de' | 'es' | 'ja' | 'fr' | 'pt-BR'`.
- `browserLocale()` must match the full tag first (`pt-BR`, and also `pt-PT` → `pt-BR` as the closest), then the base language.
- `isLocale()` follows the union. **Don't write the list twice**: derive the union from `LOCALES`.

**Plurals.** Add `tn(key, count, params?)`.
- Keys come in forms such as `palette.imported_one` / `palette.imported_other`.
- `Intl.PluralRules(locale)` picks the form.
- Japanese uses `_other` only; the type allows missing `_one` for `ja`.

Convert the handful of count messages that exist today. Find them with `grep -n "{count}" ui/src/lib/i18n/en.ts`.

**Number formatting.** Sizes like "800 × 600" stay as they are; they are technical. Percentages and decimals in option bars go through `Intl.NumberFormat(locale)` where they are displayed as text.

**Layout robustness.**
- Spanish, French and Portuguese strings run about 20–30 % longer than English. Check:
  - the menu bar, which already scrolls on tablets;
  - the options bar;
  - the New dialog;
  - the paste dialog (`Modal.svelte` with three buttons);
  - the start screen.
- Fix overflow with wrapping or `min-width: 0` truncation plus a `title` attribute, never by shortening the translations.

**Native menu.** Add a `Labels` entry per language in `src-tauri/src/main.rs`, and extend the Rust test to cover every locale.

**Website.** Optional. The marketing site has its own i18n (`website/src/lib`); keep it separate and decide per language.

### Translations (one session each)

- Translate `en.ts` completely. Keep `{placeholders}` and keyboard key names untouched.
- **Terminology:** follow each language's established terms in Aseprite, Photoshop and paint.net, and keep a short glossary at the top of each file: layer, frame, tag, slice, tileset, cel, marquee, lasso, magic wand, feather, onion skin, blend modes.
- **Blend mode names** follow the platform conventions of each language, e.g. Photoshop's localized names.
- Document content is never translated: layer, tag and slice names (CLAUDE.md §5.4).
- Ask a native speaker to review if one is available. Mark the file header `machine-assisted, review pending` until someone has.

## Tasks

1. **M: infrastructure.**
   - `docs(spec)` commit: update §9.5 Language and add a Decision Log entry (locale list, `tn` plurals, the `pt-PT` → `pt-BR` fallback).
   - Locale union, detection, `tn` and the count messages converted.
   - The native menu table.
   - Layout check at 1280 px and 390 px with a pseudo-locale. A dev-only `en-XA` that pads every string by about 40 % and wraps it in brackets is worth adding temporarily to find truncation; don't ship it.
   - e2e: the existing i18n spec parameterized over all locales. Check that the start screen renders the localized "New image" button and that `<html lang>` matches.
2. **S per language:** `es.ts`, `ja.ts`, `fr.ts`, `pt-BR.ts`.
   - Each one adds an entry to `LOCALES` and `DICTS`, and a native menu label set.
   - Each one gets an e2e smoke test: the browser locale picks it, and the File menu shows its localized label.

## Out of scope

- Right-to-left languages, which need a layout audit.
- A CJK font for the **Text tool**. Rendering Japanese on the canvas needs a bundled CJK font such as Noto Sans JP (~4 MB for a subset, OFL), or a font picker that loads system fonts. **That is a dependency and asset decision; backlog it.**
- Translated documentation and website copy.
