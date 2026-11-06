// The Effects and Adjustments menus both modes show (spec §9.2, §15
// 2026-10-11 "One effect wire format", 2026-11-04 paint.net catalog;
// docs/specs/effect-catalog-v2.md): Effects groups its entries into
// paint.net's submenus, Adjustments is flat. Each mode sees only its
// entries, a submenu without entries is left out, and entries sort by
// their name in the UI language.

import type { MenuDef } from '../components/layout/menu';
import { locale, t } from '../i18n/index.svelte';
import {
  EFFECT_SUBMENUS,
  SUBMENU_LABELS,
  entriesFor,
  type EffectDef,
  type EffectGroup,
  type EffectMode,
} from './catalog';
import { effectLabel, groupLabel } from './labels';

/** Effects + Adjustments menus for `mode`; `pick` opens the effect's dialog. */
export function effectMenus(
  pick: (def: EffectDef) => void,
  disabled: boolean,
  mode: EffectMode,
): MenuDef[] {
  const collator = new Intl.Collator(locale());
  const item = (def: EffectDef) => ({
    label: `${effectLabel(def)}${def.params.length > 0 ? '…' : ''}`,
    disabled,
    run: () => pick(def),
  });
  const items = (group: EffectGroup) =>
    entriesFor(group, mode)
      .map((def) => ({ def, name: effectLabel(def) }))
      .sort((a, b) => collator.compare(a.name, b.name))
      .map(({ def }) => item(def));
  return [
    {
      id: 'effects',
      label: t('menu.effects'),
      liveWithEffect: true,
      items: EFFECT_SUBMENUS.map((g) => ({ g, entries: items(g) }))
        .filter(({ entries }) => entries.length > 0)
        .map(({ g, entries }) => ({
          kind: 'submenu' as const,
          label: groupLabel(g, SUBMENU_LABELS[g]),
          disabled,
          items: entries,
        })),
    },
    {
      id: 'adjustments',
      label: t('menu.adjustments'),
      liveWithEffect: true,
      items: items('adjust'),
    },
  ];
}
