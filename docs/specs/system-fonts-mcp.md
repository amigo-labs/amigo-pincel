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

- 2026-11-05: `pincel-agent` receives fonts from its host instead of enumerating them — because it is platform-free with no I/O (Decision Log 2026-10-29), and the headless server and the desktop app each own the enumeration.
- 2026-11-05 (owner's choice): The headless server offers the system's fonts by family name, enumerated with `fontdb` like the desktop app; workspace font files come later — because agents run on the user's machine and should reach the fonts the editor reaches, and a file source can be added later without breaking family names.
- 2026-11-05 (clarification pass): `font` matches the exact family name first, then case-insensitively — because family names are unique by exact name (`system-fonts` Terms), so a name copied from `font.list` always identifies one family, while case-insensitive matching only helps with hand-typed names and must therefore fail loudly when it is ambiguous (AC-13).
- 2026-11-05 (clarification pass): In live mode the webview reads and registers the face before forwarding `draw.text` — because the tools run in the webview's wasm, which cannot read files synchronously, and reusing the editor's face choice and read path keeps live results equal to the editor's (AC-11).
