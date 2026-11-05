---
status: todo
created: 2026-11-05
size: L
mode: both
design: [pincel.md §5.4, pincel.md §9.2, pincel.md §9.3, pincel.md §15 (2026-09-06 ab_glyph for the Text tool), pincel.md §15 (2026-09-06 Two cores, one shell)]
---

# System fonts for the Text tools

## Context

Both Text tools rasterize text in Rust with `ab_glyph` (Decision Log 2026-09-06): `pincel-core`'s `DrawText` for Pixel mode and `pincel-image-core`'s `tools::text::Text` for Image mode. Neither core loads a font itself; the UI fetches the one bundled face, Liberation Sans Regular (SIL OFL 1.1, `ui/public/fonts/`), hands its bytes to the wasm module (`registerFont`, a byte-deduplicated registry addressed by id) and passes the id with every text command. Bold and italic are synthesized (smear / shear) because only one face ships.

paint.net's Text tool offers every font installed on the system. Pincel offers one. This spec gives both Text tools a font family list with search and preview, fed from the fonts on the user's machine, with a defined fallback when a font is missing. Since a commit is pixels (ADR-003, Decision Log 2026-09-06), nothing about the font is stored in a document: `.ora`, `.aseprite` and every export stay unchanged, and no font file is embedded or uploaded anywhere.

Where the fonts come from depends on the host:

- **PWA (Chromium):** the Local Font Access API (`queryLocalFonts()`), behind a permission prompt that needs a user gesture.
- **PWA, all browsers:** font files the user adds ("Add font file…"), kept in IndexedDB across sessions. The entry is offered while system fonts are not accessible (Firefox, Safari, iPad, permission denied or not yet granted); families added earlier stay listed in every state.
- **Desktop app:** the system's fonts enumerated natively by the Tauri shell with `fontdb` (macOS WKWebView and Linux WebKitGTK have no Local Font Access API; only Windows' WebView2 does).

The MCP side (`draw.text` with a `font` argument, a font list for agents, fonts in the headless server and in live desktop mode) is the sibling spec `system-fonts-mcp`.

### Terms

- A **face** is one font inside a file; a `.ttc` / `.otc` collection file holds several faces.
- A face's **family** is the family name its source reports: the Local Font Access `family` field in the PWA, `fontdb`'s family name on the desktop, and for an added file the typographic family name of the face (OpenType name ID 16, else the legacy family name, ID 1). Families are told apart by exact, case-sensitive name. Sources may group a font differently (a heavy weight can be its own legacy family, e.g. "Arial Black"); the font list follows the source.
- A face's **weight** and **slant** come from its source's attributes where it gives them (`fontdb`, parsed added files), otherwise from its style name: Thin 100, ExtraLight / UltraLight 200, Light 300, Regular / Normal / Book / Roman 400, Medium 500, SemiBold / DemiBold 600, Bold 700, ExtraBold / UltraBold 800, Black / Heavy 900; "Italic" or "Oblique" in the style name means slanted; an unknown style name counts as 400 upright. The width (Condensed, …) is the face's stretch; normal width is preferred when choosing.
- **Face choice** within a family: the *Regular face* is the upright face with the weight closest to 400 (ties: normal width, then the first in name order); the *Bold face* is the upright face other than the Regular face with weight ≥ 600 closest to 700; the *Italic* and *Bold Italic faces* are chosen the same way among the slanted faces. A family without upright faces uses its slanted faces as its upright ones.
- The **font list** is the family list both Text tools show. **Bundled** means the shipped Liberation Sans Regular. **System fonts** are the installed fonts the host enumerates; **added fonts** are files added with "Add font file…".
- **Reading a face** means obtaining its font bytes: a Local Font Access `blob()`, a desktop-shell file read, or loading an added file's bytes from IndexedDB. Enumerating system fonts and listing added fonts' stored names is not reading a face.
- A **text preview** is the Pixel Text dialog's live canvas preview or Image mode's text entry (AC-09); font-list entries are not text previews.

## Scope

### In scope

