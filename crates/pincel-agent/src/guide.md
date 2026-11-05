# Pincel agent guide

Pincel is a pixel-art and image editor. You edit documents in memory with
the tools below and write files only with `doc.save` and `export.*`. Every
path is relative to the workspace the server was started in.

## Modes

Each document has a mode, and the mode follows the file, as in the editor:

| Mode | For | Created by | Saved as |
| ---- | --- | ---------- | -------- |
| Pixel | Sprites, tiles, animation with frames, tags and a palette (up to 4096 px per side) | `doc.new` with `mode: "pixel"`, `doc.open` of `.aseprite` | `.aseprite` (`doc.save`), PNG (`export.png`), sprite sheets (`export.sheet`) |
| Image | Photos, screenshots, mockups, artwork with brushes, shapes, text and selections (up to 16384 px per side, 64 Mpx) | `doc.new` with `mode: "image"`, `doc.open` of `.ora`, PNG, JPEG, WebP, BMP, GIF or TIFF | OpenRaster `.ora` (`doc.save`, keeps layers), `export.png`, `export.jpeg`, `export.webp` |

Shared tools work on both: `doc.*`, `layer.*` (except the two below),
`select.*` (except `select.feather`), `draw.stroke`, `draw.shape`,
`draw.text`, `draw.gradient`, `draw.fill`, `effect.apply`,
`adjust.apply`, `canvas.resize`, `image.scale`, `image.crop_to_selection`,
`image.flip`, `image.rotate` and `export.png`. Pixel-only: `frame.*`,
`tag.*`, `palette.*`, `draw.pixels`, `draw.line`, `draw.rect`,
`draw.ellipse`, `draw.clear`, `export.sheet`. Image-only:
`layer.merge_visible`, `layer.merge_group`, `select.feather`,
`draw.recolor`, `draw.smudge`, `export.jpeg`, `export.webp`. A tool of the other mode fails and says so,
and so does an argument only the other mode's editor offers.

## Drawing in each mode

- `draw.stroke`: Pixel documents (the Pencil) take integer points, `size`
  1–8 (default 1), `shape` `square` (default) or `round`, `pixel_perfect`,
  `ink: "shading"` with `ramp_from` / `ramp_to` (default the whole
  palette) and `step` 1 / -1, `mirror` `x` / `y` / `xy` with `axis_x2` /
  `axis_y2` in half pixels (default the centre) and `wrap` `x` / `y` /
  `xy` (tiled mode). Image documents (the Brush) take `size` 1–500
  (default 8), `hardness`, `opacity` and `shape` `hard_round` (default),
  `soft_round` or `flat`. Both: `tool: "eraser"`, color default `#000000`.
- `draw.shape`: Pixel documents draw `rect`, `rounded` (`corner_radius`
  0–256, default 4), `ellipse` and `polygon` (`sides` 3–64, default 6)
  over integer corners with `stroke_width` 1–64 (default 1); lines are
  `draw.line`. Image documents add `line`, polygons by `cx` / `cy` /
  `radius` / `rotation`, `dash`, `anti_alias` (default true) and
  `stroke_width` 1–500 (default 3).
- `draw.text`: Pixel `size` 1–2000 (default 12), `anti_alias` default
  false; Image `size` 1–999 (default 48), `anti_alias` default true.
- `draw.gradient`: Pixel documents (RGBA only) take `kind` `linear` /
  `radial` and `dither` 0, 2, 4 (default) or 8; Image documents take five
  kinds, `channel` and `opacity`.
- The selection: in Pixel documents `draw.gradient`, `draw.text` and
  `draw.fill` stay inside it, while `draw.stroke`, `draw.shape`,
  `draw.pixels`, `draw.line`, `draw.rect`, `draw.ellipse` and `draw.clear`
  ignore it, as in the Pixel editor. Pixel `draw.fill` clips to the
  selection although the Pixel editor's Bucket does not. In Image
  documents every drawing tool stays inside the selection.
- Image documents only: `draw.recolor` (the Recolor tool: `from`, the
  replacement `color`, `tolerance` default 32) and `draw.smudge`
  (`strength` 1–100, default 60) take the brush arguments of
  `draw.stroke`; the Image eraser takes `eraser_mode` `to_transparent`
  (default) or `to_background` with `background` (default `#FFFFFF`);
  Image strokes take the flat-nib `angle` 0–180 (default 45); Image
  `draw.fill` takes `sample` `current_layer` (default) or `all_layers`.
