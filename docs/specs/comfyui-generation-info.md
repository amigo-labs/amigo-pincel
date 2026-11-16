---
status: todo
created: 2026-11-16
size: L
mode: both
design: [pincel.md §15 (2026-11-06 ComfyUI), pincel.md §15 (2026-11-16 Generation info)]
depends_on: [comfyui-generate, comfyui-generate-pixel]
---

# Generation info on generated layers

## Context

A generated layer is named "Generated: " plus the first 30 characters of its prompt (`comfyui-generate` AC-12; Pixel mode uses the same name). The full prompt, the negative prompt, the model, the LoRA, the strength and the seed are gone once the variant is applied — whether the user pressed Apply or an agent called `gen.image` from the Assistant chat. The user cannot see what produced a layer, cannot copy it, and cannot generate again from it.

This spec keeps those values with the layer, shows them for the active layer in the layers panel both editors share, keeps them in `.ora` and `.aseprite` files, and lets the user copy them or load them into the Generate panel. The owner chose the layer over the Assistant chat row and the Generate panel as the place to show them (2026-11-16), because the layer is where a generated result lives on — also after the chat is gone or the file is reopened.

The file-format change has its Decision Log entry (`pincel.md` §15, 2026-11-16 "Generation info").

## Terms

| Term | Meaning |
| ---- | ------- |
| Generation info | What a layer records about the generation that produced it: prompt, negative prompt (empty when none was used), model, LoRA (or none) and LoRA strength, strength, and the seed of that variant |
| Model, LoRA | The checkpoint and LoRA by the name `gen.models` lists and `gen.image` accepts (`comfyui-generate` AC-18) |
| Strength | How much of the input is regenerated, in percent as the panel's slider shows it (1–100 %; `gen.image`'s 0.01–1.0 times 100) |
| LoRA strength | The LoRA's weight, 0–2 (`comfyui-generate` AC-03) |
| Text form | The generation info as text: the seven lines of Technical notes "Text form", in that order |
| Generated layer | A layer whose generation info is recorded |

## Scope

### In scope

