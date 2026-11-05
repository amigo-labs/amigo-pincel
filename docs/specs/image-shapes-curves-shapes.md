---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §9.2, pincel.md §9.4, pincel.md §15 (2026-10-27 Image mode gains Gradient, Recolor and Smudge), pincel.md §15 (2026-11-05 MCP tool parity)]
depends_on: [image-live-objects, image-live-objects-ui, image-shapes-curves]
---

# Image mode: more shapes (triangle, diamond, star, block arrow, heart, callout)

## Context

Image mode's Shapes tool (`U`, pincel.md §5.4) offers Rectangle, Rounded Rectangle, Ellipse and a regular Polygon (3–100 sides, inscribed in the smaller side of the drag box), each as outline / fill / both with stroke width, dash pattern and anti-alias. paint.net ships dozens of shapes; the ones people reach for when annotating screenshots, building mockups and making simple game assets are arrows, stars, callouts, hearts and the box-filling triangle and diamond, none of which the regular polygon can produce.

This is part 2 of `image-shapes-curves` (part 1: Line / Curve, caps and dash patterns). The new shapes are box shapes of the live-object model (`image-live-objects`, `image-live-objects-ui`): the drag leaves the shape a live object that the core draws into its layer at every change; box handles and a body drag edit it, each edit is an ordinary undo step, the commit folds them into one, Escape cancels, Enter commits, and any other command or active-layer change commits it first. Per Decision Log 2026-11-05 "MCP tool parity", every new shape is reachable through `draw.shape`.

## Scope

### In scope

- Six new variants in the Shapes group, after Polygon: Triangle, Diamond, Star, Block Arrow, Heart, Callout
- Each with the existing outline / fill / both mode, stroke width, dash pattern, opacity and anti-alias, the selection bound and one undo step
- Star options: number of points (3–50, default 5) and inner radius (10–90 % of the outer radius, default 38 %)
- Icons, English and German names, `U` cycling, the rail flyout and the options bar's segmented control
- `draw.shape` kinds `triangle`, `diamond`, `star`, `block_arrow`, `heart`, `callout`

### Out of scope

- Hexagon, octagon and other regular n-gons as own variants — because Polygon with 6 or 8 sides already draws them
- Right triangle, cross, cloud, lightning, gear, check mark and paint.net's other catalog shapes — because they are rarer in annotation and asset work; the box-fitted family below takes them later without new options
- A movable callout tail, arrow head proportions or rotation handles — because they need extra handles that `image-live-objects` does not define; orientation comes from the drag direction instead
- Text inside a callout — because the Text tool places text on its own layer step; binding the two is a separate feature
- New shapes in Pixel mode — because Pixel mode's rectangle and ellipse are pixel-exact tools with their own rasterizers (`pixel-drawing-tools`)

## Acceptance criteria

