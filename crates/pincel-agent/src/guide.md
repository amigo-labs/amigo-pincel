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

Shared tools work on both: `doc.*`, `layer.*`, `draw.fill`, `effect.apply`,
`adjust.apply` and `export.png`. Pixel-only: `frame.*`, `tag.*`,
`palette.*`, `draw.pixels`, `draw.line`, `draw.rect`, `draw.ellipse`,
`draw.clear`, `selection.*`, `export.sheet`. Image-only: `draw.stroke`,
`draw.shape`, `draw.text`, `draw.gradient`, `select.*`, `canvas.resize`,
`image.scale`, `image.crop_to_selection`, `image.flip`, `image.rotate`,
`export.jpeg`, `export.webp`. A tool of the other mode fails and says so.

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
  Image mode `draw.gradient`); clear it with `selection.clear` (Pixel) or
  `select.clear` (Image) when you are done.
- `image.crop_to_selection` needs a selection.
- An Image document opened from PNG, JPEG or another flat format has no
  `.ora` file: `doc.save` needs a `.ora` path, or use `export.png`.
- Effects and adjustments take named parameters within the ranges listed in
  `pincel://effects`; omitted ones use the editor's defaults.
- Pixel art: keep to the palette, avoid anti-aliased tools, and check the
  preview at a large scale with `grid: true`.
