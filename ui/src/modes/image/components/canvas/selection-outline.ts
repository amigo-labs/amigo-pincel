/** One closed outline: its corner vertices in document space. */
export type Outline = Array<[number, number]>;

/**
 * Traces a selection mask's boundary as closed outlines in document space.
 * Each outline becomes one continuous subpath, so a dash runs along it and
 * the marching ants march; separate one-pixel edges would restart the dash
 * on every edge and, zoomed out, blink all at once.
 *
 * `mask` is `width × height` coverage bytes; a pixel counts as selected
 * from 128 up.
 */
export function selectionOutline(mask: ArrayLike<number>, width: number, height: number): Outline[] {
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
  const loops: Outline[] = [];
  const vx = (v: number) => v % stride;
  const vy = (v: number) => Math.floor(v / stride);
  for (const start of next.keys()) {
    // Walk the loop through `start`, consuming its edges; every vertex has
    // as many edges in as out, so the walk ends back at `start`. Only
    // corners become path vertices.
    let at = start;
    let dir = 0;
    const loop: Outline = [];
    for (let to = next.get(at)?.pop(); to !== undefined; to = next.get(at)?.pop()) {
      if (loop.length === 0 || to - at !== dir) loop.push([vx(at), vy(at)]);
      dir = to - at;
      at = to;
    }
    if (loop.length > 0) loops.push(loop);
  }
  return loops;
}

/**
 * The outlines as a device-space path for a view of `scale` device pixels
 * per document pixel and origin `(ox, oy)` in device pixels. Every vertex
 * is snapped to a device pixel centre, so a one-pixel line covers whole
 * pixels at any zoom and pan instead of smearing over two at half strength.
 */
export function devicePath(loops: readonly Outline[], scale: number, ox: number, oy: number): Path2D {
  const path = new Path2D();
  const snap = (v: number, o: number): number => Math.round(o + v * scale) + 0.5;
  for (const loop of loops) {
    loop.forEach(([x, y], i) => {
      if (i === 0) path.moveTo(snap(x, ox), snap(y, oy));
      else path.lineTo(snap(x, ox), snap(y, oy));
    });
    path.closePath();
  }
  return path;
}

/** The outlines mapped by `m` (canvas to canvas: a transform-box drag,
 *  image-transform-handles AC-23), then scaled by `scale` and offset by
 *  `(ox, oy)` into device space; not snapped, as the map turns and scales. */
export function mappedDevicePath(
  loops: readonly Outline[],
  m: readonly [number, number, number, number, number, number],
  scale: number,
  ox: number,
  oy: number,
): Path2D {
  const path = new Path2D();
  for (const loop of loops) {
    loop.forEach(([x, y], i) => {
      const px = ox + (m[0] * x + m[1] * y + m[2]) * scale;
      const py = oy + (m[3] * x + m[4] * y + m[5]) * scale;
      if (i === 0) path.moveTo(px, py);
      else path.lineTo(px, py);
    });
    path.closePath();
  }
  return path;
}
