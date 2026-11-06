// Press handling of the Clone Stamp (docs/specs/clone-stamp.md): setting
// the source with Ctrl / ⌘ + click or the Set source toggle, refusing a
// stroke without a source, and the pointer position the crosshair follows.
import { editor, tool } from '../stores/editor.svelte';
import { t } from '../../../lib/i18n/index.svelte';
import { isMac } from '../../../lib/platform';
import { cloneHover, setCloneSource, strokeOffset } from './clone-source.svelte';

/** Ctrl, or ⌘ on macOS, where Ctrl+click is the secondary click. */
function modHeld(e: PointerEvent): boolean {
  return isMac() ? e.metaKey : e.ctrlKey;
}

/**
 * A press on the canvas that sets the source point instead of painting
 * (AC-03, AC-04): with Ctrl / ⌘ held or *Set source* on. Works on a
 * locked layer too (AC-14). `true` when the press is used up.
 */
export function pressCloneSource(e: PointerEvent, point: [number, number]): boolean {
  if (!modHeld(e) && !tool.cloneSetSource) return false;
  setCloneSource(point);
  tool.cloneSetSource = false;
  editor.notice = null;
  return true;
}

/**
 * Starts a stroke pressed at `point`: fixes the offset on the first stroke
 * after the source was set (AC-05). Without a source the status bar says
 * how to set one and nothing is painted (AC-13).
 */
export function beginCloneStroke(point: [number, number]): boolean {
  if (strokeOffset(point) !== null) return true;
  editor.notice = t(isMac() ? 'clone.noSourceMac' : 'clone.noSource');
  return false;
}

/**
 * Tracks the pointer for the source crosshair (AC-15): over the canvas,
 * or anywhere while a button is held (painting). `toCanvas` maps an event
 * to canvas pixels. Returns a teardown.
 */
export function attachCloneHover(
  stage: HTMLElement,
  toCanvas: (e: PointerEvent) => [number, number],
): () => void {
  const onMove = (e: PointerEvent): void => {
    const p = toCanvas(e);
    const inside = p[0] >= 0 && p[1] >= 0 && p[0] < editor.width && p[1] < editor.height;
    cloneHover.point = inside || e.buttons !== 0 ? p : null;
  };
  const onLeave = (): void => {
    cloneHover.point = null;
  };
  stage.addEventListener('pointermove', onMove);
  stage.addEventListener('pointerdown', onMove);
  stage.addEventListener('pointerleave', onLeave);
  return () => {
    stage.removeEventListener('pointermove', onMove);
    stage.removeEventListener('pointerdown', onMove);
    stage.removeEventListener('pointerleave', onLeave);
    cloneHover.point = null;
  };
}
