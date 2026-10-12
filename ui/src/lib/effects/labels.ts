// Display labels for the effect catalog (spec §9.5). The catalog keeps its
// English labels as data; these helpers look them up in the UI language:
// effects by id (`effect.<id>`), groups by id (`effect.group.<id>`),
// parameter labels and choice options by their English text (`fx.<text>`).

import { isMessageKey, t } from '../i18n/index.svelte';
import type { EffectDef, EffectGroup } from './catalog';

function lookup(key: string, fallback: string): string {
  return isMessageKey(key) ? t(key) : fallback;
}

/** The effect's name in the UI language. */
export function effectLabel(def: EffectDef): string {
  return lookup(`effect.${def.id}`, def.label);
}

/** A menu group's name in the UI language. */
export function groupLabel(group: EffectGroup, fallback: string): string {
  return lookup(`effect.group.${group}`, fallback);
}

/** A parameter label or choice option in the UI language. */
export function fxText(text: string): string {
  const curve = /^Out @ (\d+)$/.exec(text);
  if (curve) return t('fx.outAt', { x: curve[1] ?? '' });
  return lookup(`fx.${text}`, text);
}
