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

The language follows the browser by default; the user's choice (start screen, Settings ▸ Language) is stored in the IndexedDB pref `language` (spec §9.5). On macOS the native menu takes its labels from a Rust table switched by `set_menu_language` (§11.3, Decision Log 2026-10-27); the same table also labels the desktop updater's dialogs on every platform.

Migrated from the legacy plan `19-more-languages` (removed 2026-10-29, see git history) (2026-10-28).

## Scope

### In scope

- Four new UI locales: `es`, `ja`, `fr`, `pt-BR`, selectable on the start screen and under Settings ▸ Language, and picked automatically from the browser / OS language
- Locale detection by full tag first, then base language, with every Portuguese tag falling back to `pt-BR`
- Plural-aware count messages for all six locales; the existing count messages are converted
- Locale-aware formatting for percentages, decimals and dates shown as text anywhere in the UI (options bar, Layers panel opacity, zoom label, Recovery dialog)
- Layout that holds the longer Spanish, French and Portuguese strings without clipping text silently
- Native menu and updater-dialog labels (Rust table) for the four new locales
- Updating spec §9.5 to the six locales

### Out of scope

- Right-to-left languages — because they need a layout audit of the whole frame of their own
- A CJK font for the Text tool — because rendering Japanese on the canvas needs a bundled CJK font (e.g. Noto Sans JP, ~4 MB subset, OFL) or a system-font picker, which is a dependency and asset decision; it stays on the backlog (`docs/specs/README.md`, "CJK font for the Text tool")
- Translated documentation and website copy — because the marketing site is built by `amigo-labs/website` from `docs/site/` and is decided per language there
- Translating document content (layer, tag, slice names) — because CLAUDE.md §5.4 forbids it
- Locale-aware formatting of technical sizes such as "800 × 600" — because they are technical values, not prose

## Acceptance criteria

