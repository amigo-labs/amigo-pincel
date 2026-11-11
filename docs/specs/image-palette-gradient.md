---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §9.2, pincel.md §3.7, pincel.md §15 (2026-09-06 Two cores, one shell), pincel.md §15 (2026-10-29 own color picker), pincel.md §15 (2026-10-29 palette formats), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [color-picker]
---

# Image mode: load, save and extend the Colors panel palette

## Context

paint.net's Colors window has a palette the user can replace: Save Palette As…, open a saved palette, Reset to Default, and the primary color can be put into the palette. Pincel's Image mode is meant to reach paint.net parity (owner, 2026-11-05).

The revised `color-picker` spec (2026-11-05) gives Image mode's Colors panel an always-visible recent-colors row and a fixed default palette of 48 colors (its AC-25 – AC-29), and puts "editing, saving or loading the Image-mode default palette" out of scope because a user palette needs its own storage and UI. This spec supplies that storage and UI: the palette becomes replaceable by loading a palette file, savable as a file, resettable to the default, and extendable by the current foreground color.

**This spec amends `color-picker`.** Its AC-25 ("always shows … the default palette of 48 colors") becomes: the Colors panel always shows Image mode's palette, which is the 48-color default of its AC-27 until the user loads a palette or adds a color, and again after Reset to default palette (AC-12, AC-22 here). Its out-of-scope item "Editing, saving or loading the Image-mode default palette" is replaced by this spec; editing single entries stays out of scope here. The narrow-sidebar rule is one rule for both specs: the palette wraps into more rows, as `color-picker`'s edge case says, and scrolls only past a height limit (AC-19).

Pixel mode already reads and writes four palette formats — GIMP `.gpl`, Lospec `.hex`, JASC `.pal` and PNG palette images (`palette-formats`, Decision Log 2026-10-29). Image mode loads and saves the same four formats with the same rules, so a palette file moves between the two modes unchanged.

Two facts shape the design:

- **The Image-mode palette is app state, not document state.** An Image document (PNG, JPEG, …) has no palette. The palette belongs to Image mode as a whole: the same in every Image tab, persisted in IndexedDB like the recent colors (`color-picker` AC-15), never in the document and never on the undo stack. Pixel mode's palette stays document state inside the `.aseprite` file and is not touched.
- **The palette codecs live in `pincel-core`, Image mode in `pincel-image-core`.** The two cores stay separate (Decision Log 2026-09-06). The owner chose to move the codecs into a new small shared, platform-free crate `pincel-palette` that both modes use (see Decisions), as `pincel-effects` and `pincel-trace` are shared today; for Pixel mode the move changes no behavior.

Because the palette is not document state, it is not a "document-changing capability" in the sense of the MCP tool parity decision (Decision Log 2026-11-05): no MCP tool is added, and `palette.*` on an Image document keeps failing with the mode error (`mcp-parity-structure` AC-02).

The Gradient tool's spiral kinds, the second paint.net parity item of this topic, are in `image-palette-gradient-spiral.md`; the split keeps each spec within size L.

## Scope

### In scope

- A palette menu on Image mode's Colors panel: Load palette…, a Save palette submenu (GIMP `.gpl`, Lospec `.hex`, JASC `.pal`, PNG strip 1 px, PNG strip 8 px) and Reset to default palette
- An Add color button that appends the current foreground color (with its alpha) to the palette
- Loading detects the format from the content, with the same reading rules and limits as Pixel mode's palette import
- The palette persists across reloads and desktop restarts, in IndexedDB, for Image mode as a whole
- Entry names from a `.gpl` file are kept and shown in the swatch's tooltip and accessible name
- English and German labels and messages
- Moving the palette codecs (`.gpl`, `.hex`, `.pal`, PNG palette images) out of `pincel-core` into a shared crate `pincel-palette` used by both modes, without changing Pixel mode's behavior

### Out of scope

