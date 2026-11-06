// The Clone Stamp's source point and offset (docs/specs/clone-stamp.md).
// They are a tool setting of each document tab (AC-16): not part of the
// document, its undo history or its file, and dropped with the tab.
import { tabs } from '../stores/editor.svelte';

/** A tab's source point and, once the first stroke fixed it, the offset. */
export interface CloneSource {
  /** The source pixel (whole canvas pixels). */
  readonly point: readonly [number, number];
  /** Source minus the pixel under the first stroke's press; `null` until
   *  that stroke starts (aligned cloning, AC-05, AC-07). */
  readonly offset: readonly [number, number] | null;
}

/** Sources by tab id; tab ids are unique per page load. */
const sources: Record<number, CloneSource> = $state({});

/** The pointer over the canvas (or while painting), in canvas pixels, for
 *  the source crosshair (AC-15); `null` when it is elsewhere. */
export const cloneHover = $state({ point: null as [number, number] | null });

/** The pixel under a fractional canvas position. */
export function pixelUnder(p: readonly [number, number]): [number, number] {
  return [Math.floor(p[0]), Math.floor(p[1])];
}

/** The active tab's source, `null` when none is set. */
export function activeCloneSource(): CloneSource | null {
  const tab = tabs.active;
  return tab === null ? null : (sources[tab.id] ?? null);
}

/** Sets the active tab's source point to the pixel under `p` (AC-03); the
 *  next stroke fixes a new offset (AC-07). */
export function setCloneSource(p: readonly [number, number]): void {
  const tab = tabs.active;
  if (tab === null) return;
  sources[tab.id] = { point: pixelUnder(p), offset: null };
}

/** The active tab's offset for a stroke pressed at `p`: fixed now if this
 *  is the first stroke since the source was set. `null` without a source. */
export function strokeOffset(p: readonly [number, number]): readonly [number, number] | null {
  const tab = tabs.active;
  const source = tab === null ? undefined : sources[tab.id];
  if (tab === null || source === undefined) return null;
  if (source.offset !== null) return source.offset;
  const [x, y] = pixelUnder(p);
  const offset: [number, number] = [source.point[0] - x, source.point[1] - y];
  sources[tab.id] = { point: source.point, offset };
  return offset;
}

/** Forgets the source of a closed tab. */
export function dropCloneSource(tabId: number): void {
  delete sources[tabId];
}
