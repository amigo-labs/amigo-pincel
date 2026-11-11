// Text tool: anchor, live preview, commit.

import { packColor } from '../../lib/color';
import type { UsedFace } from '../../lib/fonts/text-font.svelte';
import { t } from '../../lib/i18n/index.svelte';
import type { TextParams } from './components/TextDialog.svelte';
import { paintOverlays, recompose, syncMeta } from './render';
import { colors, ed, live, toolState } from './stores/editor.svelte';
import { notePainted } from './recent-colors';
import { pixelFont } from './text-font';

/** The face a preview or commit of `p` uses: family, Bold, Italic. */
function faceKey(bold: boolean, italic: boolean): string {
  return `${pixelFont.state.family}\u0000${bold}\u0000${italic}`;
}

/** The face `live.textFace` holds, and the one being read. */
let shownKey = '';
let wantedKey = '';
/** The last previewed parameters, redrawn once a newer face is read. */
let lastParams: TextParams | null = null;

function fontFailed(err: unknown): void {
  ed.status = t('status.failed', {
    action: t('act.textFont'),
    error: err instanceof Error ? err.message : String(err),
  });
}

/**
 * Reads the face for Bold / Italic of the current family, then shows it in
 * the preview; until then the preview keeps the previous face (AC-09).
 * `gesture` asks for an undecided permission from the current press (AC-22).
 */
function want(bold: boolean, italic: boolean, gesture = false): void {
  const key = faceKey(bold, italic);
  if (key === shownKey) {
    wantedKey = '';
    return;
  }
  if (key === wantedKey) return;
  wantedKey = key;
  pixelFont
    .resolve(bold, italic, gesture)
    .then((face) => {
      if (wantedKey !== key) return;
      wantedKey = '';
      shownKey = key;
      live.textFace = face;
      if (lastParams && toolState.textAnchor) previewTextParams(lastParams);
    })
    .catch((err: unknown) => {
      if (wantedKey === key) wantedKey = '';
      fontFailed(err);
    });
}

export function openTextAt(point: { x: number; y: number }) {
  if (!ed.doc) return;
  lastParams = null;
  if (live.textFace === null) shownKey = '';
  // From the press itself: an undecided permission is asked now (AC-22).
  want(false, false, true);
  toolState.textAnchor = point;
  void pixelFont.needed();
}

export function previewTextParams(p: TextParams) {
  lastParams = p;
  want(p.bold, p.italic);
  const face = live.textFace;
  if (!ed.doc || !live.renderer || !toolState.textAnchor || face === null) return;
  if (p.text.length === 0) {
    recompose();
    return;
  }
  try {
    const frame = ed.doc.previewText(
      face.id,
      p.text,
      toolState.textAnchor.x,
      toolState.textAnchor.y,
      p.size,
      packColor(colors.color, colors.alpha),
      face.synthBold,
      face.synthItalic,
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

/** The face a commit of `p` uses: the shown one when it is current, else
 *  read now with a loading notice (AC-18). */
async function commitFace(p: TextParams): Promise<UsedFace | null> {
  const key = faceKey(p.bold, p.italic);
  if (key === shownKey && live.textFace !== null) return live.textFace;
  ed.status = t('font.loading', { family: pixelFont.state.family });
  try {
    return await pixelFont.resolve(p.bold, p.italic);
  } catch (err) {
    fontFailed(err);
    return null;
  }
}

export async function applyTextParams(p: TextParams) {
  const doc = ed.doc;
  if (!doc || !toolState.textAnchor) return;
  const anchor = toolState.textAnchor;
  toolState.textAnchor = null;
  lastParams = null;
  const face = await commitFace(p);
  if (face === null || ed.doc !== doc) {
    recompose();
    return;
  }
  const before = doc.undoDepth;
  try {
    const placed = doc.drawText(
      face.id,
      p.text,
      anchor.x,
      anchor.y,
      p.size,
      packColor(colors.color, colors.alpha),
      face.synthBold,
      face.synthItalic,
      p.antiAlias,
      p.align,
    );
    if (placed) {
      notePainted(before, [packColor(colors.color, colors.alpha)]);
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
  lastParams = null;
  recompose();
}
