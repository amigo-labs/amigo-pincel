// The layer tree inside Image mode's flat, bottom-first layer list
// (docs/specs/image-layer-groups.md): a group's descendants follow it as one
// contiguous run, each naming its group in `parent`.
import type { LayerInfo } from './wasm';

/** Exclusive end of the subtree rooted at `layers[i]` (`i + 1` for a raster layer). */
export function subtreeEnd(layers: readonly LayerInfo[], i: number): number {
  const depth = layers[i]?.depth ?? 0;
  let j = i + 1;
  while (j < layers.length && (layers[j]?.depth ?? 0) > depth) j++;
  return j;
}

/** Indices in panel order: top-most first, each group directly above its children. */
export function treeOrder(layers: readonly LayerInfo[]): number[] {
  const children = new Map<number | null, number[]>();
  layers.forEach((l, i) => {
    const list = children.get(l.parent ?? null) ?? [];
    list.push(i);
    children.set(l.parent ?? null, list);
  });
  const order: number[] = [];
  const walk = (parent: number | null): void => {
    for (const i of [...(children.get(parent) ?? [])].reverse()) {
      order.push(i);
      walk(i);
    }
  };
  walk(null);
  return order;
}

/** Whether Merge Down applies to `layers[i]`: it and the layer below are
 *  raster layers in the same group (AC-13). */
export function canMergeDown(layers: readonly LayerInfo[], i: number): boolean {
  const layer = layers[i];
  const below = layers[i - 1];
  return (
    layer !== undefined &&
    below !== undefined &&
    layer.kind === 'raster' &&
    below.kind === 'raster' &&
    layer.parent === below.parent
  );
}

/** Whether deleting `layers[i]` (a group with its subtree) leaves a raster layer (AC-20). */
export function canDeleteLayer(layers: readonly LayerInfo[], i: number): boolean {
  if (!layers[i]) return false;
  const raster = (l: LayerInfo): boolean => l.kind === 'raster';
  const removed = layers.slice(i, subtreeEnd(layers, i)).filter(raster).length;
  return removed < layers.filter(raster).length;
}

/** Whether a pixel edit on `layers[i]` is refused with the locked-layer notice:
 *  the layer or a group it sits in is locked, or it is a group. With
 *  `movesContent` (Move layer, flips, rotations) a group is allowed unless it
 *  or a layer inside it is locked (AC-11, AC-12, AC-18). */
export function editRefused(
  layers: readonly LayerInfo[],
  i: number,
  movesContent: boolean,
): boolean {
  const layer = layers[i];
  if (!layer) return false;
  const locked = (l: LayerInfo): boolean => l.locked || l.locked_by_parent;
  if (layer.kind !== 'group') return locked(layer);
  if (!movesContent) return true;
  return layers.slice(i, subtreeEnd(layers, i)).some(locked);
}