- [ ] AC-01 (event): When the user presses `U` repeatedly, then the Shapes tool cycles Rectangle, Rounded Rectangle, Ellipse, Polygon, Triangle, Diamond, Star, Block Arrow, Heart, Callout and back to Rectangle; the rail flyout and the options bar list the same ten variants in that order, each with its own icon and its name in English and German.
- [ ] AC-02 (event): When the user drags a box with Triangle, then an isosceles triangle is drawn with its apex at the middle of the box's top edge and its base along the box's bottom edge.
- [ ] AC-03 (event): When the user drags a box with Diamond, then a rhombus is drawn whose four corners lie at the midpoints of the box's edges.
- [ ] AC-04 (event): When the user drags a box with Star, then a star with the set number of points is drawn, inscribed in the box as Polygon is: centered in the box, its outer radius half the box's shorter side, one point straight up and its inner corners at the set inner radius.
- [ ] AC-05 (event): When the user drags a box with Block Arrow, then an arrow fills the box, pointing from the box's start corner (where the drag began) toward its end corner along the dominant axis: right or left when the box is at least as wide as it is tall, down or up otherwise; proportions as in Technical notes.
- [ ] AC-06 (event): When the user drags a box with Heart, then a heart is drawn that touches all four edges of the box, with its two lobes at the top and its point at the middle of the bottom edge.
- [ ] AC-07 (event): When the user drags a box with Callout, then a speech bubble is drawn: a rounded body over the 80 % of the box's height away from the drag's start corner, and a tail whose tip lies on the start corner; the outline is one closed contour, with no stroke between body and tail.
- [ ] AC-08 (always): Every new shape is painted with the outline in the foreground color, the fill in the background color, both at the tool opacity, in the chosen mode, stroke width, dash pattern and anti-alias setting, exactly as the existing shapes do.
- [ ] AC-09 (always): With anti-alias on, every new shape's fill fully covers each pixel whose center lies more than 0.5 px inside its boundary and leaves unchanged each pixel whose center lies more than 0.5 px outside it (plus half the stroke width when outlined); with anti-alias off, every pixel is either unchanged or written at full coverage.
- [ ] AC-10 (always): Drawn in a box whose corners have integer coordinates, Triangle, Diamond, Star, Heart and an up- or down-pointing Block Arrow produce pixels mirror-symmetric about the box's vertical center line, and Diamond and a right- or left-pointing Block Arrow about its horizontal center line.
- [ ] AC-11 (event): When the user holds Shift while dragging a new shape, then its box becomes a square, as for the existing shapes.
- [ ] AC-12 (event): When the user drags a new shape, then while dragging the canvas shows that shape's own silhouette at the dragged box (in whatever form `image-live-objects-ui` previews the existing shapes' initial drag, never a rectangle stand-in), and the release leaves it a live object with its box handles; once committed, its steps are one undo step that Undo removes and Redo restores.
- [ ] AC-13 (state): While a selection exists, a new shape changes only selected pixels, scaled by the selection coverage.
- [ ] AC-14 (error): If the drag box of a new shape has zero width or zero height, then nothing is painted and no undo step is added.
- [ ] AC-15 (error): If the drag box is smaller than 3 px on a side, or a star has 50 points in a small box, then the shape is drawn (or nothing, per AC-14) without an error and without a stray pixel outside the box plus half the stroke width and 1 px.
- [ ] AC-16 (event): When an MCP client calls `draw.shape` with `triangle`, `diamond`, `block_arrow`, `heart` or `callout` and a box `(x0, y0)` → `(x1, y1)`, or with `star` and `cx`, `cy`, `radius`, `star_points`, `inner_ratio` and `rotation`, then the layer has exactly the pixels the editor's matching shape draws for the same geometry, stroke and fill, in one undo step; for `block_arrow` and `callout` the box's first corner is the drag start.
- [ ] AC-17 (error): If an MCP `draw.shape` call gives `star_points` outside 3–50, `inner_ratio` outside 0.1–0.9, a coordinate, radius or rotation that is not a finite number in single precision, or a kind not in the list, then it fails with `isError: true` naming the argument, and the document is unchanged. A zero-area box, or a star radius of 0 or less, succeeds with `changed: false` and leaves the document unchanged.
- [ ] AC-18 (always): `tools/list` lists all eleven `draw.shape` kinds and describes the star's parameters and the drag-start meaning of `block_arrow` and `callout`.
- [ ] AC-19 (event): When the user picks another box shape while a box shape is live (`U` cycling, the rail or the flyout), with any of the ten box shapes on either side, then the live object becomes the picked shape with the same box and style, as one undo step, and stays live.
- [ ] AC-20 (event): When the user changes Points or Inner radius while a Star is live, or drags a box handle of a live Block Arrow or Callout, then the shape is redrawn at once as one undo step and stays live, with AC-05 and AC-07 re-applied to the box as it now is: a handle dragged past the opposite edge carries the start corner with it, and the dominant axis is recomputed from the new width and height.

## Technical notes

**Geometry**, for a box of width W and height H spanned by the drag start S and end E:

| Shape | Construction |
| ----- | ------------ |
| Triangle | (left, bottom), (center, top), (right, bottom) |
| Diamond | Edge midpoints: top, right, bottom, left |
| Star | 2n vertices alternating outer radius R = min(W, H) / 2 and inner radius r = R × inner ratio, first outer vertex up, centered in the box (the same center / radius convention as Polygon; MCP `rotation` 0 also puts a point up) |
| Block Arrow | Along its axis of length A (W for horizontal, H for vertical) and thickness T (the other side): head length min(A / 2, T), head spanning the full thickness, shaft thickness T / 2 centered |
| Heart | Two mirrored cubic Bézier halves meeting at the top-center notch (30 % of H below the top edge) and the bottom-center point, the lobes touching the top, left and right edges |
| Callout | Body: rounded rectangle over the 80 % of H away from S, corner radius 15 % of the body's shorter side, fixed (the toolbar's Radius belongs to Rounded Rectangle). Tail: triangle with its tip on S and its base on the body edge facing S, from 15 % to 35 % of W measured from S's side; body and tail are united into one outline |

Straight-edged shapes use the existing polygon SDF (`sd_polygon`); Heart and Callout are flattened to a polygon whose distance to the exact boundary stays below 0.1 px, then use the same SDF, so fill, stroke, dash and anti-alias work as for the other shapes and the outline is one contour (AC-07). Vertex counts stay bounded (a star has at most 100 vertices, flattened curves at most a few hundred), so drawing costs no more than a 100-sided Polygon of the same box by more than a small factor.

