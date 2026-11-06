import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, test } from '@playwright/test';
import {
  defaultValues,
  EFFECTS,
  forMode,
  toParams,
  type EffectDef,
  type EffectMode,
} from '../src/lib/effects/catalog';

// mcp-parity-drawing-image AC-11: the defaults `effect.apply` and
// `adjust.apply` use in each mode equal the defaults the editor's effect
// dialog of that mode opens with. `crates/pincel-agent/src/effect-defaults.json`
// is the agent's table, regenerated and pinned by the Rust test
// `effect_defaults_file_is_current`; colour slots are null there because
// colour defaults follow the editor's colour swatches. No browser page.

type Wires = Record<string, Record<EffectMode, (number | null)[]>>;

const wires = JSON.parse(
  readFileSync(resolve(process.cwd(), '../crates/pincel-agent/src/effect-defaults.json'), 'utf8'),
) as Wires;

/** The default wire vector of `def` in `mode`, colour slots null. */
function editorDefaults(def: EffectDef, mode: EffectMode): (number | null)[] {
  const shown = forMode(def, mode);
  const wire: (number | null)[] = toParams(shown, defaultValues(shown));
  if (shown.toParams) return wire;
  let slot = 0;
  for (const p of shown.params) {
    const width = p.kind === 'point' || p.kind === 'pair' ? 2 : 1;
    if (p.kind === 'color') wire[slot] = null;
    slot += width;
  }
  return wire;
}

test('agent effect defaults equal the editor catalog in each mode', () => {
  expect(Object.keys(wires).sort()).toEqual(EFFECTS.map((e) => e.id).sort());
  for (const def of EFFECTS) {
    for (const mode of ['pixel', 'image'] as const) {
      const agent = wires[def.id]?.[mode] ?? [];
      const editor = editorDefaults(def, mode);
      expect(editor.length, `${def.id} (${mode}) slot count`).toBe(agent.length);
      editor.forEach((v, i) => {
        const a = agent[i];
        if (v === null || a === null || a === undefined) {
          expect(a ?? null, `${def.id} (${mode}) slot ${i} colour`).toBe(v);
        } else {
          expect(a, `${def.id} (${mode}) slot ${i}`).toBeCloseTo(v, 5);
        }
      });
    }
  }
});