- Effect defaults follow the document's mode, as each editor's effect
  dialog does: `pincel://effects` lists `defaults` per mode where they
  differ (e.g. `pixelate` and `drop_shadow`).

## Selections and transforms

- Selecting, both modes: `select.rect`, `select.ellipse`,
  `select.polygon`, `select.wand` (each with `mode` `replace`, `add`,
  `subtract` or `intersect`), `select.all`, `select.clear`,
  `select.invert`, `select.expand`, `select.contract`. Each returns the
  selection's bounds `{x, y, w, h}` (or `null`) and the selected-pixel
  count. `select.get` reads the coverage of up to 64×64 pixels.
- Image documents only: `feather` on `select.rect`, `select.ellipse` and
  `select.polygon`, and `select.feather`. Pixel documents refuse them.
- Undo: in Image documents a selection change is an undo step; in Pixel
  documents selections are not undo steps, as in each editor.
- Defaults follow each editor: `select.wand` `tolerance` 0 in Pixel
  documents and 32 in Image documents; `canvas.resize` `anchor` `center`
  (Pixel) or `top_left` (Image); `image.scale` `interpolation` `nearest`
  (Pixel) or `bilinear` (Image). Pixel `select.invert` without a selection
  selects everything; Image selects nothing.
- `select.delete` is the editor's Delete: it clears the selected pixels of
  the target layer. Without a selection, Image documents clear the whole
  layer and Pixel documents change nothing.
- Transforms, both modes: `canvas.resize`, `image.scale`,
  `image.crop_to_selection`, `image.flip`, `image.rotate` (the whole
  image), `layer.flip` and `layer.rotate` (one layer). With a selection,
  Pixel `layer.flip` / `layer.rotate` turn only the selection's bounds;
  Image documents turn the whole layer (a group with its subtree)
  whatever is selected. Pixel canvas transforms drop the selection; undo
  brings it back.

## Slices, tilesets and the grid

These tools work on Pixel documents only: `slice.list`, `slice.add`,
`slice.remove`, `slice.set_key`, `tileset.list`, `tileset.add`,
`tile.add`, `tile.read_pixels`, `tile.set_pixels`, `tilemap.get`,
`tilemap.place` and `grid.set`.

- A slice key's 9-patch `center` and its `pivot` are relative to the
  key's bounds, as in the Aseprite format.
- Every tileset starts with tile 0, the empty tile. `tilemap.place` with
  `tile: 0` clears a cell; painting tile 0 with `tile.set_pixels` shows in
  every empty cell.
- `layer.add` with `kind: "tilemap"` and a `tileset` adds a tilemap layer.
  `draw.*` and `effect.apply` cannot paint on tilemap layers: edit the
  tiles with `tile.set_pixels` and the cells with `tilemap.place`.
- `grid.set` changes the document grid stored in the `.aseprite` file.

## Moving content and the clipboard

- `select.move_content` (both modes) moves the selected pixels with the
  selection, one undo step; Pixel documents need a selection, Image
  documents move the whole layer without one.
- Image documents only: `layer.translate` (the Move tool on a whole layer
  or group) and `select.move` (the selection outline alone).
- There is no system clipboard; a workspace PNG takes its place.
  `export.selection` is the headless Copy (`merged: true`: Copy Merged;
  `cut: true`: Cut): it writes the selected pixels of a layer as a PNG and
  reports the area `{x, y, w, h}`. `layer.import` is the headless Paste:
  it puts an image file on a new layer above the active one at `x`, `y`
  (`expand: true` grows the canvas). Both modes; Pixel documents paste
  PNG only.

## Document structure

- Layers, both modes: `layer.add` (`kind: "group"` for a group;
  `target` + `place` `above` / `below` / `into` for an exact spot),
  `layer.duplicate`, `layer.move`, `layer.merge_down`, `layer.flatten`,
  `layer.set`, `layer.remove`, `layer.list`. Without `target`, `layer.add`
  puts the layer where the editor would: on top in Pixel documents, directly
  above the active layer in Image documents. Pixel groups cannot be
  duplicated.
- Layers, Image documents only: `layer.merge_visible`, `layer.merge_group`.
  Groups have no pixels: drawing tools need an image layer inside them.
