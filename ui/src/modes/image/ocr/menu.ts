// Image mode's Tools menu (docs/specs/ocr-dialog.md AC-01): Recognize
// Text… for now. Pixel mode has no Tools menu.

import type { MenuDef } from '../../../lib/components/layout/menu';
import { t } from '../../../lib/i18n/index.svelte';

/** The Tools menu; `recognizeText` opens the OCR dialog. */
export function toolsMenu(recognizeText: () => void, disabled: boolean): MenuDef {
  return {
    id: 'tools',
    label: t('menu.tools'),
    items: [{ label: t('menu.recognizeText'), disabled, run: recognizeText }],
  };
}