- [ ] AC-01 (always): The language picker on the start screen and the Language list of the Settings dialog list exactly six languages, each labelled in its own language: English, Deutsch, Español, 日本語, Français, Português (Brasil).
- [ ] AC-02 (event): When the user picks one of the six languages, then every user-visible UI string switches to that language without a reload, and after a reload the same language is still active.
- [ ] AC-03 (event): When no language pref is stored, then the UI starts in the first entry of the browser's language list that matches a supported locale — matching the full tag first (`pt-BR`), then the base language (`es-MX` → `es`, `ja-JP` → `ja`, `fr-CA` → `fr`), with any `pt` tag (`pt`, `pt-PT`, `pt-BR`) → `pt-BR` — and in English when no entry matches.
- [ ] AC-04 (always): The document's `<html lang>` attribute equals the active locale's tag (`en`, `de`, `es`, `ja`, `fr`, `pt-BR`).
- [ ] AC-05 (always): Every locale defines every message key of the English dictionary; a locale missing a key fails `pnpm check`.
- [ ] AC-06 (always): Every translated message contains exactly the same set of `{placeholder}` names as its English source.
- [ ] AC-07 (event): When a message that depends on a count is shown, then its wording follows the active locale's plural rule for that count (e.g. English "1 snapshot" vs "2 snapshots"; French uses its "one" form for 0 and 1), and no shown message uses a "(s)"-style fallback.
- [ ] AC-08 (always): Japanese count messages use one form for every count.
- [ ] AC-09 (state): While French, German, Spanish or Portuguese (Brazil) is active, percentages, decimals and dates shown as text anywhere in the UI (options bar, Layers panel opacity, zoom label, Recovery dialog) use that locale's format — decimal separator, percent format and date format (e.g. French shows the Image-mode brush opacity 100 as "100 %" with a narrow no-break space).
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
- **Percent text in the options bar**: `ui/src/modes/image/components/toolbar/ToolOptions.svelte` (`{tool.opacity}%`, `{tool.hardness}%`, `{tool.smudgeStrength}%`). Other percent / decimal / date text: `LayersPanel.svelte` opacity, Image-mode zoom label in `MainCanvas.svelte`, `RecoveryDialog.svelte` sizes and dates. Route every user-visible percentage, decimal and date through one `formatNumber` / `formatPercent` / `formatDate` helper in `ui/src/lib/i18n/`; technical sizes ("800 × 600") stay as they are.
- **Layout check** from the plan: a dev-only pseudo-locale (`en-XA`, strings padded ~40 % and bracketed) helps find truncation; never shipped. Fix overflow with wrapping or `min-width: 0` truncation plus `title`, never by shortening translations. Places to check: menu bar (already scrolls on tablets), options bar, New dialog, paste dialog (`Modal.svelte` with three buttons), start screen.
- **Native labels** `src-tauri/src/main.rs`: `struct Labels`, `LABELS_EN` / `LABELS_DE`, `labels_for(lang)` (anything but `de` is English), a `static GERMAN: AtomicBool` read by `current_labels()` — used by the menu builder and by `src-tauri/src/updater.rs` (update dialogs). The bool has to become a locale. Test `labels_for_german_and_fallback`.
- **UI font**: `ui/src/app.css` uses the `system-ui` stack, which renders Japanese on every target OS — nothing to ship.
- **e2e**: `ui/e2e/i18n.spec.ts` covers the German browser default, persistence across reload and Settings ▸ Language. The plan suggests parameterizing it over all locales (localized "New image" button on the start screen, `<html lang>`), plus a per-language smoke (browser locale picks it; the File menu shows its localized label). AC-06 can be checked by a Playwright-run test that imports the dictionaries.

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

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | e2e `the picker lists six languages in their own names; each switches at once and survives a reload (AC-01, AC-02, AC-04)` (ui/e2e/more-languages.spec.ts); `both pickers label each language in its own language (AC-09)` (ui/e2e/i18n.spec.ts, start screen and Settings) | built |        |
| AC-02 | e2e `the picker lists six languages in their own names; each switches at once and survives a reload …` (ui/e2e/more-languages.spec.ts); `Settings ▸ Language switches menus, panels, tools and buttons at once …` (ui/e2e/i18n.spec.ts) | built |        |
| AC-03 | `the browser language picks a locale: full tag, base language, Portuguese as pt-BR (AC-03)` and the four browser tests `a es-MX / ja-JP / fr-CA / pt-PT browser starts in … (AC-03, AC-04)` (ui/e2e/more-languages.spec.ts); `Italian browser › starts in English …` (ui/e2e/i18n.spec.ts) | built |        |
| AC-04 | the four browser tests and the picker test check `<html lang>` (ui/e2e/more-languages.spec.ts) | built |        |
| AC-05 | every dictionary is typed `Record<MessageKey, string>` against `en.ts` (`pnpm check`); `every translation has exactly the placeholders of its English source (AC-06)` walks every English key in every locale (ui/e2e/i18n-format.spec.ts) | built |        |
| AC-06 | `every translation has exactly the placeholders of its English source (AC-06)` (ui/e2e/i18n-format.spec.ts) | built |        |
| AC-07 | `count messages follow the locale plural rules (AC-07, AC-08)`, `no shown message uses a "(s)"-style fallback (AC-07)` (ui/e2e/i18n-format.spec.ts) | built |        |
| AC-08 | `count messages follow the locale plural rules (AC-07, AC-08)` (ui/e2e/i18n-format.spec.ts); every plural block in `ja.ts` has only its `other` form | built |        |
| AC-09 | `numbers, percentages and dates follow the locale (AC-09)` (ui/e2e/i18n-format.spec.ts); e2e `French writes the brush opacity as a French percentage (AC-09)` (ui/e2e/more-languages.spec.ts) | built |        |
| AC-10 | e2e `at 1280 px <locale> fits or shows its full text as a tooltip (AC-10)` and `at 390 px …`, twelve tests over the start screen, New dialog, menu bar, options bar (five tools) and paste dialog (ui/e2e/more-languages.spec.ts) | built |        |
| AC-11 | `labels_for_each_language_with_english_fallback`, `set_language_switches_the_current_labels`, `every_label_set_is_complete` (src-tauri/src/menu_labels.rs); the updater's messages are in-app now (About dialog, `t()`), so they follow the UI language with every other string | built |        |
| AC-12 | e2e `layer names stay as stored in every language (AC-12)` (ui/e2e/more-languages.spec.ts); `… stay untranslated in German (AC-08)` (ui/e2e/i18n.spec.ts) | built |        |
| AC-13 | manual: a native reader checks the core terms per language (Decisions, 2026-11-12, lists the choices to review) | open |        |

## Decisions