**Core and wasm.** `draw_shape` gains the kinds `triangle`, `diamond`, `block_arrow`, `heart`, `callout` (taking `points[0..2]`, first point = drag start) and `star` (taking `center`, `radius`, `rotation` like `polygon`, plus serde-defaulted `star_points` and `inner_ratio`). The ts-rs mirror is regenerated. `shapes.rs` is over the 800-line limit, so the new shapes live in their own module.

**MCP.** The kind names are `triangle`, `diamond`, `star`, `block_arrow` (not `arrow`, to keep it apart from part 1's arrow caps), `heart`, `callout`; the star takes `cx`, `cy`, `radius`, `star_points` (default 5), `inner_ratio` (default 0.38) and `rotation` in degrees, mirroring `polygon`.

**Live object.** All ten box shapes share the live record's box and style; Star adds its points and inner ratio. The box keeps its first corner as the drag start, also when a handle drag flips it, which is what Block Arrow's direction and Callout's tail read (AC-20). `image-live-objects` AC-07 and `image-live-objects-ui` AC-08 list the four existing box shapes; this spec extends that conversion to all ten (AC-19).

**UI.** The options bar shows Points and Inner radius for Star only. The initial-drag preview uses the same geometry as the core, in the form `image-live-objects-ui` chooses for all shapes (AC-12).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Block Arrow dragged exactly diagonally (equal horizontal and vertical movement) | Horizontal wins: points right or left |
| Block Arrow in a box taller than wide pointing right | Head length is half the width; the arrow is stubby but fills the box |
| Callout dragged from the bottom-left corner to the top-right | Body at the top, tail pointing down-left to the start corner |
| Star with inner radius 90 % and 3 points | A slightly indented triangle, still one contour |
| Star with 50 points in a 6 px box | Drawn without error (AC-15); it reads as a disc |
| Heart with a very flat box (W ≫ H) | A wide heart touching all four edges (AC-06) |
| A new shape dragged partly off the canvas | Only the on-canvas part is painted |
| Dashed outline on a Callout | The dash runs around the one outline, across the body-tail junction |
| Live Star converted to Polygon and back | Box and style kept; the star's points and inner ratio come back from the tool options (AC-19) |
| Escape while a new shape is live | The shape is cancelled; the layer shows the pixels from before it (`image-live-objects-ui` AC-11) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Six new shapes, picked over paint.net's full catalog | They cover annotation (arrow, callout), mockups (triangle, diamond) and simple assets (star, heart); every other common shape is either a Polygon or a later box-fitted addition | Low |
| Orientation of Block Arrow and Callout comes from the drag direction | Gives four orientations without a new option or handle; diagonal arrows are part 1's line with an Arrow cap | Low |
| Triangle and Diamond fill the box instead of being regular | That is what distinguishes them from Polygon with 3 / 4 sides | Low |
| Default inner radius 38 % | Gives the classic five-point star (pentagram proportions) | Low |
| `image-live-objects` and `image-live-objects-ui` land first | Their box handles, body drag, commit and cancel apply unchanged to the new shapes | Low |
| One Shapes group with ten variants | The flyout and the options bar reach every variant directly; `U` cycling is the keyboard fallback | Medium: ten presses for the last variant |

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

## Decisions

- 2026-11-05, owner's choice: the new shapes follow the shared live-object architecture — the core draws the live object into its layer at every change, its record sits beside the document on `CommandBus` (not in `.ora`), each edit is an ordinary undo step, the commit folds them into one, Escape cancels, Enter commits, and any other command or active-layer change commits it first; on area shapes a body drag moves the shape — because one model for every live object keeps undo, autosave and MCP behavior identical across kinds.
- 2026-11-05: All six new shapes are box shapes with box handles; Star, like Polygon, is inscribed in its box (center and radius derived from it) rather than stretched — because a drag box is how every area shape is drawn today and what `image-live-objects`' box handles edit, and a stretched star stops reading as a star.
- 2026-11-05: Block Arrow and Callout read their orientation from the box's current start and end corners, recomputed after every handle drag (AC-20) — because the box is the object's only geometry; keeping a stale direction would make a flipped arrow point backwards.
- 2026-11-05: Callout's corner radius is fixed at 15 % — because the proportion works at every size and keeps the options bar to the Star's two extra controls.
- 2026-11-05: A zero-area box or non-positive star radius over MCP reports `changed: false` instead of failing (AC-17) — because `draw.shape` already reports a draw that changes nothing as `changed: false`, and a degenerate box is not a malformed call.
- 2026-11-05: Part 2 depends on `image-shapes-curves` — because AC-08 includes its Dash-Dot patterns.