- Tags, Pixel documents only: `tag.add` (optional `color`), `tag.set`
  (name, `from` + `to`, direction), `tag.remove`, `tag.list`.
- Palette, Pixel documents only: `palette.get` (colors and names),
  `palette.set`, `palette.load_preset`, `palette.set_entry`,
  `palette.add_entry`, `palette.remove_entry`, `palette.move_entry`,
  `palette.import` (`.gpl`, `.hex`, `.pal` or a PNG palette image) and
  `palette.export` (`format` `gpl`, `hex`, `pal` or `png`). The `gpl`,
  `hex` and `pal` formats store no alpha: `palette.export` drops it.
  Removing, moving and importing entries works in `rgba` documents only
  (`doc.info` reports `color_mode`).
- `doc.info` also reports the `selection` bounds `{x, y, w, h}` or `null`.

## Workflow

The loop is plan → draw → preview → export, in both modes.

1. **Plan** — decide the canvas size, the layers and (Pixel) the palette
   before drawing. Small Pixel sprites are 16×16 to 64×64.
2. **Create or open** — `doc.new` or `doc.open`; the new document becomes
   the active one, and tools without `doc` act on it.
3. **Draw** — add layers with `layer.add` and draw on the active layer (or
   pass `layer`). Pixel: `draw.pixels`, `draw.line`, `draw.rect`,
   `draw.ellipse`, `draw.fill`. Image: `draw.stroke`, `draw.shape`,
   `draw.text`, `draw.fill`, `draw.gradient`, then `adjust.apply` /
   `effect.apply`.
4. **Preview** — look at the result with `doc.preview` after every few
   edits and fix what looks wrong; check exact colors with
   `doc.read_pixels`.
5. **Export** — `doc.save` keeps the editable document; `export.png` (and
   `export.sheet`, `export.jpeg`, `export.webp`) write the flat result.

## Coordinates

- Document pixels, origin (0, 0) at the top-left, x to the right, y down.
- Pixel tools take integer pixels; rectangles and ellipses are given by two
  inclusive corners (x0, y0) and (x1, y1).
- Image tools take numbers; (x + 0.5, y + 0.5) is the center of pixel
  (x, y), which matters for thin strokes and lines.
- `draw.text` places (x, y) at the top of the first line; `align` decides
  whether x is its left edge, center or right edge.

## Colors

- Colors are `"#RRGGBB"` or `"#RRGGBBAA"` (alpha 00 = transparent).
- Pixel documents also take `{ "index": n }`, an entry of the palette
  (`palette.get`); `pincel://palettes/{id}` lists the preset palettes.
- Image opacities and hardness are percentages (0–100); layer opacity is
  0–255 in both modes.

## Undo

- Every successful call that changes a document is exactly one undo step.
  `doc.undo` reverts the last call as a whole, `doc.redo` re-applies it;
  both report the remaining depths.
- A call that fails changes nothing.
- `doc.info` reports `dirty` (unsaved changes); `doc.close` discards them.

## Preview

- `doc.preview` returns a PNG image you can look at, scaled up with
  nearest-neighbour so Pixel art stays crisp; without `scale` the longest
  side becomes at least 256 px.
- The preview is capped at 2048 px per side and never downscales: for a
  large Image document, preview a `region`.
- `grid: true` (scale 4 and up) outlines every document pixel.
- `doc.read_pixels` gives exact `#RRGGBBAA` values for up to 64×64 pixels,
  of the composed image or of one `layer`.

## Common pitfalls

- Drawing on the wrong layer: tools act on the active layer; `layer.add`
  makes the new layer active, and `layer.list` shows the ids to pass as
  `layer`.
- Locked layers refuse edits; unlock with `layer.set` and `locked: false`.
- A selection limits `draw.fill`, `adjust.apply`, `effect.apply` (and in
  Image mode `draw.gradient`); clear it with `select.clear` when you are
  done.
- `image.crop_to_selection` needs a selection.
- An Image document opened from PNG, JPEG or another flat format has no
  `.ora` file: `doc.save` needs a `.ora` path, or use `export.png`.
- Effects and adjustments take named parameters within the ranges listed in
  `pincel://effects`; omitted ones use the editor's defaults.
- Pixel art: keep to the palette, avoid anti-aliased tools, and check the
  preview at a large scale with `grid: true`.
