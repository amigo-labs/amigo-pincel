// Whether a text field has the keyboard focus, for the native menu, whose
// text-editing entries and chords change with it
// (docs/specs/macos-native-menu.md AC-12, AC-13).

import { isTypingTarget } from '../shell/shortcuts';

/** `on` while a text field (input, textarea, contenteditable) has focus. */
export const textFocus = $state({ on: false });

let tracking = false;

/** Starts following the focus; safe to call more than once. */
export function trackTextFocus(): void {
  if (tracking || typeof window === 'undefined') return;
  tracking = true;
  textFocus.on = isTypingTarget(document.activeElement);
  window.addEventListener('focusin', (e) => {
    textFocus.on = isTypingTarget(e.target);
  });
  window.addEventListener('focusout', (e) => {
    textFocus.on = isTypingTarget(e.relatedTarget);
  });
}
