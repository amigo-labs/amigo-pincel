// The Clone Stamp's stroke command (docs/specs/clone-stamp.md): the brush
// tip settings the other brush tools share, the tool's own Sample, and the
// active tab's fixed offset.
import { core, type CloneStrokeCommand } from './wasm';
import { editor, tool } from '../stores/editor.svelte';
import { opacity01, penFields, syncInfo } from './controller';
import { activeCloneSource } from '../tools/clone-source.svelte';

/** Clones along the given points; one undo step per `strokeId`. Does
 *  nothing before the stroke's press fixed the offset. */
export function cloneStroke(
  points: Array<[number, number]>,
  strokeId: number,
  pressure: number[] = [],
  azimuth: Array<number | null> = [],
): void {
  const offset = activeCloneSource()?.offset ?? null;
  if (editor.handle === null || points.length === 0 || offset === null) {
    return;
  }
  const cmd: CloneStrokeCommand = {
    type: 'clone_stroke',
    layer: editor.activeLayer,
    size: tool.size,
    opacity: opacity01(),
    shape: tool.shape,
    hardness: Math.min(1, Math.max(0, tool.hardness / 100)),
    offset: [offset[0], offset[1]],
    sample: tool.cloneSample,
    points,
    ...penFields(pressure, azimuth),
    stroke_id: strokeId,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}