- One spec of size L instead of a split: the plan sizes it as one M session for the infrastructure plus one S session per language; `spec-build` can group the infrastructure ACs (AC-03 … AC-11) first and each language after. Splitting would only be needed if it grew to XL.
- No i18n dependency; plurals and numbers use the built-in `Intl` APIs — keeps CLAUDE.md §9's no-new-dependency rule (plan recommendation).
- Every Portuguese tag maps to `pt-BR` — it is the closest of the shipped locales for `pt-PT` users, and shipping a separate `pt-PT` was not requested (plan).
- Overflow is fixed by layout (wrap or truncate with `title`), never by shortening translations — translations must stay complete and natural (plan).
- Each new dictionary carries a short glossary of the core terms at its top and a header note `machine-assisted, review pending` until a native speaker has reviewed it — keeps terminology consistent and the review status visible (plan).
- The `en-XA` pseudo-locale is a dev-only aid and never ships (plan).
- Keyboard key names and `{placeholders}` stay untranslated in every locale (plan; AC-06 checks placeholders).
- 2026-10-29: A Decision Log entry records the six locales (en, de, es, ja, fr, pt-BR), plural messages through an in-house `tn(key, count)` on `Intl.PluralRules` with `_one` / `_other` keys (ja: `_other` only), numbers through `Intl.NumberFormat`, and every `pt*` tag → `pt-BR`, revisiting 2026-10-12 with still no i18n dependency; `pincel.md` §9.5 is updated in the same commit — the 2026-10-12 entry explicitly asked to revisit at a third locale (owner accepted the recommendation).
- 2026-10-29: Every user-visible percentage, decimal and date anywhere in the UI goes through one `formatNumber` / `formatPercent` / `formatDate` helper; technical sizes stay unformatted (AC-09) — mixed separators on one screen look broken, and one helper costs less than deciding per call site (owner accepted the recommendation).
- 2026-10-29: Ships machine-assisted; one review issue per locale. All four new locales ship marked "machine-assisted, review pending" (header note plus glossary), with one tracking issue per locale for a native review, and the owner names any reviewer they have — the plan says not to block on reviewers, and AC-13 is a manual check anyway (owner accepted the recommendation).
- 2026-11-12: The entry moved from the View menu into the Settings dialog (Edit ▸ Settings…, Ctrl+, / ⌘,; `settings-dialog`) — the owner wanted View to hold only what the canvas shows. Its behavior is unchanged.
- 2026-11-12 (spec-build): count messages use a small ICU subset in the dictionaries, `{n, plural, one {# file} other {# files}}`, resolved by `ui/src/lib/i18n/format.ts` with `Intl.PluralRules` (no dependency; Decision Log 2026-11-12); the `(s)` fallbacks and the code-chosen `…One` / `…Many` key pairs (SVG export, keymap import) became single plural keys, so French takes "one" for 0. `percent()`, `decimal()` and `dateTime()` format the options bar, the Layers panel opacity, the Image zoom label and the Recovery dialog; the Image-mode size input and rulers write a decimal comma for de, es, fr and pt-BR (`usesDecimalComma`). Pixel mode's zoom label is a factor ("4×"), a technical value.
- 2026-11-12 (spec-build): locale detection lives in rune-free `ui/src/lib/i18n/locales.ts` (`pickLocale`), so a Node-side spec tests it. The French percent separator is whatever the runtime's CLDR writes (U+00A0 in Node 26 / CLDR 48, U+202F in older data); the test accepts either no-break space, which is what AC-09 asks for.
- 2026-11-12 (spec-build): the four dictionaries were translated by one agent each against `en.ts`, with Photoshop / Aseprite / paint.net terms; each checked key count, order and placeholders. Choices for the AC-13 review: es — frame "Fotograma", slice "Sector", marquee "Marco", feather "Calar", smudge "Dedo"; ja — Photoshop JP names (選択範囲, なげなわ, 自動選択, ぼかし, 2 階調化), image mode "イメージ"; fr — frame "image" (the Frame menu "Animation" beside Pixel mode's "Image" menu), inch "po", RGB "RVB", "{action} : échec ({error})"; pt-BR — marquee "Letreiro", tag "Etiqueta", glow "Luminescência", inch "pol". Width / height abbreviations follow each language (es An/Al, fr L/H, pt-BR L/A).
- 2026-11-12 (spec-build): `i18n` AC-02 and AC-09 were reworded for six locales (see that spec). The layout check (AC-10) found nothing to fix at 1280 px or 390 px. Red runs: the formatting and placeholder tests failed to load before `format.ts` and the dictionaries existed; the picker, detection and layout tests ran red against the two-locale build (five-language list missing, Spanish browser English); the menu-label tests failed to compile before `menu_labels.rs`.
