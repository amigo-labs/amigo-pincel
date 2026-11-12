---
status: todo
created: 2026-11-05
size: M
mode: both
design: [mcp.md, pincel.md §15 (2026-09-06 ab_glyph for the Text tool), pincel.md §15 (2026-10-29 agent tools in pincel-agent), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [system-fonts, mcp-headless-image, mcp-parity-drawing]
---

# System fonts for MCP `draw.text`

## Context

`system-fonts` lets the editor's Text tools use the fonts installed on the system. The MCP tool `draw.text` writes with the bundled Liberation Sans only: `pincel-agent` embeds the font file and always passes font id 0 (`crates/pincel-agent/src/image/draw.rs`). `mcp-parity-drawing` makes `draw.text` work on Pixel documents too (its AC-14) and still leaves fonts other than the bundled one out, because no editor offered a font choice when it was written. Decision Log 2026-11-05 "MCP tool parity" asks that every document-changing capability of the editor have an MCP tool; with system fonts, choosing a font becomes part of the Text capability.

This spec extends `draw.text` as `mcp-headless-image` and `mcp-parity-drawing` define it, for Image and Pixel documents, with a `font` argument; it lifts `mcp-parity-drawing`'s out-of-scope entry "Fonts other than the bundled Liberation Sans". It adds `font.list` so agents learn which fonts exist, and makes the hosts of the agent tools supply the system's fonts: the headless `pincel-mcp` server and, in live mode (`mcp-live-desktop`), the desktop app. `pincel-agent` stays platform-free and does no I/O (AGENTS.md §5, Decision Log 2026-10-29).

**"The editor"** means, as in the MCP parity specs, the result of the core command the UI issues for the same action. Terms (*face*, *family*, *Regular / Bold / Italic / Bold Italic face*, *bundled*, *reading a face*) are those of `system-fonts`, including its face-choice rules and its rule that upright regular "Liberation Sans" is always the bundled face (`system-fonts` AC-23).

## Scope

### In scope

- `font` argument on `draw.text` for Image and Pixel documents: a family name; Bold / Italic pick real faces as in the editor
- `font.list`: the families available to the host, with their style names
- System fonts in the headless `pincel-mcp` server (`fontdb`) and in live desktop mode
- Errors for unknown, ambiguous or unusable fonts
- `pincel://guide` documents `font` and `font.list`

### Out of scope

- Font files passed by path (workspace font files) or as base64 — because the owner chose system fonts by family name first (2026-11-05); workspace font files come in a later spec
- The PWA's added fonts in MCP — because MCP runs only in the headless server and the desktop app (live mode is desktop-only, `mcp-live-desktop`), and neither has "Add font file…"
- Glyph fallback, shaping, variable-font axes — because `system-fonts` leaves them out for the editor too, and the tool reaches only what the editor does
- A "current font" kept by the server between calls — because MCP tools are stateless per call apart from documents; an agent passes `font` with each call

## Acceptance criteria

- [ ] AC-01 (always): `tools/list` contains `font.list`, meeting `mcp-headless-pixel` AC-02, with an optional string argument `query`; the `draw.text` schema has an optional string argument `font`.
- [ ] AC-02 (event): When the client calls `font.list`, then the result is `{ "families": [ { "name": <family>, "styles": [<style name>, …] }, … ] }` with every family available to the host exactly once, sorted case-insensitively by name, each with the style names of its faces as the host reports them, sorted by weight then upright before slanted; "Liberation Sans" is always listed.
- [ ] AC-03 (option): Where `font.list` gets a non-empty `query`, only families whose name contains it (case-insensitive) are listed; an empty `query` lists all families.
- [ ] AC-04 (event): When `draw.text` is called without `font`, or with `font` "Liberation Sans" and neither Bold nor Italic, then the result equals the result of the same call before this spec: the editor's text with the bundled face.
- [ ] AC-05 (event): When `draw.text` is called with `font` naming a listed family, then the document's pixels equal the editor's after placing the same text with that family, size, color, alignment, anti-alias, Bold and Italic in a document of the same mode, including the face choice and synthesis of `system-fonts` AC-07.
- [ ] AC-06 (event): When `font` matches no family exactly but exactly one family case-insensitively, then that family is used.
- [ ] AC-13 (error): If `font` matches no family exactly and several families case-insensitively, then the call fails with `isError: true` and the message `font "<value>" is ambiguous: <name>, <name>; pass the exact name`, listing every matching name, and the document is unchanged.
- [ ] AC-07 (error): If `font` matches no family, or is an empty string, then the call fails with `isError: true`, the message names the value and points to `font.list`, and the document is unchanged.
- [ ] AC-08 (error): If the chosen face cannot be read or parsed, then the call fails with `isError: true` naming the family and the reason, and the document is unchanged.
- [ ] AC-09 (state): While the headless `pincel-mcp` server runs, `font.list` reports the bundled family plus every family with a TrueType / OpenType face that `fontdb` finds on the system, found without any user interaction.
- [ ] AC-10 (always): `font.list` reads no face; a face is read only by a `draw.text` call that uses it, and at most once per host run.
- [ ] AC-11 (state): While the desktop app hosts the agent tools in live mode, `font.list` reports the same families as the app's own font list, and `draw.text` with `font` produces the pixels the editor produces with that family.
- [ ] AC-12 (always): `pincel://guide` describes `font.list`, the `font` argument of `draw.text` and the matching rules of AC-06, AC-13 and AC-07.

## Technical notes

- `pincel-agent` gets fonts from its host through a small provider: list families with their faces (family, style name, weight, slant, stretch, an opaque face id) and return one face's bytes plus face index by id. The default provider holds the bundled face alone, so tests and hosts without enumeration keep today's behavior. It replaces the fixed `FONT` constant / `font_id: 0` in `crates/pincel-agent/src/image/` and serves Pixel `draw.text` as added by `mcp-parity-drawing`. The face choice of `system-fonts` (Terms) is one shared function, so the editor and the tools cannot choose differently.
- `pincel-mcp` implements the provider with `fontdb`, the enumerator `system-fonts` uses for the desktop app (owner decision 2026-11-05). Enumeration reads font metadata, which AC-10 does not count as reading a face.
- **Live mode:** the tools run in the webview's wasm through `agentCall` (`mcp-live-desktop`), where no file can be read synchronously. The webview hands the agent its font list (the same data its font list shows) and, before forwarding a `draw.text` call with `font`, resolves the face with the editor's face choice, reads it through the desktop shell (async) and registers it with `registerFont(bytes, faceIndex?)`; the provider inside wasm then returns the registered face. A failed read turns into AC-08's error.
- Tests use a fake provider with fixture fonts (Liberation Sans, Liberation Serif Regular / Bold / Italic) for AC-02 – AC-08, AC-10 and AC-13, counting reads for AC-10; AC-09 is a `pincel-mcp` test that checks the bundled family plus the families `fontdb` reports on the CI host; AC-11 runs against the mocked Tauri bridge.
- `font.list` results can be long (thousands of families); they carry names and style names only, no previews.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `font: "arial"` with only "Arial" installed | Case-insensitive match (AC-06) |
| "Arial" and "ARIAL" both installed, `font: "arial"` | Ambiguous: fails listing both (AC-13); `font: "Arial"` matches exactly |
| Server on a machine without system fonts (container) | `font.list` lists Liberation Sans only; `draw.text` without `font` works (AC-04) |
| Font uninstalled while the server runs | Reading the face fails; AC-08 |
| `font` given as an empty string | Fails per AC-07 naming the empty value, not treated as a missing `font` — because an empty name is a client error |
| Bold requested, family has no Bold face | Synthesized bold, as in the editor (`system-fonts` AC-07) |
| `font: "Liberation Sans"` with Bold on, Liberation Sans Bold installed | The installed Bold face is used, as in the editor (`system-fonts` AC-23) |
| `draw.text` on a Pixel document with `font` | Same face choice as on Image documents; pixels equal the Pixel editor's (AC-05) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| An unknown font is an error for agents, not a silent fallback as in the editor | An agent can correct the call from the message and `font.list`; a silent substitution would make its result differ from what it asked for without it noticing | Agents must handle one more error |
| Fonts are addressed by family name, not by file or PostScript name | It is what the editor's user picks, and names are portable across machines that share fonts | Families differing only in case need the exact name (AC-13) |
| `font.list` is a tool, not a resource | Agents read the tool list on connect, and the result depends on the machine and changes when fonts are installed | none known |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `list_tools_has_font_list_and_the_font_argument` (`crates/pincel-agent/src/tests/fonts.rs`), `tool_schemas_are_valid` (`crates/pincel-agent/src/tests/doc.rs`) | built | |
| AC-02 | `font_list_lists_each_family_once_sorted_with_its_styles`, `font_list_without_host_fonts_lists_liberation_sans` (`crates/pincel-agent/src/tests/fonts.rs`) | built | |
| AC-03 | `font_list_query_filters_by_name_ignoring_case` (`crates/pincel-agent/src/tests/fonts.rs`) | built | |
| AC-04 | `draw_text_without_font_or_with_regular_liberation_sans_uses_the_bundled_face` (`crates/pincel-agent/src/tests/fonts.rs`) | built | |
| AC-05 | `draw_text_with_a_family_matches_the_editor_on_image_documents`, `draw_text_with_a_family_matches_the_editor_on_pixel_documents`, `face_choice_cases_match_the_shared_table` (`crates/pincel-agent/src/tests/fonts.rs`); `the editor chooses the faces of the shared face-choice table` (`ui/e2e/system-fonts-mcp.spec.ts`) | built | |
| AC-06 | `draw_text_font_matches_one_family_ignoring_case` (`crates/pincel-agent/src/tests/fonts.rs`) | built | |
| AC-07 | `draw_text_unknown_or_empty_font_fails_and_points_to_font_list` (`crates/pincel-agent/src/tests/fonts.rs`) | built | |
| AC-08 | `draw_text_unreadable_face_fails_naming_family_and_reason` (`crates/pincel-agent/src/tests/fonts.rs`) | built | |
| AC-09 | `font_list_reports_the_bundled_and_the_system_families` (`crates/pincel-mcp/tests/stdio.rs`); `faces_lists_each_face_with_family_style_and_attributes`, `read_returns_the_file_and_face_index_of_a_listed_face`, `read_of_a_removed_file_fails_with_the_reason` (`crates/pincel-mcp/src/fonts.rs`) | built | |
| AC-10 | `font_list_reads_no_face_and_draw_text_reads_each_face_once` (`crates/pincel-agent/src/tests/fonts.rs`) | built | |
| AC-11 | `call_image_wire_draws_text_with_the_face_the_editor_read`, `call_pixel_wire_draws_text_with_the_face_the_editor_read`, `call_wire_fonts_report_a_face_the_editor_could_not_read`, `call_wire_fonts_feed_font_list_and_matching` (`crates/pincel-agent/src/tests/live.rs`); `agent_call_draws_text_with_a_face_the_editor_registered` (`crates/pincel-wasm/src/agent.rs`, `crates/pincel-image-wasm/src/agent.rs`); "AC-11 font.list reports the families of the app font list, reading no face", "AC-11 draw.text with font on an Image document draws what the editor draws", "AC-11 draw.text with font on a Pixel document draws what the editor draws", "AC-11 draw.text with a face the shell cannot read fails and changes nothing" (`ui/e2e/system-fonts-mcp-live.spec.ts`) | built | |
| AC-12 | `guide_describes_font_list_and_the_font_matching_rules` (`crates/pincel-agent/src/resources.rs`) | built | |
| AC-13 | `draw_text_ambiguous_font_fails_listing_every_match` (`crates/pincel-agent/src/tests/fonts.rs`) | built | |

## Decisions

- 2026-11-05: `pincel-agent` receives fonts from its host instead of enumerating them — because it is platform-free with no I/O (Decision Log 2026-10-29), and the headless server and the desktop app each own the enumeration.
- 2026-11-05 (owner's choice): The headless server offers the system's fonts by family name, enumerated with `fontdb` like the desktop app; workspace font files come later — because agents run on the user's machine and should reach the fonts the editor reaches, and a file source can be added later without breaking family names.
- 2026-11-05 (clarification pass): `font` matches the exact family name first, then case-insensitively — because family names are unique by exact name (`system-fonts` Terms), so a name copied from `font.list` always identifies one family, while case-insensitive matching only helps with hand-typed names and must therefore fail loudly when it is ambiguous (AC-13).
- 2026-11-05 (clarification pass): In live mode the webview reads and registers the face before forwarding `draw.text` — because the tools run in the webview's wasm, which cannot read files synchronously, and reusing the editor's face choice and read path keeps live results equal to the editor's (AC-11).
- 2026-11-11 (spec-build): The provider is the public trait `pincel_agent::FontProvider` (`faces()` → `FontFaceInfo` with id, family, style, weight, italic, stretch; `read(id)` → file bytes and face index), installed with `Workspace::with_fonts`; `Workspace::new` keeps the bundled face alone. The workspace caches each read face, failed reads included, so a host run reads a face at most once (AC-10) whatever the host does — because the workspace lives exactly as long as the host's session.
- 2026-11-11 (spec-build): The face choice of `system-fonts` exists twice, in the editor's `ui/src/lib/fonts/faces.ts` and in `crates/pincel-agent/src/fonts.rs`, and one case table, `crates/pincel-agent/src/face-choice-cases.json`, is checked against both (`face_choice_cases_match_the_shared_table`, `ui/e2e/system-fonts-mcp.spec.ts`) — because the headless server has no webview to run the TypeScript rules, and moving the editor's synchronous face choice into a wasm module would make the shared font list depend on one mode's core; the table keeps the two from choosing differently.
- 2026-11-11 (spec-build): A read face counts as usable only when the Text tool's own parser accepts it (`Text::layout_lines` of `pincel-image-core`), else AC-08 — because the Image core silently draws nothing with an unparseable face, which would turn a broken font into an unchanged document without an error.
- 2026-11-11 (spec-build): `font.list` takes the optional `doc` every tool takes and ignores it, as `doc.list` does — because `mcp-headless-pixel` makes `doc` part of every tool's schema (`tool_schemas_are_valid`).
- 2026-11-11 (spec-build): Without `font`, Bold / Italic stay synthesized on the bundled face even where Liberation Sans Bold is installed; with `font: "Liberation Sans"` the installed faces serve Bold / Italic — because AC-04 keeps calls without `font` exactly as before this spec, while the edge-case table asks for the editor's choice once the family is named.
- 2026-11-11 (spec-build): `pincel-mcp` depends on `fontdb` 0.23, the version `src-tauri` uses (Decision Log 2026-11-05 names both hosts), enumerates on the first `font.list` or `draw.text` with `font` and keeps the catalog for the run; it repeats the desktop list's 30-line `style_name` instead of sharing it — because `pincel-mcp` and `pincel-tauri` share no crate that could hold it without a new dependency edge, and both must build style names the same way.
- 2026-11-11 (spec-build): `ui/e2e/system-fonts-mcp.spec.ts` was green on its first run: it checks the editor's existing face choice (built by `system-fonts`) against the new table; its red counterpart is the Rust table test, which failed until `pincel-agent` had the rules.
- 2026-11-11 (spec-build): The tests use the `Pincel Test Serif` fixtures of `system-fonts` (`crates/pincel-core/tests/fixtures`: Regular, Italic, Bold Italic files and a collection whose Bold is face 1) in place of Liberation Serif — because they are already in the repository, cover a collection face, and are told apart from the bundled Liberation Sans.
- 2026-11-13 (spec-build): In live mode the webview hands the tools its fonts with each `draw.text` that has `font`: `agentCall` / `agent_call` take an optional `fonts_json` (`pincel_agent::live::WireFonts`) holding the font list's faces (the bundled one aside, ids by list position) and the face the editor's choice picked, read with `readFace` and registered with the mode's core (`registerFont`) — or the read's error, which becomes AC-08's reason. `pincel_agent::live::LiveFonts` is the `FontProvider` over them, and `call_pixel` / `call_image` (and their `_wire` forms) take it; a face the webview did not read cannot be used. Every other call passes no fonts, so calls without `font` stay as before (AC-04) — because the wasm tools cannot read a file synchronously, and resolving the face with the editor's own `familyFaces` / `faceFor` keeps live results equal to the editor's (Decision 2026-11-05).
- 2026-11-13 (spec-build): The webview resolves `font` to a family with `matchFamily` (`ui/src/lib/fonts/faces.ts`), a copy of `match_family` in `pincel-agent`, only to know which face to read; the tools still match and report AC-06, AC-07 and AC-13 themselves, and a family the copy did not match reads nothing — because a wrong guess can then only fail a call, never draw with another face.
- 2026-11-13 (spec-build): In live mode `font.list` is answered by the webview from the app's font list (`familyList()`, the list the Text tools show) without an open document, like `doc.list`, not by the wasm tools — because it needs no document while every wasm tool call runs on one (`mcp-live-desktop` AC-09 holds for document tools only); its style order repeats `style_names` of `pincel-agent` (`ui/src/lib/agent/fonts.ts`).
- 2026-11-13 (spec-build): Red runs for AC-11: `ui/e2e/system-fonts-mcp-live.spec.ts` failed in all four tests ("no document is open" for `font.list`, `font "pincel test serif" matches no family` for `draw.text`), and the Rust tests failed to compile until `live::WireFonts` and the `fonts` parameters existed. The mocked shell's font commands moved from `system-fonts-desktop.spec.ts` into `ui/e2e/font-helpers.ts` (`installShellFonts`, `shellLog`) so both specs use them.
