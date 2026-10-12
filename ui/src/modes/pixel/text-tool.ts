// Text tool: anchor, live preview, commit.

import { packColor } from '../../lib/color';
import { t } from '../../lib/i18n/index.svelte';
import { loadDefaultFont } from '../../lib/text/font';
import type { TextParams } from './components/TextDialog.svelte';
import { paintOverlays, recompose, syncMeta } from './render';
import { colors, ed, live, toolState } from './stores/editor.svelte';

export async function openTextAt(point: { x: number; y: number }) {
  if (!ed.doc) return;
  try {
    live.textFontId = await loadDefaultFont();
    toolState.textAnchor = point;
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.textFont'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function previewTextParams(p: TextParams) {
  if (!ed.doc || !live.renderer || !toolState.textAnchor || live.textFontId === null) return;
  if (p.text.length === 0) {
    recompose();
    return;
  }
  try {
    const frame = ed.doc.previewText(
      live.textFontId,
      p.text,
      toolState.textAnchor.x,
      toolState.textAnchor.y,
      p.size,
      packColor(colors.color, colors.alpha),
      p.bold,
      p.italic,
      p.antiAlias,
      p.align,
      1,
    );
    try {
      live.renderer.draw(frame);
    } finally {
      frame.free();
    }
    paintOverlays();
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.textPreview'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function applyTextParams(p: TextParams) {
  if (!ed.doc || !toolState.textAnchor || live.textFontId === null) return;
  const anchor = toolState.textAnchor;
  toolState.textAnchor = null;
  try {
    const placed = ed.doc.drawText(
      live.textFontId,
      p.text,
      anchor.x,
      anchor.y,
      p.size,
      packColor(colors.color, colors.alpha),
      p.bold,
      p.italic,
      p.antiAlias,
      p.align,
    );
    if (placed) {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
      ed.status = t('px.textPlaced');
    } else {
      ed.status = t('px.textMissed');
      recompose();
    }
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.text'),
      error: err instanceof Error ? err.message : String(err),
    });
    recompose();
  }
}

export function cancelText() {
  toolState.textAnchor = null;
  recompose();
}