- A font family control in both Text tools: the Image options bar and the Pixel Text dialog
- The font list: bundled family, system families and added families, searchable, each entry shown in its own font
- System fonts in the PWA (Local Font Access API) and in the desktop app (`fontdb`)
- "Add font file…" in the PWA, kept in IndexedDB, shared by both modes
- Bold / Italic using the family's real faces when it has them, synthesis otherwise
- Faces inside font collections
- Remembering the chosen family per mode across sessions, and the fallback when it is missing
- Text previews in the chosen family

### Out of scope

- MCP `draw.text` and the headless server — because they are the sibling spec `system-fonts-mcp`
- Per-glyph fallback (a character the chosen font lacks borrows another font's glyph) — because it needs a fallback chain and shaping; a missing glyph renders as the font's `.notdef` glyph as today, and a user can pick a font that covers the script
- Complex-script shaping (ligatures, Arabic joining, Indic reordering) and right-to-left text — because `ab_glyph` lays out glyph by glyph; it needs a shaping engine, a dependency decision of its own
- A style dropdown and variable-font axes — because paint.net offers Bold / Italic toggles only; other weights are reachable where the source lists them as their own family, and a variable font renders at its default instance
- Underline, strikethrough and on-canvas editing of the open text — because `image-live-text` owns them; that spec keeps the font as one property of the live text, which this spec's family choice fills. Text that stays editable after commit is out too — because it needs a text layer, which ADR-003 rules out
- Removing an added font from the list — because paint.net has no font management either and an unused entry costs only IndexedDB space; clearing the site data removes them
- "Add font file…" in the desktop app — because the desktop app lists every installed font; a user installs a font through the system
- Bundling more fonts (e.g. a CJK face, backlog "CJK font for the Text tool") — because system and added fonts already cover CJK fonts, and bundling is an asset-size decision of its own
- Storing a font reference in documents — because committed text is pixels and no format of Pincel's carries text
- Web fonts (`.woff`, `.woff2`), bitmap-only and Type 1 fonts (`.fon`, `.pfb`) — because `ab_glyph` reads TrueType / OpenType outlines only

## Acceptance criteria

- [ ] AC-01 (always): Both Text tools have a font family control showing the current family, and the font list always contains "Liberation Sans", also with no system or added fonts available. While system fonts are not accessible in the PWA (API unsupported, permission denied or not yet granted), the font list also offers "Add font file…".
- [ ] AC-02 (event): When the user opens the font list, then it shows each family of the bundled, system and added fonts exactly once, sorted case-insensitively by name, however many faces each family has.
- [ ] AC-03 (event): When the user types into the font list's search field, then only families whose name contains the typed text (case-insensitive) stay listed, an empty field lists all families, and with no match the list says so.
- [ ] AC-04 (always): Each family entry in the font list renders its name in that family's own font.
- [ ] AC-05 (always): Opening, searching and scrolling the font list reads no system-font face; a system-font face is read only when it is used for a text preview or a commit, and no face is read more than once per session.
- [ ] AC-06 (event): When the user picks a family and places text, then the committed pixels equal those the core produces for that family's Regular face, with size, color, alignment, anti-alias and the undo behavior unchanged.
- [ ] AC-07 (option): Where Bold, Italic or both are on and the chosen family has the matching Bold, Italic or Bold Italic face, the text is rasterized with that face and without synthesized bold or italic. Where the family lacks it, the nearest available face is synthesized: Bold Italic falls back to the Bold face plus synthesized italic, then the Italic face plus synthesized bold, then the Regular face with both; Bold or Italic alone falls back to the Regular face with that one synthesized.
- [ ] AC-08 (event): When the face to use is one face of a font collection, then the text is rasterized with that face, not with the collection's first face.
- [ ] AC-09 (state): While the Pixel Text dialog is open, its live preview on the canvas is drawn in the face a commit would use and follows a change of family, Bold or Italic; while Image mode's text entry is open, it displays the typed text in that face. While a newly chosen face is still being read, the preview keeps showing the previous face.
- [ ] AC-10 (event): When the user picks a family in one mode, then that mode's Text tool shows the same family after a reload of the app, and the other mode's choice is unchanged.
- [ ] AC-11 (error): If the current family is unavailable when the Text tool needs it (uninstalled, its face unreadable or unparseable, the added font gone from IndexedDB, or system-font access denied), then the tool uses Liberation Sans, the family control shows the missing family's name marked as missing, the status bar says that Liberation Sans is used instead, and placing text still works. The status message appears the first time per session that the tool needs the missing family (Image mode: selecting the Text tool; Pixel mode: opening the Text dialog).
- [ ] AC-12 (event): When the user opens the font list in the PWA while local-font access is not yet granted, then the browser's permission prompt is requested from that click, and on approval the system families appear in the list without reopening it.
- [ ] AC-13 (error): If local-font access is denied, dismissed or not supported, then the font list shows the bundled and added families, a note in the list says why system fonts are missing (denied: how to allow them in the site settings; dismissed: that opening the list again asks again; unsupported: that this browser cannot list system fonts) and points to "Add font file…", and no error dialog appears.
- [ ] AC-14 (state): While local-font access is granted, the remembered family is available at app start without a prompt and without opening the font list.
- [ ] AC-22 (state): While local-font access is not yet decided and the remembered family is a system family, the family control shows that family without the missing mark, and the first press with the Text tool requests the permission prompt from that press; on approval the family is used, otherwise AC-11 applies.
- [ ] AC-15 (event): When the font list is opened in the desktop app, then it lists the system families the desktop shell reports, without any permission prompt, and text placed with one of them equals the core's output for that family's Regular face.
- [ ] AC-16 (always): The desktop font list contains only families with at least one TrueType / OpenType face; in the PWA, a listed face the core cannot rasterize is handled by AC-11 when it is used.
- [ ] AC-17 (always): Listing, previewing and using fonts makes no network request beyond loading the app, and a saved or exported document is byte-identical to one with the same pixels made while Liberation Sans was selected.
- [ ] AC-23 (always): The "Liberation Sans" entry always rasterizes upright, normal-weight text with the bundled face; Liberation Sans faces among the system or added fonts join that entry only as its Bold, Italic and Bold Italic faces.
- [ ] AC-18 (event): When text is committed while the face to use is still being read, then the commit waits for it with a loading notice in the status bar and uses the face of the family current at commit time; if reading fails, AC-11 applies to that commit.
- [ ] AC-20 (event): When the user adds one or more font files through "Add font file…" in either mode, then each face in them (every face of a collection) joins its family in the font list of both modes, the first added family becomes the current family of the mode it was added in, and the added families stay listed and usable after a reload, also after local-font access is granted later.
- [ ] AC-21 (error): If one or more files of a pick are not TrueType / OpenType fonts or collections the core can rasterize, or cannot be stored, then those files are not added, one status message names each rejected file with its reason, the other files of the pick are added, and the current family changes only if one was added.
- [ ] AC-24 (event): When an added face has the same family and style name as a face added earlier, then it replaces the earlier one, and the font list still shows that family once.
- [ ] AC-19 (state, manual: real installs on three operating systems): While the desktop app runs, every TrueType / OpenType family listed by the system's font list (Windows: Settings ▸ Personalization ▸ Fonts; macOS: Font Book; Linux: `fc-list : family`) can be found with the font list's search, possibly under the name `fontdb` reports for it.

## Technical notes

- **Cores:** `ab_glyph` stays the rasterizer; no new core dependency. Both cores already take `&[u8]` font bytes. Collections need a face index: `ab_glyph` parses index 0 (`FontVec::try_from_vec` / `FontRef::try_from_slice`); `try_from_vec_and_index` exists. Real Bold / Italic faces mean the commit names one face and only the missing style is synthesized; the shell chooses the face, the cores do not learn about families.
- **Wasm:** `registerFont` in `pincel-wasm` (`crates/pincel-wasm/src/text.rs`) and `register_font` in `pincel-image-wasm` (`crates/pincel-image-wasm/src/lib.rs`) take bytes only today; both become `registerFont(bytes, faceIndex?)` with the index defaulting to 0, so existing callers keep working. A registered font is identified by bytes plus face index. Both registries keep every blob forever and deduplicate by comparing whole blobs; faces are registered lazily (AC-05), so the count stays small.
- **UI:** one font source module and one font list component under `ui/src/lib/` serve both modes (AGENTS.md §5.4). It replaces the two separate loaders of the bundled font (`ui/src/lib/text/font.ts` for Pixel mode, `ensureFont()` in `ui/src/modes/image/core/controller.ts`). The remembered family is an IndexedDB pref (`lib/idb/prefs.ts`), one per mode.
- **Font-list rendering (AC-04, AC-05):** system families render by their CSS family name, which the browser resolves to the installed font without Pincel reading it. The bundled and added families render through a `FontFace` built from their bytes (in memory or IndexedDB), which AC-05 allows. The desktop CSP (`font-src 'self' data:`) has to allow whatever source is used.
- **Image text entry:** a DOM `<textarea>` styled `font-family: 'Liberation Sans', Arial, sans-serif` (`MainCanvas.svelte`), but the app registers no `@font-face` for Liberation Sans, so the entry shows Arial or a system fallback today, not the committed face. AC-09 fixes this. If `image-live-text` lands first, the entry is a core-rendered live text and AC-09 holds by passing it the chosen face.
- **PWA:** `queryLocalFonts()` returns `FontData` (`family`, `style`, `fullName`, `postscriptName`, `blob()`); the permission is `local-fonts` (`navigator.permissions.query`: `granted` / `denied` / `prompt`), and requesting it needs transient user activation (a click on the font list, AC-12, or a Text-tool press, AC-22). `FontData` carries no face index, and whether `blob()` returns a whole collection or a single face is not checked. AC-08 does not depend on it: the face is found by its `postscriptName` among the faces in the returned bytes, index 0 when the bytes hold one face.
- **Desktop:** `fontdb` in `src-tauri` enumerates the system fonts; a Tauri command lists families / faces (family, style, weight, slant, stretch, PostScript name, an opaque face id) and another returns one face's bytes and face index by that id. The handlers stay thin (AGENTS.md §5.5), and the face read must not widen the open file-read surface (backlog "Scope Tauri file commands"): it reads only files the enumerator returned.
- **Added fonts:** the PWA parses each added file once when it is added, and stores its bytes plus each face's family, style, weight, slant, stretch and face index in IndexedDB (`lib/idb/`), so listing reads only the stored names.
- **Tests:** e2e fakes `window.queryLocalFonts` and `navigator.permissions` with `addInitScript` (as `ui/e2e/shell-open.spec.ts` fakes pickers) and counts `blob()` calls (AC-05); desktop paths run against the mocked Tauri bridge (`ui/e2e/tauri-mock.ts`). The face-choice rules (Terms) are pure functions with unit coverage through Playwright's Node-side specs (rune-free module, AGENTS.md Gotchas). Fixtures: a second OFL family with Bold and Italic faces and a small collection built from it (e.g. Liberation Serif), next to `crates/pincel-core/tests/fixtures/LiberationSans-Regular.ttf`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 2,000 installed families | The list opens and filters without reading a system face (AC-05); only visible entries need to render |
| Family whose only face is weight 900 ("Arial Black" as its own legacy family) | That face is the Regular face; Bold on synthesizes bold (no other face ≥ 600) |
| Family with Light, Regular, SemiBold, Bold | Regular = 400, Bold = 700; SemiBold is used only when the family has no 700 face |
| Family with Bold but no Bold Italic, Bold and Italic both on | Bold face plus synthesized italic (AC-07) |
| `.ttc` collection face (e.g. a face in `msgothic.ttc`) | That face is used, not face 0 (AC-08) |
| Remembered family uninstalled since the last session | Fallback per AC-11; the pref keeps the name so the family is used again once reinstalled |
| Font removed between opening the list and placing text | Reading fails; AC-11 for that commit, no partly placed text |
| 20 MB CJK font | The commit waits with a loading notice (AC-18); later commits reuse the face (AC-05) |
| Family changed while the previous face is still loading | The commit uses the family current at commit time (AC-18); the preview shows the previous face until the new one is read (AC-09) |
| Text with characters the font lacks | `.notdef` glyph as today (out of scope: per-glyph fallback) |
| Permission prompt dismissed in the PWA | Bundled and added families with the "dismissed" note (AC-13); opening the list again asks again, as far as the browser allows |
| Liberation Sans installed on the system or added | Joins the bundled entry as Bold / Italic faces only; upright regular text stays the bundled face (AC-23) |
| Pixel and Image tabs open at once | Each mode keeps its own family (AC-10); added fonts are shared (AC-20); a face used by both modes is read once (AC-05) |
| Two faces of one family share a file | The file is read once per session |
| An added file repeats a family and style added earlier | The newer face replaces the older one (AC-24) |
| IndexedDB cleared by the browser | Added families are gone; a remembered added family falls back per AC-11 |
| A pick with a `.woff2`, an image and a valid `.ttf` | The `.ttf` is added; one message names the two rejected files and their reasons (AC-21) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Bold / Italic stay toggles, no style dropdown | paint.net has toggles only, and the toggles already exist in both modes | Weights other than Regular / Bold are reachable only where the source lists them as their own family |
| The chosen family is remembered per mode, added fonts are shared | Pixel and Image text already keep separate sizes and anti-alias defaults, so a sprite font and a caption font are different choices; an added file is a font the user owns, not a per-mode setting | A user expecting one shared choice picks twice |
| System fonts are enumerated lazily: on the first font-list open or Text-tool press, or at start when access is granted | `queryLocalFonts()` needs a user gesture the first time; reading faces up front would take seconds with thousands of fonts | The first list open waits for the enumeration |
| A PWA face that fails to parse is found only when it is used | Parsing every installed face to filter the list means reading all font files, against AC-05 | Such a family stays listed until first used, then falls back per AC-11 |
| Font licenses allow rasterizing text into an image | Typical desktop font licenses permit output use; nothing is embedded or redistributed (AC-17) | A restrictive font is the user's choice, as in paint.net |
| Families are told apart by exact name | Exact names are what the sources report and what MCP `font` matches first (`system-fonts-mcp`) | Two families differing only in case are both listed |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

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
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |
| AC-23 |      | open   |        |
| AC-24 |      | open   |        |

## Decisions

- 2026-11-05: Fonts stay outside the cores. The shell finds, reads and chooses faces; the cores keep receiving font bytes, so §5.1's no-platform-deps rule holds and the core APIs gain at most a face index — because enumeration is platform work, and the same cores also run in wasm where only the browser can list fonts.
- 2026-11-05: An unavailable font falls back to Liberation Sans with a visible notice instead of blocking the tool — because a missing font must never stop a user from placing text, and the notice keeps the substitution from going unnoticed.
- 2026-11-05 (owner's choice): The desktop app enumerates system fonts with `fontdb`, a new `src-tauri` dependency — because it is pure Rust, reuses the `ttf-parser` 0.25 that `ab_glyph` already brings, and works the same on Windows, macOS and Linux, where the webviews' font APIs differ.
- 2026-11-05 (owner's choice): Where system fonts are not accessible in the PWA, the user adds font files through "Add font file…" and they are kept in IndexedDB across sessions — because it is the only way to any other font in Firefox, Safari and on iPad, the fonts stay on the machine, and IndexedDB is the allowed store (AGENTS.md §9).
- 2026-11-05 (owner's choice): Collection faces reach the cores through `registerFont(bytes, faceIndex?)`, the index defaulting to 0 — because it is non-breaking for existing callers of the published `pincel-wasm` API and keeps one registration function in both wasm crates.
- 2026-11-05 (clarification pass): Faces are chosen by weight and slant, derived from style names where the source gives no attributes — because Local Font Access reports only a style string while `fontdb` and parsed files report attributes, and one rule must give the same choice on every host.
- 2026-11-05 (clarification pass): The bundled face always serves upright regular "Liberation Sans" — because the default output must not change between machines that do or do not have Liberation Sans installed (AC-17, and MCP `draw.text` without `font`).
- 2026-11-05 (clarification pass): A newer added face with the same family and style replaces the older one — because the font list and the Bold / Italic toggles address faces by family and style only, two such faces could not be told apart, and the newer pick is the user's latest intent (e.g. an updated version of the font).
- 2026-11-05 (clarification pass): A system family remembered while the permission is undecided asks for it on the first Text-tool press — because that press is a user gesture, so the remembered choice works without forcing the user to open the font list first.
