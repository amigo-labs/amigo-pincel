// The order the layers panel lists Pixel-mode layers in: top-most first at
// every level, each group directly above its (indented) children. The panel
// and the Alt+↑ / Alt+↓ keys (power-user-shortcuts-coverage AC-05) share it.

import type { Document } from '../../lib/core';

/** Layer ids in panel order. */
export function layerTreeOrder(doc: Document): number[] {
  // Children of each parent (`-1` = the root), bottom first.
  const children: Record<number, number[]> = {};
  for (let i = 0; i < doc.layerCount; i += 1) {
    let id: number;
    try {
      id = doc.layerIdAt(i);
    } catch {
      continue;
    }
    const parent = doc.layerParent(id) ?? -1;
    (children[parent] ??= []).push(id);
  }
  const order: number[] = [];
  const walk = (parent: number, guard: number) => {
    if (guard > doc.layerCount) return;
    for (const id of [...(children[parent] ?? [])].reverse()) {
      order.push(id);
      walk(id, guard + 1);
    }
  };
  walk(-1, 0);
  return order;
}
