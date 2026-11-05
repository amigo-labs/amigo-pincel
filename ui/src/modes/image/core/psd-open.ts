// Opening Photoshop files (docs/specs/psd-import.md). The shell opens a PSD
// here *before* it mounts the Image editor, so a file that cannot be opened
// (PSB, 16-bit, CMYK, too large, broken) leaves the start screen or the open
// document as it was (AC-08 – AC-10). The opened document waits here until
// the editor's `openBytes` takes it, so the file is decoded once.
import { core, initCore, type PsdNotice, type PsdOpened } from './wasm';
import { MAX_CANVAS_AREA, MAX_CANVAS_DIM } from '../stores/editor.svelte';
import { t } from '../../../lib/i18n/index.svelte';

let prepared: { bytes: Uint8Array; opened: PsdOpened } | null = null;

/** Opens `bytes` as a PSD within Image mode's canvas limits and keeps the
 *  document for `takePsd`; throws (opening nothing) when it cannot. */
export async function preparePsd(bytes: Uint8Array): Promise<void> {
  await initCore();
  const opened = core.openPsd(bytes, MAX_CANVAS_DIM, MAX_CANVAS_AREA);
  if (prepared) core.closeDocument(prepared.opened.handle);
  prepared = { bytes, opened };
}

/** The PSD `preparePsd` opened from these very bytes, else opens it now. */
export function takePsd(bytes: Uint8Array): PsdOpened {
  const ready = prepared;
  if (ready) {
    prepared = null;
    if (ready.bytes === bytes) return ready.opened;
    core.closeDocument(ready.opened.handle);
  }
  return core.openPsd(bytes, MAX_CANVAS_DIM, MAX_CANVAS_AREA);
}

function line(n: PsdNotice): string {
  switch (n.kind) {
    case 'fallback':
      return t(`psd.fallback.${n.reason}`);
    case 'adjustment':
      return t('psd.adjustment', { layer: n.layer });
    case 'text':
      return t('psd.text', { layer: n.layer });
    case 'smart_object':
      return t('psd.smartObject', { layer: n.layer });
    case 'effects':
      return t('psd.effects', { layer: n.layer });
    case 'mask':
      return t('psd.mask', { layer: n.layer });
    case 'clipping':
      return t('psd.clipping', { layer: n.layer });
    case 'blend':
      return t('psd.blend', { layer: n.layer, key: n.key });
    case 'pass_through':
      return t('psd.passThrough', { group: n.group });
  }
}

/** The one status-bar notice for an import's notices; `null` when the
 *  import changed nothing (AC-06, AC-07). */
export function psdNotice(notices: readonly PsdNotice[]): string | null {
  if (notices.length === 0) return null;
  return t('psd.notice', { items: notices.map(line).join('; ') });
}
