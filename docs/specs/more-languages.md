---
status: todo
created: 2026-10-28
size: L
mode: both
design: [pincel.md §9.5, pincel.md §11.3, pincel.md §15]
---

# More UI languages: es, ja, fr, pt-BR

## Context

The UI ships in English and German today. The user wants four more: **Spanish (`es`)**, **Japanese (`ja`)**, **French (`fr`)** and **Portuguese, Brazil (`pt-BR`)**. Pixel art and indie games are strong in Japan, Brazil and the Spanish- and French-speaking world.

The i18n system was built to make this cheap (Decision Log 2026-10-12 "Own DE/EN dictionary"): typed dictionaries per locale, `en.ts` defines the keys, a missing key fails `pnpm check`, no i18n dependency. That entry says "Revisit if a third locale with complex plural rules arrives" — this feature is that third locale. Today there are no plural rules: count messages use fallbacks such as `'{count} snapshot(s)'`.

The language follows the browser by default; the user's choice (start screen, View ▸ Language) is stored in the IndexedDB pref `language` (spec §9.5). On macOS the native menu takes its labels from a Rust table switched by `set_menu_language` (§11.3, Decision Log 2026-10-27); the same table also labels the desktop updater's dialogs on every platform.

Migrated from the legacy plan `19-more-languages` (removed 2026-10-29, see git history) (2026-10-28).

## Scope

### In scope

- Four new UI locales: `es`, `ja`, `fr`, `pt-BR`, selectable on the start screen and under View ▸ Language, and picked automatically from the browser / OS language
- Locale detection by full tag first, then base language, with every Portuguese tag falling back to `pt-BR`
- Plural-aware count messages for all six locales; the existing count messages are converted
- Locale-aware number formatting for percentages and decimals shown as text in the options bar
- Layout that holds the longer Spanish, French and Portuguese strings without clipping text silently
- Native menu and updater-dialog labels (Rust table) for the four new locales
- Updating spec §9.5 to the six locales

### Out of scope

- Right-to-left languages — because they need a layout audit of the whole frame of their own
- A CJK font for the Text tool — because rendering Japanese on the canvas needs a bundled CJK font (e.g. Noto Sans JP, ~4 MB subset, OFL) or a system-font picker, which is a dependency and asset decision; it stays on the backlog (`docs/specs/README.md`, "CJK font for the Text tool")
- Translated documentation and website copy — because the marketing site has its own i18n (`website/src/lib/i18n/`) and is decided per language separately
- Translating document content (layer, tag, slice names) — because CLAUDE.md §5.4 forbids it
- Locale-aware formatting of technical sizes such as "800 × 600" — because they are technical values, not prose

## Acceptance criteria

- [ ] AC-01 (always): The language picker on the start screen and the View ▸ Language submenu list exactly six languages, each labelled in its own language: English, Deutsch, Español, 日本語, Français, Português (Brasil).
- [ ] AC-02 (event): When the user picks one of the six languages, then every user-visible UI string switches to that language without a reload, and after a reload the same language is still active.
- [ ] AC-03 (event): When no language pref is stored, then the UI starts in the first entry of the browser's language list that matches a supported locale — matching the full tag first (`pt-BR`), then the base language (`es-MX` → `es`, `ja-JP` → `ja`, `fr-CA` → `fr`), with any `pt` tag (`pt`, `pt-PT`, `pt-BR`) → `pt-BR` — and in English when no entry matches.
- [ ] AC-04 (always): The document's `<html lang>` attribute equals the active locale's tag (`en`, `de`, `es`, `ja`, `fr`, `pt-BR`).
- [ ] AC-05 (always): Every locale defines every message key of the English dictionary; a locale missing a key fails `pnpm check`.
- [ ] AC-06 (always): Every translated message contains exactly the same set of `{placeholder}` names as its English source.
- [ ] AC-07 (event): When a message that depends on a count is shown, then its wording follows the active locale's plural rule for that count (e.g. English "1 snapshot" vs "2 snapshots"; French uses its "one" form for 0 and 1), and no shown message uses a "(s)"-style fallback.
- [ ] AC-08 (always): Japanese count messages use one form for every count.
- [ ] AC-09 (state): While French, German, Spanish or Portuguese (Brazil) is active, percentages and decimals shown as text in the options bar use that locale's decimal separator and percent format (e.g. French shows the Image-mode brush opacity 100 as "100 %" with a narrow no-break space).
- [ ] AC-10 (state): While any of the six locales is active, at viewport widths 1280 px and 390 px, every text element of the menu bar, options bar, New dialog, paste dialog and start screen either shows its full text or, where it is truncated, carries its full text as a `title` tooltip; no element overflows its container visibly.
- [ ] AC-11 (event): When the desktop app receives a language change to any of the six locales, then the native menu (macOS) and the updater dialogs (all desktop platforms) use that language's labels; an unknown language tag gives English labels.
- [ ] AC-12 (always): Layer, tag and slice names are shown exactly as stored in the document, whatever the active locale.
- [ ] AC-13 (always, manual: translation quality needs a human reader): The translations of the core terms — layer, frame, tag, slice, tileset, cel, marquee, lasso, magic wand, feather, onion skin and the blend-mode names — follow the established terms of Aseprite, Photoshop or paint.net in that language.

## Technical notes

Hints from the legacy plan and a 2026-10-29 code check; paths and line numbers drift — grep for the names.