- Recording the generation info when a variant becomes a layer, in Pixel and Image mode, by Apply or by `gen.image`
- A "Generation" section in the layers panel's properties block for the active layer, and the prompt in the layer row's tooltip
- Copy (all values as text) and "Use in Generate panel" (load the values into the panel's fields)
- Keeping the info through undo / redo, autosave recovery, `.ora` (Image) and `.aseprite` (Pixel)

### Out of scope

- Showing or editing the info in the Assistant chat or a dialog — because the owner chose the layers panel (Context)
- Editing the recorded values — because they describe how the layer was made; a changed value would describe a generation that never happened
- Agent tools reading the info (`layer.list`, `layer.info`) — because the agent already gets prompt, seed and model in the `gen.image` result; a later spec if agents need it for older layers
- Keeping the info in flat formats (PNG, JPEG, WebP, …) and PSD — because those keep no Pincel layers (PSD is import only)
- Recording the variant count, generation size, sampler, workflow, target box, input, mask or the model's "Model type" choice — because "Use in Generate panel" restores the panel's settings, and the panel derives all of these from the model, the document and the selection when Generate is pressed (`comfyui-generate` Technical notes); a byte-exact replay is not the goal
- Showing or editing a Pixel layer's user data in Pincel otherwise — because Pincel has no user-data editor; Aseprite stays the place for it

## Acceptance criteria

- [ ] AC-01 (event): When a generated variant becomes a new layer — by Apply in the Generate panel of either editor, or by a `gen.image` call — then that layer records its generation info: the prompt and negative prompt the variant was generated with, the model, the LoRA (or none) with its strength, the strength, and the seed of that variant.
- [ ] AC-02 (state): While the active layer is a generated layer, the editor's layers panel shows a "Generation" section in the active layer's properties with every value of its generation info (the negative prompt as "—" when empty, the LoRA as "—" without a LoRA strength when none, the strength in percent); while the active layer is not a generated layer, the section is absent. The section follows the active layer as it changes (another layer, another tab, the layer deleted).
- [ ] AC-03 (always): The tooltip of a generated layer's row in the layers panel contains the layer name and the full prompt.
- [ ] AC-04 (event): When the user clicks Copy in the Generation section, then the clipboard holds the generation info in the text form, with the same English labels in every UI language; if the clipboard refuses the write, the section shows "Could not copy" and nothing else changes.
- [ ] AC-05 (event): When the user clicks "Use in Generate panel" in the Generation section, then the Generate panel is expanded (as if the user expanded it) with prompt, negative prompt, model, LoRA, LoRA strength, strength and seed set to the recorded values; the variant count is unchanged, because it is the user's batch choice and not part of one variant; a model or LoRA the server no longer has falls back with the notice of `comfyui-generate` AC-03.
- [ ] AC-06 (state): While the editor has no Generate panel (the browser app), "Use in Generate panel" is absent and the rest of the section shows.
- [ ] AC-07 (event): When the user undoes the step that added a generated layer and redoes it, then the layer is back with the same generation info.
- [ ] AC-08 (always): Painting on a generated layer, renaming it, moving it and changing its opacity, blend mode, visibility or lock keep its generation info; Duplicate layer gives the copy the same generation info; Merge down keeps the generation info of the lower layer only, and Flatten produces a layer without one, because a flattened layer is a composite of every layer, not one variant; undoing and redoing any of these steps restores each layer's generation info as it was.
- [ ] AC-09 (event): When an Image document with generated layers is saved as OpenRaster and opened again, then every generated layer has its generation info again with every value equal, and every other layer attribute the file kept before this feature carries the same value as before.
- [ ] AC-10 (event): When a Pixel document with generated layers is saved as `.aseprite` and opened again, then every generated layer has its generation info again with every value equal, and the layer's user data in the file reads the text form, because that is what Aseprite shows in the layer's properties.
- [ ] AC-11 (event): When an editor recovers a document from its autosave snapshot, then its generated layers have their generation info again.
- [ ] AC-12 (error): If a layer's stored generation info is incomplete or invalid — a value missing, an empty prompt or model, a seed that is not a whole number from 0 to 18 446 744 073 709 551 615 (ComfyUI's seed range), a strength outside 1–100 % or a LoRA strength outside 0–2, and for a Pixel layer also a user-data text that is not exactly the text form (a line missing, extra, out of order or with another label) — then the file opens, that layer is not a generated layer, and saving keeps the Pixel layer's user-data text unchanged.
- [ ] AC-13 (always): Every user-visible string of the Generation section, its buttons and its tooltip has an entry in every UI language's dictionary, and the German UI shows them in German; the text form keeps English labels (AC-04).

## Technical notes