- Removing, recoloring or reordering single palette entries — because the parity target is paint.net's load / save / reset / add, and per-entry editing needs a selection model on the swatches (Pixel mode's palette panel has one; Image mode would need its own); editing the file and loading it again covers the rare case
- paint.net's own `.txt` palette format (`AARRGGBB` lines) — because it is a new file format for both modes and belongs next to `palette-formats` as its own spec, like the Adobe `.ase` / `.aco` backlog row
- A list of several saved palettes inside the app (paint.net's palettes folder) — because files already store palettes, and a managed library needs its own storage, naming and UI
- Any change to Pixel mode's palette panel or to the palettes stored in Pixel documents — because Pixel mode's palette is document state with its own spec (`pixel-palette`, `palette-formats`)
- MCP tools for the Image-mode palette — because the palette is not document state, and the MCP tool parity decision covers document-changing capabilities only
- Undo for palette changes — because the palette is app state like the recent colors and the foreground color, which are not on the document's undo stack either
- Synchronizing the palette live between two open browser windows of the PWA — because no other preference is synchronized live; the window that changed the palette last wins at the next start

## Acceptance criteria

- [ ] AC-01 (always): Image mode's Colors panel has a palette menu offering Load palette…, a Save palette submenu with the entries GIMP `.gpl`, Lospec `.hex`, JASC `.pal`, "PNG strip (1 px)" and "PNG strip (8 px)", and Reset to default palette, plus an Add color button; each has an accessible name.
- [ ] AC-02 (event): When the user loads a palette file, then the palette in the Colors panel is replaced by the file's colors, and the status bar shows "loaded {count} palette colours" (German: "{count} Palettenfarben geladen") with the number of loaded colors.
- [ ] AC-03 (always): Loading a file in Image mode yields the same colors (R, G, B, A) in the same order as Pixel mode's Palette ▸ Import of the same file: the format is detected from the content, not the file name (a `.txt` file with hex lines loads as `.hex`), and the reading rules of `palette-formats` AC-02 – AC-05 apply.
- [ ] AC-04 (error): If a loaded file cannot be read, then the palette stays unchanged and the status bar shows "Load palette failed: {reason}" (German through the same pattern), where the reason is the one Pixel mode's import gives: for a `.gpl`, `.hex` or `.pal` colour line that cannot be read, its 1-based line number; for a `.pal` without its `0100` or count line, the missing header; for more than 256 entries, the limit; for a file without colour entries, "no colours found"; for a file with the PNG signature that cannot be decoded, the decode failure; for a PNG with more than 256 distinct non-transparent colours, "not a palette image". Bytes that are not PNG, `.gpl` or `.pal` are read as `.hex` (`palette-formats` AC-02), so an arbitrary file fails with the `.hex` error for its first unreadable line.
- [ ] AC-05 (event): When the user dismisses the file picker of Load palette… without choosing a file, then the palette stays unchanged and no message is shown.
- [ ] AC-06 (event): When the user chooses an entry of the Save palette submenu, then a file in that format is written as in Pixel mode's export (`palette-formats` AC-10 – AC-12), with the suggested name `palette.gpl`, `palette.hex`, `palette.pal` or `palette.png`, and the status bar shows "saved {count} palette colours" (German: "{count} Palettenfarben gespeichert") with the number of saved entries.
- [ ] AC-07 (always): Saving the palette in any of the five Save palette entries and loading that file again gives the same colors in the same order, with alpha set to 255 for `.gpl`, `.hex` and `.pal`, which store none; for a PNG strip this holds for a palette without duplicate and without fully transparent entries.
- [ ] AC-08 (always): A `.gpl` file with named entries, loaded and saved again as `.gpl`, keeps every entry's name; a swatch with a name shows it in its tooltip and accessible name next to its hex value.
- [ ] AC-09 (event): When the user presses Add color, then the current foreground color, including its alpha, is appended as the last palette entry.
- [ ] AC-10 (error): If the user presses Add color while the palette holds 256 entries and none of them equals the foreground color, then the palette stays unchanged and the status bar says that the palette is full.
- [ ] AC-11 (error): If the user presses Add color while an entry with the same R, G, B and A as the foreground color is already in the palette, then the palette stays unchanged and the status bar says that the color is already in the palette — also when the palette holds 256 entries, because the duplicate is the more useful message.
- [ ] AC-12 (event): When the user chooses Reset to default palette, then the palette becomes the default palette of `color-picker` AC-27 (48 colors), without a confirmation prompt.
- [ ] AC-13 (event): When the app is reloaded, or the desktop app is quit and started again, then Image mode's palette is restored with the same colors, alpha and names in the same order as before.
- [ ] AC-14 (error): If the stored palette cannot be read (not a list, malformed entries), then Image mode starts with the stored entries that are readable, at most the first 256 of them, or with the default palette when none is readable; no error is shown.
- [ ] AC-15 (always): Loading, saving, adding to or resetting the palette adds no undo step to any document and does not mark any document as having unsaved changes.
- [ ] AC-16 (state): While several Image documents are open in tabs, every Image tab shows the same palette, and a change made in one tab shows in the others when they become active.
- [ ] AC-17 (always): No Image-mode palette action changes the palette of a Pixel document or what Pixel mode's palette panel shows, and no Pixel-mode palette action changes Image mode's palette.
- [ ] AC-18 (always): A loaded or extended palette's swatches behave as the default palette's (`color-picker` AC-26): a left click sets the foreground color, a right click or long press the background color, each including alpha, and no context menu opens.
- [ ] AC-19 (state): While the palette holds more swatches than fit in one row of the Colors panel, the palette wraps into more rows with every swatch at least 16 px and no horizontal scrolling; while it needs more than 8 rows, the palette area is 8 rows of swatches high and scrolls vertically on its own, and the foreground / background swatches and the recent-colors row stay visible.
- [ ] AC-20 (option): Where the UI language is German, the palette menu, its entries, the Add color button and the palette's status messages are German.
- [ ] AC-21 (always): Pixel mode reads and writes palette files exactly as before the codecs were shared: the `pincel-core` and `pincel-wasm` palette tests (`.gpl` import / export, `importGpl` / `exportGpl` / `importPalette` / `exportPalette`) and the e2e palette specs stay green with unchanged assertions, and the codec tests of `palette-formats` AC-02 – AC-15 that move to the shared code keep their input bytes and their expected colors, order, alpha, names, errors and line numbers — only the type they are written against may change.
- [ ] AC-22 (state): While the user has not loaded a palette or added a color since the last Reset to default palette (or ever), Image mode stores no palette of its own and shows the built-in default palette of the running app version, so a later change of the built-in default reaches that user; after a load or an add, the stored palette is shown instead.
- [ ] AC-23 (event): When the user dismisses the save dialog of a Save palette entry, then nothing is written and no message is shown.
- [ ] AC-24 (error): If writing the saved palette file fails, then the status bar shows "Save palette failed: {reason}" and the palette is unchanged.
- [ ] AC-25 (error): If storing the changed palette in IndexedDB fails (e.g. the storage quota is full), then the change stays in effect until the app is reloaded and the status bar says that the palette could not be stored.

## Technical notes

**Persistence.** IndexedDB pref through `ui/src/lib/idb/prefs.ts` (`getPref` / `setPref`), e.g. key `palette:image`, holding the entries as `{ rgba: number; name?: string }[]`; never `localStorage` (AGENTS.md §9). The pref exists only once the user loaded a palette or added a color; Reset removes it (`removePref`), so the built-in default of the running version shows (AC-22).

**File picking.** `pickAndOpen('palette')` and `saveExport(bytes, name, format)` from `ui/src/lib/fs` already serve Pixel mode's palette panel (`ui/src/modes/pixel/components/PalettePanel.svelte`, whose ⋯ menu is the pattern for the menu and the export submenu) and work in the PWA and the desktop app.

**Codecs.** `crates/pincel-core/src/codec/palette_formats.rs` (`detect_palette_format`, `read_palette`, `write_hex`, `write_jasc_pal`, palette PNG strip) and `codec/gpl.rs`, over `pincel-core`'s `Palette` / `PaletteEntry` / `Rgba`, capped at `MAX_PALETTE_ENTRIES` (256). They move into the new crate `crates/pincel-palette` (platform-free, no I/O, `thiserror`, `png` from the workspace; the rules of AGENTS.md §5.1) with its own palette entry type (RGBA plus optional name) and the 256 limit; `pincel-core` uses it and converts to its `Palette` (keeping its public re-exports so callers do not change), `pincel-image-wasm` (through `pincel-image-core` or directly) exposes load / save to `ui/src/modes/image/core/wasm.ts` (AGENTS.md §5.4). The existing codec tests move with the code; the `pincel-core` / `pincel-wasm` palette tests stay where they are. The move is its own commit series before the Image-mode work, and the clippy / test lists in AGENTS.md §10 and `.agents/gates.json` gain the crate (the latter with the owner's approval, AGENTS.md "Quality gates").

