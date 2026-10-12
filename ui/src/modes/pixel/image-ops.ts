// Image menu: effects, transforms, resize / scale, crop.

import type { EffectDef } from '../../lib/effects/catalog';
import { effectLabel } from '../../lib/effects/labels';
import { t } from '../../lib/i18n/index.svelte';
import type { ResizeResult } from './components/ResizeDialog.svelte';
import type { TransformOp } from './menus';
import { paintOverlays, recompose, syncMeta } from './render';
import { dialogs, ed, live } from './stores/editor.svelte';

/** An Image-menu command. */
export type ImageAction =
  | `layer:${TransformOp}`
  | `canvas:${TransformOp}`
  | 'canvas:resize'
  | 'canvas:scale'
  | 'canvas:crop';

export function openEffect(def: EffectDef) {
  if (!ed.doc) return;
  dialogs.effect = def;
}

export function previewEffectParams(params: number[]) {
  if (!ed.doc || !live.renderer || !dialogs.effect) return;
  if (params.length === 0 && dialogs.effect.params.length > 0) {
    recompose();
    return;
  }
  try {
    const frame = ed.doc.previewEffect(dialogs.effect.id, Float64Array.from(params), 1);
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

export function applyEffectParams(params: number[]) {
  if (!ed.doc || !dialogs.effect) return;
  const def = dialogs.effect;
  dialogs.effect = null;
  try {
    if (ed.doc.applyEffect(def.id, Float64Array.from(params))) {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
      ed.status = t('px.effectApplied', { effect: effectLabel(def) });
    } else {
      ed.status = t('px.effectOutside', { effect: effectLabel(def) });
      recompose();
    }
  } catch (err) {
    ed.status = t('px.effectFailed', {
      effect: effectLabel(def),
      error: err instanceof Error ? err.message : String(err),
    });
    recompose();
  }
}

export function cancelEffect() {
  dialogs.effect = null;
  recompose();
}

export function afterImageCommand(label: string) {
  live.needsRecompose = true;
  syncMeta();
  ed.docRev += 1;
  ed.status = label;
}

export function imageAction(action: ImageAction) {
  if (!ed.doc) return;
  try {
    switch (action) {
      case 'layer:flip_horizontal':
      case 'layer:flip_vertical':
      case 'layer:rotate_90_cw':
      case 'layer:rotate_90_ccw':
      case 'layer:rotate_180':
        ed.doc.transformLayer(action.slice('layer:'.length));
        afterImageCommand(t('px.layerTransformed'));
        break;
      case 'canvas:flip_horizontal':
      case 'canvas:flip_vertical':
      case 'canvas:rotate_90_cw':
      case 'canvas:rotate_90_ccw':
      case 'canvas:rotate_180':
        ed.doc.transformCanvas(action.slice('canvas:'.length));
        afterImageCommand(t('px.canvasTransformed'));
        break;
      case 'canvas:resize':
        dialogs.resize = 'resize';
        break;
      case 'canvas:scale':
        dialogs.resize = 'scale';
        break;
      case 'canvas:crop':
        if (ed.doc.cropToSelection()) afterImageCommand(t('px.cropped'));
        else ed.status = t('px.cropNone');
        break;
    }
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.imageCommand'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function submitResize(result: ResizeResult) {
  dialogs.resize = null;
  if (!ed.doc) return;
  try {
    if (result.mode === 'resize') {
      ed.doc.resizeCanvas(result.width, result.height, result.anchor);
      afterImageCommand(t('px.resized', { w: result.width, h: result.height }));
    } else {
      ed.doc.scaleImage(result.width, result.height, result.interpolation);
      afterImageCommand(t('px.scaled', { w: result.width, h: result.height }));
    }
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.resize'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}
