// Package selection rules of the install wizard (docs/specs/comfyui-install.md
// AC-03, AC-04, AC-11). Rune-free, so a Node-side spec can test it.

import type { Hardware, Package } from './install-bridge';

/** Packages that need `id` (the LoRA needs SDXL base). */
export function dependents(catalog: readonly Package[], id: string): Package[] {
  return catalog.filter((p) => p.requires === id);
}

function hasCheckpoint(catalog: readonly Package[], ids: readonly string[]): boolean {
  return catalog.some((p) => p.kind === 'checkpoint' && ids.includes(p.id));
}

/** `selected` without `id` and what needs it, in catalog order. */
function without(catalog: readonly Package[], selected: readonly string[], id: string): string[] {
  const gone = new Set([id, ...dependents(catalog, id).map((p) => p.id)]);
  return catalog.filter((p) => selected.includes(p.id) && !gone.has(p.id)).map((p) => p.id);
}

/** Whether `id` may be deselected: at least one checkpoint stays (AC-04). */
export function canDeselect(catalog: readonly Package[], selected: readonly string[], id: string): boolean {
  return hasCheckpoint(catalog, without(catalog, selected, id));
}

/** The selection after checking or unchecking `id`: checking adds what it
 *  needs, unchecking removes what needs it; a change that would leave no
 *  checkpoint is refused (AC-04). */
export function toggle(
  catalog: readonly Package[],
  selected: readonly string[],
  id: string,
  on: boolean,
): string[] {
  if (on) {
    const add = new Set([id]);
    const needed = catalog.find((p) => p.id === id)?.requires;
    if (needed) add.add(needed);
    return catalog.filter((p) => selected.includes(p.id) || add.has(p.id)).map((p) => p.id);
  }
  return canDeselect(catalog, selected, id) ? without(catalog, selected, id) : [...selected];
}

/** Bytes the install needs: the base install, the selected packages and the
 *  reserve (AC-03). */
export function requiredBytes(
  catalog: readonly Package[],
  selected: readonly string[],
  base: number,
  reserve: number,
): number {
  return catalog.filter((p) => selected.includes(p.id)).reduce((sum, p) => sum + p.size, base + reserve);
}

/** The package's minimum for this hardware (VRAM or Mac memory) and whether
 *  the hardware is below it (AC-04). */
export function minimum(pkg: Package, hardware: Hardware): { bytes: number; mac: boolean; below: boolean } {
  const mac = hardware.device?.backend === 'mps';
  const bytes = mac ? pkg.minMemory : pkg.minVram;
  const have = hardware.device?.memory ?? 0;
  return { bytes, mac, below: hardware.device !== null && have < bytes };
}

/** The first selection: SD 1.5, the smallest checkpoint, else the first one. */
export function defaultSelection(catalog: readonly Package[]): string[] {
  const first = catalog.find((p) => p.id === 'sd15') ?? catalog.find((p) => p.kind === 'checkpoint');
  return first ? [first.id] : [];
}