- **Text form** (AC-04, AC-10). Seven lines in this order, English labels, each `Label: value` split at the first `: `: `Prompt: …`, `Negative prompt: …` (nothing after `: ` when none), `Model: …`, `LoRA: …` (nothing after `: ` when none, like the negative prompt, so no LoRA name can be mistaken for "none"), `LoRA strength: 1` (`1`, the panel's default, when no LoRA was used), `Strength: 100%`, `Seed: 1234`. Inside a value a backslash is written `\\` and a line break `\n`, so every value stays on one line and reads back exactly. Numbers in shortest decimal form with `.` (`0.75`, `55.5%`); lines end with LF, CRLF reads the same. Pixel mode reads a layer's user data back by this form; Image mode stores the same values per layer in `stack.xml` (Decision Log 2026-11-16 "Generation info").
- **Where the info travels.** Image mode keeps it on the layer in the document model, so the autosave snapshot, undo and Duplicate carry it like any layer field. Pixel mode keeps it in the layer's user-data text, which `.aseprite` and the Pixel snapshot already carry.
- **UI.** The section sits in the shared layers panel's properties block below opacity and blend mode; both editors hand it the active layer's info through their layer rows. "Use in Generate panel" goes through each editor's Generate panel state.
- **Tests.** Rust round-trip tests for `.ora` and `.aseprite` (AC-09, AC-10, AC-12) and undo / merge / duplicate in both cores (AC-07, AC-08); Playwright against the mocked Tauri bridge and the mock ComfyUI flow of `ui/e2e/comfyui-generate*.spec.ts` for AC-01 – AC-06, AC-11, AC-13.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Variant from a later round of a reviewed generation (`generation-self-improvement`) | Records the prompt and negative prompt of its own round (AC-01) |
| Flux model, negative prompt typed but ignored | Records the negative prompt as empty, because Flux generated without it (AC-01) |
| Seed field empty (random) | Records the seed the variant was actually generated with (AC-01) |
| Third of four variants applied | Records that variant's own seed (consecutive seeds, `comfyui-generate` Technical notes) (AC-01) |
| Prompt of 2000 characters with line breaks | Shown in full in the section, wrapping, and contained in full in the tooltip; its line breaks escaped in the text form (Technical notes) (AC-02 – AC-04) |
| Image layer with a prompt of 100 000 characters | Recorded and kept in full; `.ora` and the snapshot have no length limit (AC-01, AC-09, AC-11) |
| "Use in Generate panel" before the server's model list has loaded | The fields take the recorded values; the model and LoRA are checked when the list arrives, as for the stored panel model (`comfyui-generate` AC-03) |
| "Could not copy" shown | It stays until the next click in the section or until the section shows another layer (AC-04) |
| Generated layer's user data edited in Aseprite to other text | No generation info on reopening; the text stays as written (AC-12) |
| `.ora` from another program | Opens as before; no layer is a generated layer (AC-09, AC-12) |
| Generated layer copied and pasted as a new layer (Copy / Paste) | The pasted layer has no generation info, because Paste makes a new layer from pixels (AC-08) |
| "Use in Generate panel" while the panel holds unapplied variants | The fields change; the variants stay until Discard or Generate (`comfyui-generate` AC-13) |
| Sprite sheet export with JSON data of a Pixel document | A generated layer's `data` field holds the text form, as for any layer user data |
| Prompt containing `Seed: 5`, a colon or a line break | Reads back unchanged: each line's label is fixed by its position and line breaks are escaped (AC-10, AC-12) |
| `gen.image` with strength 0.555 | Records 55.5 %; "Use in Generate panel" sets the slider to the nearest whole percent, halves rounded up, 56 %, because the slider moves in whole percent (AC-01, AC-05) |
| Pixel layer whose text form exceeds 65 535 bytes (the `.aseprite` string limit) | The layer is added without generation info, because the file could not keep it (AC-01, AC-10) |
| "Use in Generate panel" while a generation runs | The fields change; the running generation keeps its settings (`comfyui-generate` AC-08) |
| "Use in Generate panel" with a Flux model | The negative prompt field stays disabled, as for any Flux model (`comfyui-generate` AC-03) |
| Generated layer active while the server is unreachable | The section shows; "Use in Generate panel" fills the fields with the recorded values without a fallback, because there is no model list to check against; Generate shows the server status as usual (AC-05) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A recorded prompt is never empty | The panel's Generate needs a prompt (`comfyui-generate` AC-04) and `gen.image` requires one (AC-16), so an empty prompt in a file marks it as not written by Pincel (AC-12) | none |
| The section sits in the active layer's properties block, and the prompt in the row tooltip | That block already shows the active layer's properties in both editors; the tooltip shows the prompt of any layer on hover, not only the active one | The block grows for generated layers |
| English labels in the copied text and the user data | The text is data: it must read the same on every machine and parse back from `.aseprite` whatever language saved it | German users see English labels when pasting |
| Pixel mode stores the info in the layer's user-data text | `.aseprite` already keeps it, and Aseprite shows it in the layer properties without a Pincel-only chunk | A user who writes their own text there loses the info (AC-12) |
| Merge down keeps the lower layer's info | Merge down paints the upper layer into the lower one, which stays the same layer with its name and properties, so its record stays like its other properties; Flatten replaces every layer with a new one | A generated layer merged onto a plain one loses its info; a generated lower layer keeps a record its pixels no longer match exactly |
| Duplicate copies the info | The copy shows the same generated pixels | Edits to the copy keep a record that no longer matches it exactly (as for the original) |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/generation-info.spec.ts` "AC-01 – AC-05 AC-07 Apply records the variant; …"; `ui/e2e/generation-info.spec.ts` "AC-01 AC-02 AC-13 gen.image records its generation on a Pixel layer, shown in German"; `generated_paste_records_its_generation_and_redo_keeps_it` (crates/pincel-image-core/src/command/paste.rs); `a_generated_layer_records_its_generation_info_for_the_layers_panel` (crates/pincel-image-wasm/src/generate.rs, crates/pincel-wasm/src/generate.rs); `with_generation_skips_a_text_beyond_the_aseprite_limit` (crates/pincel-core/src/document/generation.rs) | built  |        |
| AC-02 | `ui/e2e/generation-info.spec.ts` "AC-01 – AC-05 AC-07 Apply records the variant; …"; `ui/e2e/generation-info.spec.ts` "AC-01 AC-02 AC-13 gen.image records its generation on a Pixel layer, shown in German"; `ui/e2e/generation-info.spec.ts` "AC-06 the browser app shows the section without …" | built  |        |
| AC-03 | `ui/e2e/generation-info.spec.ts` "AC-01 – AC-05 AC-07 Apply records the variant; …" | built  |        |
| AC-04 | `ui/e2e/generation-info.spec.ts` "AC-01 – AC-05 AC-07 Apply records the variant; …"; `to_text_writes_seven_lines_with_escapes` (crates/pincel-image-core/src/document/generation.rs); `to_text_writes_seven_escaped_lines` (crates/pincel-core/src/document/generation.rs) | built  |        |
| AC-05 | `ui/e2e/generation-info.spec.ts` "AC-01 – AC-05 AC-07 Apply records the variant; …"; `ui/e2e/generation-info.spec.ts` "AC-05 a recorded model the server no longer has falls back …" | built  |        |
| AC-06 | `ui/e2e/generation-info.spec.ts` "AC-06 the browser app shows the section without …" | built  |        |
| AC-07 | `ui/e2e/generation-info.spec.ts` "AC-01 – AC-05 AC-07 Apply records the variant; …"; `generated_paste_records_its_generation_and_redo_keeps_it` (crates/pincel-image-core/src/command/paste.rs); `generated_layer_keeps_its_generation_info_through_undo_redo_and_aseprite` (crates/pincel-core/tests/generation_info.rs) | built  |        |
| AC-08 | `layer_edits_keep_the_generation_info` (crates/pincel-image-core/src/command/properties.rs, crates/pincel-core/tests/generation_info.rs); `duplicate_layer_copies_the_generation_info_and_redo_keeps_it` (crates/pincel-image-core/src/command/layers.rs); `merge_down_keeps_only_the_lower_generation_info_and_undo_restores_both`, `flatten_drops_the_generation_info_and_undo_restores_it` (crates/pincel-image-core/src/command/merge.rs); `duplicate_copies_merge_down_keeps_the_lower_and_flatten_drops_generation_info` (crates/pincel-core/tests/generation_info.rs) | built  |        |
| AC-09 | `to_ora_bytes_then_from_ora_bytes_keeps_the_generation_info` (crates/pincel-image-core/src/codec/ora.rs); `ui/e2e/generation-info.spec.ts` "AC-06 the browser app shows the section without …" (a stored `.ora` opens with its info) | built  |        |
| AC-10 | `generated_layer_keeps_its_generation_info_through_undo_redo_and_aseprite` (crates/pincel-core/tests/generation_info.rs); `from_text_reads_the_text_form_back_also_with_crlf` (crates/pincel-core/src/document/generation.rs) | built  |        |
| AC-11 | `snapshot_round_trip_restores_the_document` (crates/pincel-image-core/src/codec/snapshot.rs, sample with generation info) and `to_ora_bytes_then_from_ora_bytes_keeps_the_generation_info` (Image autosave: snapshot → ORA); `generated_layer_keeps_its_generation_info_through_undo_redo_and_aseprite` (crates/pincel-core/tests/generation_info.rs; Pixel autosave: `.aseprite`) | built  |        |
| AC-12 | `from_ora_bytes_ignores_incomplete_or_invalid_generation_info` (crates/pincel-image-core/src/codec/ora.rs); `from_text_refuses_anything_but_the_exact_valid_form` (crates/pincel-core/src/document/generation.rs); `foreign_user_data_text_is_no_generation_info_and_survives_saving` (crates/pincel-core/tests/generation_info.rs); `a_generation_with_an_invalid_value_is_not_recorded` (crates/pincel-wasm/src/generate.rs) | built  |        |
| AC-13 | `ui/e2e/generation-info.spec.ts` "AC-01 AC-02 AC-13 gen.image records its generation on a Pixel layer, shown in German"; key parity of all six dictionaries in `pnpm check` (`Record<MessageKey, string>`) | built  |        |

## Decisions

- 2026-11-16: The info lives on the layer and shows in the layers panel's properties block, with Copy and "Use in Generate panel"; both modes; kept in `.ora` and `.aseprite` (owner). A separate spec rather than revisions of `comfyui-generate` and `comfyui-generate-pixel`, because it spans both modes, the shared layers panel and two file formats.
- 2026-11-16 (spec-build): Image mode keeps the info as an optional `generation` field on `pincel-image-core`'s `Layer` (serde default, so older snapshots read as before) and writes seven `pincel:*` attributes in the namespace `urn:amigo-labs:pincel:ora` on `<layer>`; line breaks and tabs inside attribute values are written as character references, because an XML reader turns a raw one into a space. Pixel mode keeps the text form as the layer's user-data text (`Layer::with_generation` / `Layer::generation` in `pincel-core`).
- 2026-11-16 (spec-build): The text form also escapes a carriage return as `\r`, so a CRLF inside a prompt reads back exactly like a line feed does.
- 2026-11-16 (spec-build): `pincel-wasm` takes and returns the info as an exported `Generation` class (seed as a decimal string) instead of JSON, because `serde_json` is only a dev-dependency there and a new runtime dependency needs approval (AGENTS.md §9); `pincel-image-wasm`, which already depends on `serde_json`, takes JSON like its other calls and reports the info in `LayerInfo.generation`. `add_generated_layer` in `pincel-image-wasm` takes the target box as one `[x, y, w, h]` slice, which keeps it within clippy's argument limit without a suppression.
- 2026-11-16 (spec-build): The Generate store keeps the values of a generation from Generate on (the effective negative prompt — empty for Flux — and the effective strength, 100 % when there is nothing to refine) and the first variant's seed from the backend; Apply records seed + index of the applied variant. "Use in Generate panel" sets the fields and expands the panel through a new `expandPanel(mode, id)` request that `EditorFrame` acts on (sidebar, drawer or bottom sheet); expanding reads the server's models, so a missing model falls back with the existing notice also when the list was not loaded yet.
- 2026-11-16 (spec-build): The layers panel's properties block may shrink and scrolls when the panel is short, and the layer list keeps at least two rows: with the Generation section the block outgrew short panels and its buttons lay under the next panel's header. LoRA strength is shown only with a LoRA; strength uses the app's percent format with one decimal.
- 2026-11-16 (spec-build): Red runs: `ui/e2e/generation-info.spec.ts` against the build before the UI change, 4 of 4 failed at the missing Generation section (Apply, `gen.image` and opening the `.ora` passed before it); the Rust blocks went red on the missing API (`with_generation`, `GENERATION_NS`, the new `add_generated_layer` arguments, `layer_generation`). Written with their module and green at once: the `pincel-core` text-form unit tests, and the layer-edit, duplicate, merge-down and flatten tests of both cores (those commands carry the field or the user data already).
- 2026-11-16 (spec-build): AC-11 is proven through what autosave stores: Image mode hands the document to the encoder as a snapshot and stores ORA, Pixel mode stores `.aseprite`; both round trips keep the info. No e2e recovery run, because the recovery specs drive the browser app, which cannot generate.
