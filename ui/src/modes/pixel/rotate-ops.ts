// Layer ▸ Rotate…: any-angle rotation with Nearest or RotSprite and a live
// preview (docs/specs/rotsprite.md). The core decides what turns: the
// selection content, else the active layer's cel in the current frame.

import { t } from '../../lib/i18n/index.svelte';
import { afterImageCommand } from './image-ops';
import { paintOverlays, recompose } from './render';
import { dialogs, ed, live } from './stores/editor.svelte';

/** Wire names of the core's rotation methods. */
export type RotateMethod = 'nearest' | 'rotsprite';

/** What the open Rotate dialog turns: input size and the RotSprite cap. */
export interface RotateDialogState {
  width: number;
  height: number;
  max: number;
}

/** Angle bounds of the dialog (AC-07, AC-11). */
export const MIN_ANGLE = -180;
export const MAX_ANGLE = 180;

/** Whole degrees, clamped to the nearest bound (AC-11). */
export function clampAngle(v: number): number {
  if (!Number.isFinite(v)) return 0;
  return Math.max(MIN_ANGLE, Math.min(MAX_ANGLE, Math.round(v)));
}

/** RotSprite refuses inputs wider or taller than the cap (AC-09). */
export function rotspriteTooLarge(s: RotateDialogState): boolean {
  return s.width > s.max || s.height > s.max;
}

/** Opens the dialog, or says why nothing can rotate (AC-10). */
export function openRotate(): void {
  if (!ed.doc) return;
  const layer = ed.doc.paintTargetLayer();
  if (layer !== 0xffffffff && ed.doc.layerLocked(layer)) {
    ed.status = t('layers.locked', { name: ed.doc.layerName(layer) });
    return;
  }
  const [width, height] = ed.doc.rotateInputSize();
  if (width === undefined || height === undefined) {
    ed.status = t('pxRotate.nothing');
    return;
  }
  dialogs.rotate = { width, height, max: ed.doc.rotspriteMaxSide() };
}

/** Draws the would-be result; the document stays as it is (AC-08). */
export function previewRotate(angle: number, method: RotateMethod): void {
  const state = dialogs.rotate;
  if (!ed.doc || !live.renderer || !state) return;
  if (method === 'rotsprite' && rotspriteTooLarge(state)) {
    recompose();
    return;
  }
  try {
    const frame = ed.doc.previewRotate(angle, method, 1);
    try {
      live.renderer.draw(frame);
    } finally {
      frame.free();
    }
    paintOverlays();
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.preview'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function applyRotate(angle: number, method: RotateMethod): void {
  const state = dialogs.rotate;
  if (!ed.doc || !state) return;
  if (method === 'rotsprite' && rotspriteTooLarge(state)) return;
  dialogs.rotate = null;
  try {
    if (ed.doc.rotateLayer(angle, method)) afterImageCommand(t('pxRotate.done', { angle }));
    else {
      ed.status = t('pxRotate.nothing');
      recompose();
    }
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.imageCommand'),
      error: err instanceof Error ? err.message : String(err),
    });
    recompose();
  }
}

export function cancelRotate(): void {
  dialogs.rotate = null;
  recompose();
}
