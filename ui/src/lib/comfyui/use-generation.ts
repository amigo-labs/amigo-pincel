// "Use in Generate panel" of a generated layer, for both editors
// (docs/specs/comfyui-generation-info.md AC-05, AC-06): the layer's values
// go into the panel's fields and the panel is expanded, as if the user
// expanded it — which also checks the model against the server.

import { expandPanel } from '../components/layout/panels.svelte';
import type { LayerGeneration } from '../components/layers/layers';
import { isTauri } from '../platform';
import { useGeneration } from './generation.svelte';

/** The layers panel's handler for `mode`; none where the editor has no
 *  Generate panel (the browser app, AC-06). */
export function generationUser(mode: 'image' | 'pixel'): ((g: LayerGeneration) => void) | undefined {
  if (!isTauri()) return undefined;
  return (g) => {
    useGeneration(g);
    expandPanel(mode, 'generate');
  };
}