**Panel.** `ui/src/modes/image/components/panels/ColorsPanel.svelte` (today 18 lines: the shared `ColorPair`); `color-picker` adds the recent row and the default palette there. Status messages go through `editor.notice` (Image mode's status bar).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `.pal` file whose fourth column makes an entry fully transparent | The entry loads and shows as a transparent swatch over the checkerboard; clicking it sets a foreground with alpha 0 (AC-03, AC-18) |
| Loaded `.gpl` file containing the same color twice | Both entries load, because the file is taken as it is (AC-03); Add color refuses that color (AC-11) |
| Palette with alpha entries saved as `.hex` | The file has RGB only; loading it back gives the colors opaque (AC-07) |
| Palette of 256 entries in a 240 px wide sidebar | The palette wraps into rows, is 8 rows high and scrolls vertically; swatches stay at least 16 px (AC-19) |
| Add color with the foreground already in a full (256-entry) palette | "Already in the palette"; palette unchanged (AC-11) |
| Stored palette with 300 readable entries (written by a faulty build) | The first 256 are used (AC-14) |
| File with the PNG signature but truncated data | "Load palette failed" with the decode failure; palette unchanged (AC-04) |
| PNG photo with thousands of colors chosen in Load palette… | Error "not a palette image"; palette unchanged (AC-04) |
| Two PWA windows change the palette | Each window keeps its own palette until reload; the next start shows the last stored one (out of scope: live sync) |
| Add color pressed with a half-transparent foreground (`#11223380`) | Entry `#11223380` is appended; saved as `.gpl` it comes back as `#112233` opaque (AC-09, AC-07) |
| Palette reset while the user had added colors | The added colors are gone without a prompt; saving first keeps them (AC-12) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The palette is one for Image mode as a whole, not one per document | Image documents carry no palette, and paint.net's palette is app-wide | Users who want a palette per project must save and load files |
| Reset needs no confirmation | paint.net's Reset to Default does not ask; a palette worth keeping can be saved as a file first | A user loses added colors by one misclick |
| Add color takes the foreground, not the background | paint.net adds the primary color; the left button and the foreground are Pincel's primary everywhere (§9.2) | None known |
| Load uses the 256-entry limit and reading rules of Pixel mode | One file then behaves the same in both modes (AC-03), and the codecs already enforce it | paint.net palettes hold 96 colors, so no real paint.net palette hits the limit |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `the Colors panel has the palette menu, its Save submenu and Add color (AC-01)` — `ui/e2e/image-palette.spec.ts` | built |        |
| AC-02 | `loading a .txt file with hex lines replaces the palette and says how many (AC-02, AC-03)` — `ui/e2e/image-palette.spec.ts` | built |        |
| AC-03 | `loading a .txt file with hex lines replaces the palette and says how many (AC-02, AC-03)`, `a file loads the same colors in the same order as Pixel mode imports it (AC-03)` — `ui/e2e/image-palette.spec.ts`; `palette::tests::read_detects_the_format_from_the_content` — `crates/pincel-image-wasm/src/palette.rs` | built |        |
| AC-04 | `an unreadable file keeps the palette and names the reason (AC-04)` — `ui/e2e/image-palette.spec.ts`; `palette::tests::read_unreadable_file_names_the_reason` — `crates/pincel-image-wasm/src/palette.rs` | built |        |
| AC-05 | `dismissing the file picker of Load palette… changes nothing and says nothing (AC-05)` — `ui/e2e/image-palette.spec.ts` | built |        |
| AC-06 | `each Save palette entry writes its format; loading it back gives the colors (AC-06, AC-07)` — `ui/e2e/image-palette.spec.ts` | built |        |
| AC-07 | `each Save palette entry writes its format; loading it back gives the colors (AC-06, AC-07)` — `ui/e2e/image-palette.spec.ts`; `palette::tests::write_then_read_round_trips_every_format` — `crates/pincel-image-wasm/src/palette.rs` | built |        |
| AC-08 | `.gpl entry names survive load and save and show on the swatches (AC-08)` — `ui/e2e/image-palette.spec.ts`; `palette::tests::write_gpl_keeps_entry_names` — `crates/pincel-image-wasm/src/palette.rs` | built |        |
| AC-09 | `Add color appends the foreground with its alpha, refuses a duplicate; Reset brings the default back (AC-09, AC-11, AC-12, AC-15, AC-22)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-10 | `Add color on a full palette says it is full, unless the color is in it (AC-10, AC-11)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-11 | `Add color appends the foreground with its alpha, refuses a duplicate; Reset brings the default back (AC-09, AC-11, AC-12, AC-15, AC-22)`, `Add color on a full palette says it is full, unless the color is in it (AC-10, AC-11)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-12 | `Add color appends the foreground with its alpha, refuses a duplicate; Reset brings the default back (AC-09, AC-11, AC-12, AC-15, AC-22)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-13 | `the palette survives a reload with colors, alpha and names (AC-13)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-14 | `an unreadable stored palette falls back silently (AC-14)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-15 | `Add color appends the foreground with its alpha, refuses a duplicate; Reset brings the default back (AC-09, AC-11, AC-12, AC-15, AC-22)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-16 | `every Image tab shows the same palette (AC-16)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-17 | `Image and Pixel palettes do not touch each other (AC-17)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-18 | `loaded swatches pick foreground and background with alpha, without a context menu (AC-18)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-19 | `a 256-color palette wraps, stops at 8 rows and scrolls on its own in a 240 px sidebar (AC-19)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-20 | `the palette menu, Add color and the palette messages are German in the German UI (AC-20)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-21 | `pincel-palette`: `formats::tests::*` (15 codec tests), `gpl::tests::*` (4) — `crates/pincel-palette/src/{formats,gpl}.rs`; `pincel-core`: `codec::gpl::tests::*`, `codec::palette_formats::tests::*` (unchanged assertions); `pincel-wasm`: `tests::import_palette_*`, `tests::export_palette_writes_each_format`, `tests::palette_presets_and_gpl_round_trip_through_undo`, `tests::import_gpl_invalid_file_returns_err_and_keeps_palette`; `pincel-agent`: `tests::tags_palette::palette_*`; e2e `ui/e2e/palette.spec.ts` | built |        |
| AC-22 | `Add color appends the foreground with its alpha, refuses a duplicate; Reset brings the default back (AC-09, AC-11, AC-12, AC-15, AC-22)` — `ui/e2e/image-palette-state.spec.ts` | built |        |
| AC-23 | `dismissing the save dialog writes nothing and says nothing (AC-23)` — `ui/e2e/image-palette.spec.ts` | built |        |
| AC-24 | `a failed write says so and keeps the palette (AC-24)` — `ui/e2e/image-palette.spec.ts` | built |        |
| AC-25 | `a palette that cannot be stored stays until reload and says so (AC-25)` — `ui/e2e/image-palette-state.spec.ts` | built |        |

## Decisions

- 2026-11-05: The topic "palette and gradient" is split into this spec (palette) and `image-palette-gradient-spiral.md` (spiral gradient kinds) — together they exceed size L (AGENTS.md §3.2), and the two share no code.
- 2026-11-05: The Image-mode palette is app state persisted in IndexedDB for Image mode only. "Per mode" in the request does not extend to Pixel mode, because Pixel mode's palette is document state stored in the `.aseprite` file (§3.7).
- 2026-11-05: No MCP tool for the Image-mode palette — the MCP tool parity decision (2026-11-05) covers document-changing capabilities, and this palette changes no document.
- 2026-11-05: Add color refuses a color that is already in the palette (AC-11), while Load keeps duplicates from the file (edge cases) — a pick palette gains nothing from a second identical swatch, but a loaded file is taken as it is so a load → save round trip stays faithful.
- 2026-11-05: Load and save offer exactly Pixel mode's formats and export entries — one palette file then works in both modes, and the codecs and their tests already exist.
- 2026-11-05: The palette codecs (`palette_formats.rs` and the `.gpl` reader / writer) move from `pincel-core` into a new small shared, platform-free crate `pincel-palette`, used by `pincel-core` and by `pincel-image-core` / `pincel-image-wasm` — owner's choice (option A of three; the others were a `pincel-image-wasm` → `pincel-core` dependency and calling the Pixel wasm adapter from the Image editor), because one copy keeps a palette file identical in both modes (AC-03) without linking Pixel mode's core into Image mode or bypassing the Image wasm adapter, as with the gradient math (Decision Log 2026-10-29). The move preserves Pixel mode's behavior (AC-21). Recorded as Decision Log 2026-11-05.
- 2026-11-05: This spec amends `color-picker` AC-25 and replaces its out-of-scope item on loading / saving the Image-mode palette (see Context), because both specs describe the same panel and must not contradict each other; `color-picker` is revised to match.
- 2026-11-05: Narrow sidebar: wrap into rows first, scroll vertically past 8 rows (AC-19) — wrapping is `color-picker`'s rule for the 48 defaults, and a cap of 8 rows keeps a 256-color palette from pushing the rest of the sidebar out of view.
- 2026-11-05: Reset removes the stored palette instead of storing the default (AC-22), because users who never customized their palette should get a later improved default.
- 2026-11-05: A failed IndexedDB write keeps the change for the session and says so (AC-25), because discarding a change the user just made is worse than losing it at reload, and the notice lets the user save the palette as a file first.
- 2026-11-05: Status messages reuse Pixel mode's wording and the shared `status.failed` pattern ("{action} failed: {error}"), because one vocabulary across modes keeps strings and translations small.
- 2026-11-05: A duplicate wins over a full palette in Add color's message (AC-10, AC-11), because "already in the palette" tells the user the color is available, while "full" would suggest it is missing.
- 2026-11-11 (spec-build): `pincel-core` keeps `codec/gpl.rs` and `codec/palette_formats.rs` as thin adapters over `pincel-palette` (same function names, signatures and re-exports; `GplError`, `PaletteFormat`, `PaletteFormatError` are re-exported from the new crate), so callers and the existing `pincel-core` codec tests stay unchanged; the same tests are ported to `pincel-palette`'s `Vec<PaletteEntry>` with their input bytes and expectations. `MAX_PALETTE_ENTRIES` is now `pincel_palette::MAX_ENTRIES` (256), one limit for both. The AC-21 tests were green at once: the code moved unchanged, so there is no red run; the PNG decode / encode error texts are kept byte for byte (including the doubled `png decode failed:` prefix `pincel-core` produced), because AC-21 asks for unchanged errors.
- 2026-11-11 (spec-build): `pincel-image-wasm` reads and writes palette files directly through `pincel-palette` (`read_palette_file` → `PaletteFile` with packed `0xRRGGBBAA` colors and names, `write_palette_file(colors, names, format, name, cell)`); `pincel-image-core` is not involved, because the palette is no document state. New public API: these three in `pincel-image-wasm`, and `pincel-palette`'s `Rgba`, `PaletteEntry`, `MAX_ENTRIES` plus the moved codec functions and error types.
- 2026-11-11 (spec-build): The palette state lives in `ui/src/modes/image/stores/palette.svelte.ts` (`custom: null` = built-in default), its pure rules (stored form, AC-14 reading, Add color's duplicate-before-full check) in the rune-free `ui/src/modes/image/palette.ts`. The stored form is `{ rgba, name? }[]` with `rgba` packed `0xRRGGBBAA` as the Technical notes suggest; an entry is readable when `rgba` is an integer 0 … 0xFFFFFFFF and `name` is absent or a string.
- 2026-11-11 (spec-build): A named swatch's tooltip and accessible name are `{name} {hex}` (e.g. "Deep sea #0a141e"); a name equal to the swatch's own `#rrggbb` is not repeated, because a `.gpl` written by Pincel labels unnamed entries with their hex code (Pixel mode's export does the same) and reading it back yields that label as the name.
- 2026-11-11 (spec-build): Saving names the GIMP palette `palette` (the suggested file name's stem); Add color and Reset show no status message on success — the swatches change visibly and the ACs ask for none.
- 2026-11-11 (spec-build): The 8-row cap (AC-19) is measured, not fixed: the swatches stretch with the panel width, so `SwatchGrid` (new optional `maxRows` and `names` props) sets its `max-height` from the first swatch's height and the row gap through a `ResizeObserver`, and scrolls vertically past it.
- 2026-11-11 (spec-build): AC-13's desktop half (quit and restart) is not automated: the desktop app keeps IndexedDB in its WebView2 profile across restarts like every other pref (`recentColors:*`, `layout:*`), so the reload test covers the same storage path.
- 2026-11-11 (spec-build): Red runs: the five `pincel-image-wasm` palette tests failed against stub `read` / `write` bodies (4 of 5; `write_refuses_unknown_format_and_empty_strip` passed against the stub's error, it checks refusals only), and all 19 e2e tests of `image-palette.spec.ts` / `image-palette-state.spec.ts` failed against the UI without the palette menu (missing "Palette options" / "Add color", AC-14 showing the default instead of the stored entries).
