// What a raster drop onto an open editor does (docs/specs/file-access.md
// AC-25): ask, add the files as layers, or open the first as a document.
// "Don't ask again" in the drop dialog and File ▸ Dropped Images set it;
// stored as one IndexedDB pref like `autoNew`, best effort — without
// IndexedDB the choice holds for the session.

import type { MenuSubmenu } from '../components/layout/menu';
import { t } from '../i18n/index.svelte';
import { isIdbAvailable } from '../idb/db';
import { getPref, setPref } from '../idb/prefs';

/** The remembered action for dropped raster images. */
export type DropChoice = 'ask' | 'layer' | 'document';

const DROP_CHOICE_PREF = 'droppedImages';

/** The current choice; `'ask'` until the pref is read. */
export const dropChoice = $state<{ value: DropChoice }>({ value: 'ask' });

const isChoice = (v: unknown): v is DropChoice => v === 'ask' || v === 'layer' || v === 'document';

/** Reads the stored choice; keeps `'ask'` when unset or unreadable. */
export async function loadDropChoice(): Promise<void> {
  if (!isIdbAvailable()) return;
  try {
    const v = await getPref(DROP_CHOICE_PREF);
    if (isChoice(v)) dropChoice.value = v;
  } catch (err) {
    console.error('getPref droppedImages failed', err);
  }
}

/** Remembers `choice` for later drops, also after a restart. */
export function setDropChoice(choice: DropChoice): void {
  dropChoice.value = choice;
  if (!isIdbAvailable()) return;
  setPref(DROP_CHOICE_PREF, choice).catch((err: unknown) => {
    console.error('setPref droppedImages failed', err);
  });
}

/** File ▸ Dropped Images, with the current choice checked (both modes). */
export function droppedImagesMenu(): MenuSubmenu {
  const item = (choice: DropChoice, label: string) => ({
    label,
    checked: dropChoice.value === choice,
    run: () => setDropChoice(choice),
  });
  return {
    kind: 'submenu',
    label: t('menu.droppedImages'),
    items: [
      item('ask', t('menu.droppedAsk')),
      item('layer', t('menu.droppedLayer')),
      item('document', t('menu.droppedDocument')),
    ],
  };
}
