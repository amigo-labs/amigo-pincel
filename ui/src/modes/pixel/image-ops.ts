// Image menu: effects, transforms, resize / scale, crop.

import { rgbaOf, type EffectDef } from '../../lib/effects/catalog';
import { effectLabel } from '../../lib/effects/labels';
import { EffectSession } from '../../lib/effects/session.svelte';
import { t } from '../../lib/i18n/index.svelte';
import type { ResizeResult } from './components/ResizeDialog.svelte';
import type { TransformOp } from './menus';
import { paintOverlays, recompose, syncMeta } from './render';
import { colors, dialogs, ed, live } from './stores/editor.svelte';

/** An Image-menu command. */
export type ImageAction =
  | `layer:${TransformOp}`
  | `canvas:${TransformOp}`
  | 'canvas:resize'
  | 'canvas:scale'
  | 'canvas:crop';

/** Opens `def` at its defaults in the effect panel; an effect already
 *  open is cancelled first, even the same one (effect-panel AC-05). */
export function openEffect(def: EffectDef) {
  if (!ed.doc) return;
  if (dialogs.effect) recompose();
  // Colour parameters start at the current colours (outline-effect AC-01).
  dialogs.effect = new EffectSession(def, {
    foreground: rgbaOf(colors.color, colors.alpha),
    background: rgbaOf(colors.bgColor, colors.bgAlpha),
  });
}

/** Live preview of the open effect; `null` (Preview off) shows the document. */
export function previewEffectParams(params: number[] | null) {
  if (!ed.doc || !live.renderer || !dialogs.effect) return;
  const def = dialogs.effect.def;
  if (params === null) {
    recompose();
    return;
  }
  try {
    const frame = ed.doc.previewEffect(def.id, Float64Array.from(params), 1);
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
  const def = dialogs.effect.def;
  let changed: boolean;
  try {
    changed = ed.doc.applyEffect(def.id, Float64Array.from(params));
  } catch (err) {
    // The panel stays open with its values; the document is unchanged
    // (effect-panel AC-16).
    ed.status = t('px.effectFailed', {
      effect: effectLabel(def),
      error: err instanceof Error ? err.message : String(err),
    });
    return;
  }
  dialogs.effect = null;
  if (changed) {
    live.needsRecompose = true;
    syncMeta();
    ed.docRev += 1;
    ed.status = t('px.effectApplied', { effect: effectLabel(def) });
  } else {
    ed.status = t('px.effectOutside', { effect: effectLabel(def) });
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