- **Dictionaries** `ui/src/lib/i18n/`: `en.ts` (≈690 lines, source of keys), `de.ts` typed against it. `index.svelte.ts` holds `type Locale = 'en' | 'de'`, `LOCALES` (picker labels), `DICTS`, `browserLocale()` (base language only today), `isLocale()`, `applyLocale()` (sets `<html lang>`), `t(key, params)` with `{name}` placeholders.
- **Shell side** `ui/src/lib/i18n/locale.ts`: `initLocale` (restores pref `language`), `setLocale`, `syncNativeMenu` (Tauri `set_menu_language`), `languageMenu()`.
- **Count messages today**: `grep -n "{count}" ui/src/lib/i18n/en.ts` — e.g. `recovery.count` (`'{count} snapshot(s)'`), `palette.imported`, `palette.exported`. Also check `{n}` messages (`sheet.allFrames`).
- **Plural design from the plan**: `tn(key, count, params?)` with keys such as `palette.imported_one` / `palette.imported_other`, `Intl.PluralRules(locale)` selecting the form; the type lets `ja` omit `_one`. Derive the `Locale` union from `LOCALES` instead of writing the list twice.
- **Percent text in the options bar**: `ui/src/modes/image/components/toolbar/ToolOptions.svelte` (`{tool.opacity}%`, `{tool.hardness}%`, `{tool.smudgeStrength}%`). Other percent / decimal text outside the options bar: `LayersPanel.svelte` opacity, Image-mode zoom label in `MainCanvas.svelte`, `RecoveryDialog.svelte` sizes and dates.
- **Layout check** from the plan: a dev-only pseudo-locale (`en-XA`, strings padded ~40 % and bracketed) helps find truncation; never shipped. Fix overflow with wrapping or `min-width: 0` truncation plus `title`, never by shortening translations. Places to check: menu bar (already scrolls on tablets), options bar, New dialog, paste dialog (`Modal.svelte` with three buttons), start screen.
- **Native labels** `src-tauri/src/main.rs`: `struct Labels`, `LABELS_EN` / `LABELS_DE`, `labels_for(lang)` (anything but `de` is English), a `static GERMAN: AtomicBool` read by `current_labels()` — used by the menu builder and by `src-tauri/src/updater.rs` (update dialogs). The bool has to become a locale. Test `labels_for_german_and_fallback`.
- **UI font**: `ui/src/app.css` uses the `system-ui` stack, which renders Japanese on every target OS — nothing to ship.
- **e2e**: `ui/e2e/i18n.spec.ts` covers the German browser default, persistence across reload and View ▸ Language. The plan suggests parameterizing it over all locales (localized "New image" button on the start screen, `<html lang>`), plus a per-language smoke (browser locale picks it; the File menu shows its localized label). AC-06 can be checked by a Playwright-run test that imports the dictionaries.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Browser language list is empty or has only unsupported tags (`zh-CN`, `ko`) | English |
| Browser list `['pt-PT', 'en']` | Portuguese (Brazil) — the first matching entry wins |
| Browser list `['en-US', 'ja']` | English — order of the list is respected |
| Stored pref holds an unknown value (e.g. `'it'` from a future version, or garbage) | Pref ignored; browser default applies |
| Count 0 in French | French "one" form (French plural rules treat 0 as singular) |
| Count 1 in Japanese | The single Japanese form |
| A translated string is longer than its slot at 390 px | Wraps, or truncates with the full text in `title` (AC-10) |
| Desktop app on Windows / Linux switches to Japanese | No native menu to relabel; updater dialogs use Japanese labels (AC-11) |
| Japanese text typed into the Text tool | Glyphs may be missing — out of scope (CJK font) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `Intl.PluralRules` and `Intl.NumberFormat` are available in every target runtime | Both are baseline in all evergreen browsers, WebView2, WKWebView and WebKitGTK | Low |
| Machine-assisted translations are acceptable for the first release when marked as review pending | The plan says to ask a native speaker if one is available, not to block on one | Medium: terminology errors reach users until reviewed (AC-13 is a manual check) |

## Open questions

- The Decision Log entry is not written yet: the six-locale list, the in-house `tn` plural helper on `Intl.PluralRules` (revisiting the 2026-10-12 "own DE/EN dictionary" entry), and the `pt-PT` → `pt-BR` fallback (CLAUDE.md §2.1 — must land with or before the code, together with the §9.5 update).
- Should percentages and decimals outside the options bar (Layers panel opacity, Image-mode zoom label, Recovery dialog sizes and dates) also follow the locale's number format, or only the options bar as the plan says?
- Is a native-speaker reviewer available for any of the four languages, or do all four ship marked "machine-assisted, review pending"?

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
| AC-13 |      | open   |        |

## Decisions

- One spec of size L instead of a split: the plan sizes it as one M session for the infrastructure plus one S session per language; `spec-build` can group the infrastructure ACs (AC-03 … AC-11) first and each language after. Splitting would only be needed if it grew to XL.
- No i18n dependency; plurals and numbers use the built-in `Intl` APIs — keeps CLAUDE.md §9's no-new-dependency rule (plan recommendation).
- Every Portuguese tag maps to `pt-BR` — it is the closest of the shipped locales for `pt-PT` users, and shipping a separate `pt-PT` was not requested (plan).
- Overflow is fixed by layout (wrap or truncate with `title`), never by shortening translations — translations must stay complete and natural (plan).
- Each new dictionary carries a short glossary of the core terms at its top and a header note `machine-assisted, review pending` until a native speaker has reviewed it — keeps terminology consistent and the review status visible (plan).
- The `en-XA` pseudo-locale is a dev-only aid and never ships (plan).
- Keyboard key names and `{placeholders}` stay untranslated in every locale (plan; AC-06 checks placeholders).
