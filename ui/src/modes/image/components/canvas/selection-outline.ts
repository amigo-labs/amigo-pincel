/**
 * Traces a selection mask's boundary as closed outlines in document space.
 * Each outline is one continuous subpath, so a dash runs along it and the
 * marching ants march; separate one-pixel edges would restart the dash on
 * every edge and, zoomed out, blink all at once.
 *
 * `mask` is `width × height` coverage bytes; a pixel counts as selected
 * from 128 up.
 */
export function selectionOutline(mask: ArrayLike<number>, width: number, height: number): Path2D {
  const sel = (x: number, y: number): boolean =>
    x >= 0 && y >= 0 && x < width && y < height && (mask[y * width + x] ?? 0) >= 128;
  // Directed boundary edges with the selection on the right: vertex → next
  // vertices (two where selected pixels touch only at a corner). Vertices
  // are keyed `y * stride + x`.
  const stride = width + 1;
  const next = new Map<number, number[]>();
  const edge = (x0: number, y0: number, x1: number, y1: number): void => {
    const from = y0 * stride + x0;
    const to = y1 * stride + x1;
    const list = next.get(from);
    if (list) list.push(to);
    else next.set(from, [to]);
  };
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (!sel(x, y)) continue;
      if (!sel(x, y - 1)) edge(x, y, x + 1, y);
      if (!sel(x + 1, y)) edge(x + 1, y, x + 1, y + 1);
      if (!sel(x, y + 1)) edge(x + 1, y + 1, x, y + 1);
      if (!sel(x - 1, y)) edge(x, y + 1, x, y);
    }
  }
  const path = new Path2D();
  const vx = (v: number) => v % stride;
  const vy = (v: number) => Math.floor(v / stride);
  for (const start of next.keys()) {
    // Walk the loop through `start`, consuming its edges; every vertex has
    // as many edges in as out, so the walk ends back at `start`. Only
    // corners become path vertices.
    let at = start;
    let dir = 0;
    let open = false;
    for (let to = next.get(at)?.pop(); to !== undefined; to = next.get(at)?.pop()) {
      if (!open) {
        path.moveTo(vx(at), vy(at));
        open = true;
      } else if (to - at !== dir) {
        path.lineTo(vx(at), vy(at));
      }
      dir = to - at;
      at = to;
    }
    if (open) {
      path.lineTo(vx(at), vy(at));
      path.closePath();
    }
  }
  return path;
}
