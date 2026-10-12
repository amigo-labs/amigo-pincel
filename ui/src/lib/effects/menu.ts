// The Effects and Adjustments menus both modes show (spec §9.2, §15
// 2026-10-11 "One effect wire format"): Effects groups its entries into
// Blur ▸ / Sharpen ▸ / Distort ▸ / Noise ▸ submenus, Adjustments is flat.

import type { MenuDef } from '../components/layout/menu';
import { t } from '../i18n/index.svelte';
import { EFFECTS, EFFECT_MENU_GROUPS, GROUP_LABELS, type EffectDef } from './catalog';
import { effectLabel, groupLabel } from './labels';

/** Effects + Adjustments menus; `pick` opens the effect's dialog. */
export function effectMenus(pick: (def: EffectDef) => void, disabled: boolean): MenuDef[] {
  const item = (def: EffectDef) => ({
    label: `${effectLabel(def)}${def.params.length > 0 ? '…' : ''}`,
    disabled,
    run: () => pick(def),
  });
  return [
    {
      id: 'effects',
      label: t('menu.effects'),
      items: EFFECT_MENU_GROUPS.map((g) => ({
        kind: 'submenu' as const,
        label: groupLabel(g, GROUP_LABELS[g]),
        disabled,
        items: EFFECTS.filter((e) => e.group === g).map(item),
      })),
    },
    {
      id: 'adjustments',
      label: t('menu.adjustments'),
      items: EFFECTS.filter((e) => e.group === 'adjust').map(item),
    },
  ];
}
