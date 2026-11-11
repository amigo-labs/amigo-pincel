// Pixel mode's Text-tool font (docs/specs/system-fonts.md): the family the
// Text dialog uses, remembered for Pixel mode, its faces registered with
// `pincel-wasm`.

import { registerFont } from '../../lib/core';
import { createTextFont } from '../../lib/fonts/text-font.svelte';
import { ed } from './stores/editor.svelte';

export const pixelFont = createTextFont(
  'pixel',
  (bytes, index) => registerFont(bytes, index),
  (message) => {
    ed.status = message;
  },
);
