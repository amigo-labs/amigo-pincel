---
status: todo
created: 2026-10-28
size: M
mode: Image
design: [pincel.md §4.2, pincel.md §15 (2026-10-11 OpenRaster project format)]
depends_on: [image-layer-groups]
---

# Layer groups in OpenRaster files

## Context

Opening an `.ora` with nested `<stack>`s today flattens them and **drops the group's own opacity, visibility and blend mode**: a hidden or 50 % group's children then show fully visible. That is silent data loss. Once Image mode has layer groups (spec `image-layer-groups`), `.ora` read and write must carry them: nested stacks open as groups with their attributes, and grouped documents save as nested stacks, so a layered file round-trips through Pincel, GIMP, Krita and MyPaint.

Already decided: `.ora` is the Image-mode project format, and `stack.xml` is read and written by a small in-crate attribute reader — **no XML crate** (Decision Log 2026-10-11).

## Scope

### In scope

- Reading nested `<stack>` elements as groups with their name, opacity, visibility and `composite-op`
- Writing groups as nested `<stack>` elements with `isolation="isolate"`
- Write → read round-trip of the layer tree
- Flat `.ora` files keep opening as before

### Out of scope

- Pass-through stacks (`isolation="auto"` rendered as pass-through) — because Image mode composites groups isolated only (spec `image-layer-groups`)
- Group masks and clipping — because Image mode has no masks yet
- Krita `.kra` files — because they are a separate container format (see `psd-import` out of scope)

## Acceptance criteria

- [ ] AC-01 (event): When an `.ora` whose `stack.xml` contains nested `<stack>` elements is opened, then each nested stack becomes a group with the stack's name, opacity, visibility and blend mode, and each layer sits inside the group of the stack that contains it, in the file's order.
- [ ] AC-02 (state): While a group opened from a nested stack is hidden or below 100 % opacity, its children are hidden or composited at that opacity as a whole, exactly as for a group created in Pincel — never fully visible.
- [ ] AC-03 (event): When a document with groups is saved as `.ora`, then each group is written as a nested `<stack>` carrying its name, opacity, visibility, blend mode and `isolation="isolate"`, with its children inside it in document order.
- [ ] AC-04 (always): Writing a document with nested groups to `.ora` and reading it back yields the same layer tree: names, order, parents, visibility, opacity, blend modes and pixels.
- [ ] AC-05 (always): An `.ora` with a single flat `<stack>` opens with the same layers, offsets and properties as before this feature.
- [ ] AC-06 (event): When the user saves a document with a group as `.ora` in Image mode and reopens the file, then the layers panel still shows the group with its child indented below it.
- [ ] AC-07 (always): The `mergedimage.png` written into the `.ora` equals the document composite, with groups applied in isolation.

## Technical notes

Hints from the legacy plan (2026-10-28); paths verified 2026-10-29, line numbers drift.

- `crates/pincel-image-core/src/codec/ora.rs` writes one flat `<stack>` (≈L115). It reads every `<layer>` start tag in document order with the `tags()` scanner (≈L248); nested stack attributes are discarded. Keep the attribute scanner and track `<stack` / `</stack>` depth.
- Existing test `from_ora_bytes_places_offset_layers_and_flattens_nested_stacks` (≈L384) becomes `from_ora_bytes_keeps_nested_stack_attributes`; add a write → read → equal-tree round-trip test (AC-04).
- Layer attributes already mapped: opacity, visibility, `svg:*` composite ops, Krita's `edit-locked`, `selected` (Decision Log 2026-10-11).
- e2e pattern for save and reopen: `ui/e2e/ora.spec.ts` (download + file chooser); layer rows via `getByRole('button', { name: /^Hide / })` and the row's `data-depth`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Nested stack without any layers | Opens as an empty group |
| Stack with an unknown `composite-op` | Same fallback as an unknown `composite-op` on a layer today |
| Stack with `isolation="auto"` or no `isolation` attribute (Krita / GIMP default) | Opens as an isolated group, because pass-through is out of scope |
| Stacks nested several levels deep | Every level becomes a group with the correct parent |
| More nested stacks plus layers than the 999-layer limit | Open fails with the existing layer-limit error; no crash, the previous document stays open |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Stacks without `isolation="isolate"` are read as isolated groups | Pass-through groups are out of scope in `image-layer-groups` | Medium — a file from Krita / GIMP with pass-through groups and non-Normal child blend modes renders differently than in those apps |

## Open questions

- Should the group's lock be written and read (Krita's `edit-locked` on `<stack>`, as layers do today)? The plan's attribute list for `<stack>` is `name opacity visibility composite-op isolation` only, so a locked group would lose its lock on save.
- What should opening do when `stack.xml` has unbalanced `<stack>` / `</stack>` tags — refuse the file with an error, or recover best-effort by closing open stacks at the end?

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

## Decisions

- No XML crate: the existing attribute scanner gains `<stack` / `</stack>` depth tracking — because Decision Log 2026-10-11 rules out an XML parser dependency for `stack.xml`.
- Groups are written with `isolation="isolate"` — because Image-mode groups composite isolated (spec `image-layer-groups`), and the attribute tells other ORA readers to do the same.
