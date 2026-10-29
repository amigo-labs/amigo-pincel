# Aseprite compatibility check

Manual check for AC-24 in `docs/specs/aseprite-round-trip.md`. A file Pincel saves must open in upstream Aseprite with everything intact.

The two `.aseprite` files next to this checklist are Pincel's writer output. `tests/aseprite_compat_fixture.rs` builds both sprites and fails if the writer no longer produces these exact bytes. After an intended writer change, regenerate them and repeat this check:

```bash
PINCEL_REGENERATE_FIXTURES=1 cargo test -p pincel-core --test aseprite_compat_fixture
```

Open each file in Aseprite 1.3 or later. The Console window must not appear for either file: no "tileset … not found", no "didn't found layer", no "Unexpected chunk type".

Record the result in the AC-24 row of the spec's `## Evidence` as `manual: {who} checked {what}, {date}`.

## `aseprite-compat-tilemap-min.aseprite`

The smallest file that shows whether Aseprite resolves a tilemap layer's tileset. Use it to bisect if the full file fails.

- [ ] Canvas 16 × 24 px, one frame of 100 ms.
- [ ] One layer `Tiles`, a tilemap layer using tileset `Terrain` with a tile size of 3 × 2 px.
- [ ] The tilemap shows the grid from [Tilemap](#tilemap) below.

## `aseprite-compat-check.aseprite`

### Sprite

- [ ] Canvas 32 × 24 px, color mode RGB Color.

### Layers

Top to bottom in the Layers panel:

| Layer | Kind | Visible | Blend mode | Opacity |
| ----- | ---- | ------- | ---------- | ------- |
| `Tiles` | tilemap | yes | Normal | 255 |
| `Hidden Notes` | image | **no** | Normal | 255 |
| `Characters` | group | yes | — | — |
| `Hero` (inside `Characters`) | image | yes | **Multiply** | **160** |
| `Background` | image | yes | Normal | 255 |

`Background` is an ordinary layer with that name, not Aseprite's special background layer.

### Frames and cels

- [ ] 4 frames: 100, 150, 200 and 250 ms.
- [ ] `Background`: frame 1 is a full light-blue (150, 200, 255) image. Frames 2–4 are linked cels to frame 1, and the timeline joins them.
- [ ] `Hero`: an 8 × 8 red (220, 40, 40) square at y = 8. Its x is 4, 8, 12 and 16 in frames 1 to 4. With Multiply at opacity 160 it looks dark red on the blue.
- [ ] `Hidden Notes`: a 4 × 4 yellow square at (0, 0), in frame 1 only. You only see it after turning the layer on.

### Tags

| Tag | Frames | Direction | Color | Repeat |
| --- | ------ | --------- | ----- | ------ |
| `idle` | 1–2 | Forward | red (200, 40, 40) | none |
| `bounce` | 2–4 | Ping-pong | blue (40, 90, 220) | 2 |

### Palette

| Index | Name | RGBA |
| ----- | ---- | ---- |
| 0 | `Sky` | 150, 200, 255, 255 |
| 1 | `Hero Red` | 220, 40, 40, 255 |
| 2 | `Grass` | 60, 170, 70, 255 |
| 3 | `Note Yellow` | 250, 220, 60, 128 |

### Slices

Check on frame 1. Center and pivot coordinates are relative to the slice.

| Slice | Bounds (x, y, w, h) | Color | 9-slice center | Pivot |
| ----- | ------------------- | ----- | -------------- | ----- |
| `hitbox` | 4, 8, 8, 8 | red | none | none |
| `panel` | 16, 2, 12, 10 | blue (0, 128, 255) | 3, 3, 6, 4 (canvas x 19–24, y 5–8) | 6, 5 (canvas 22, 7) |

### Tilemap

- [ ] Layer `Tiles` uses tileset `Terrain`, tile size 3 × 2 px, with 3 tiles: empty, green with a dark-green left column, and brown with a white top-left pixel.
- [ ] The tilemap covers canvas x 2–13, y 16–21, a grid 4 tiles wide and 3 high. Read the grid row by row, left to right:

| Row | Column 1 | Column 2 | Column 3 | Column 4 |
| --- | -------- | -------- | -------- | -------- |
| 1 | green | green, flipped X (dark column on the right) | brown | brown, flipped Y (white pixel bottom-left) |
| 2 | empty | green | brown, flipped X and Y (white pixel bottom-right) | green |
| 3 | brown | empty | green, flipped Y | brown |

The Y flip in row 3, column 3 does not change how the green tile looks, because its dark column runs the full height. The tile inspector shows the flag.

## Optional re-save check

- [ ] Use File > Save As in Aseprite to save a copy, then open that copy in Pincel. Layers, frames, tags, slices and the tilemap are still there.
