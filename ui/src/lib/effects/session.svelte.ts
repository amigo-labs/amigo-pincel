// One open effect (effect-panel spec): the effect, its edited values and the
// Preview toggle. The editor owns the session, not the panel component: the
// frame re-mounts the panel when the window crosses a layout boundary
// (sidebar / drawer / bottom sheet, AC-14), and the values must survive that.

import { defaultValues, type EffectDef } from './catalog';

let nextToken = 0;

/** The state of the effect whose panel is shown. `def` stays the catalog
 *  object (not a proxy); the edited fields are reactive. */
export class EffectSession {
  readonly def: EffectDef;
  /** New per opening, so re-choosing the same effect restarts it (AC-05). */
  readonly token: number;
  /** Parameter values keyed by `ParamDef.key`. */
  values = $state<Record<string, number>>({});
  /** Live preview on. */
  preview = $state(true);
  /** Per linked pair, the side edited last (re-linking takes its value). */
  lastEdited = $state<Record<string, 'first' | 'second'>>({});

  /** A fresh session for `def` at its defaults, with the preview on. */
  constructor(def: EffectDef) {
    nextToken += 1;
    this.def = def;
    this.token = nextToken;
    this.values = defaultValues(def);
  }

  /** Back to the defaults (Reset). */
  reset(): void {
    this.values = defaultValues(this.def);
    this.lastEdited = {};
  }
}
