// Image mode's Text-tool font (docs/specs/system-fonts.md): the family the
// options bar shows, remembered for Image mode, and the face the live text is
// drawn with for the tool's Bold / Italic, registered with
// `pincel-image-wasm`.

import { createTextFont, type UsedFace } from '../../../lib/fonts/text-font.svelte';
import { t } from '../../../lib/i18n/index.svelte';
import { editor, tool } from '../stores/editor.svelte';
import { core, initCore } from './wasm';

export const imageFont = createTextFont(
  'image',
  async (bytes, index) => {
    await initCore();
    return core.registerFont(bytes, index);
  },
  (message) => {
    editor.notice = message;
  },
);

/** The face for the current family and the tool's Bold / Italic. */
function key(): string {
  return `${imageFont.state.family}\u0000${tool.textBold}\u0000${tool.textItalic}`;
}

let shown: { key: string; face: UsedFace } | null = null;
let loading: { key: string; face: Promise<UsedFace> } | null = null;

/** The face read last, current or not; `null` before the first. */
export function loadedFace(): UsedFace | null {
  return shown?.face ?? null;
}

/** The face for the tool's current options when it has been read. */
export function currentFace(): UsedFace | null {
  return shown !== null && shown.key === key() ? shown.face : null;
}

/**
 * Reads the face for the tool's current family, Bold and Italic. Call it
 * synchronously from a press with `gesture` set: an undecided permission is
 * asked for from that press (AC-22). A failed load is retried on the next
 * call (image-live-text AC-19).
 */
export function ensureFace(gesture = false): Promise<UsedFace> {
  const k = key();
  if (shown !== null && shown.key === k) return Promise.resolve(shown.face);
  if (loading !== null && loading.key === k) return loading.face;
  const face = imageFont.resolve(tool.textBold, tool.textItalic, gesture);
  const entry = { key: k, face };
  loading = entry;
  face
    .then((f) => {
      shown = { key: k, face: f };
    })
    .catch(() => undefined)
    .finally(() => {
      if (loading === entry) loading = null;
    });
  return face;
}

/** A font change still being read for the live text (AC-18). */
let pendingEdit: Promise<void> | null = null;

/** Whether a font change for the live text is still being read. */
export function fontEditPending(): boolean {
  return pendingEdit !== null;
}

/** Records the font change `edit` the live text waits for. */
export function trackFontEdit(edit: Promise<void>): void {
  const p = edit.finally(() => {
    if (pendingEdit === p) pendingEdit = null;
  });
  pendingEdit = p;
}

/** Waits for a font change still being read, with a loading notice
 *  (AC-18); returns at once when none is. */
export async function fontEditsDone(): Promise<void> {
  if (pendingEdit === null) return;
  editor.notice = t('font.loading', { family: imageFont.state.family });
  await pendingEdit;
}
