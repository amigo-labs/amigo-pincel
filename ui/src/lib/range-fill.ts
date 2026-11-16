// The shared slider (`am-range`) draws its travelled part from the custom
// property `--am-range-fill`, the value as a share of the range (Firefox
// draws it by itself). `use:rangeFill={value}` keeps the property in step
// with the input: on every input event and whenever `value` changes.

import type { Action } from 'svelte/action';

function share(el: HTMLInputElement): string {
  const min = Number(el.min || 0);
  const max = Number(el.max || 100);
  const value = Number(el.value);
  if (!(max > min) || !Number.isFinite(value)) return '0%';
  return `${Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100))}%`;
}

/** Svelte action for an `<input type="range" class="am-range">`. The
 *  parameter is the slider's value (or anything that changes with it): it
 *  only tells the action when to read the input again. */
export const rangeFill: Action<HTMLInputElement, unknown> = (el) => {
  const set = (): void => {
    el.style.setProperty('--am-range-fill', share(el));
  };
  // The value, `min` and `max` reach the element in the same flush as the
  // parameter, in no fixed order: read them once the flush is over.
  const later = (): void => {
    set();
    queueMicrotask(set);
  };
  later();
  el.addEventListener('input', set);
  el.addEventListener('change', set);
  return {
    update: later,
    destroy: () => {
      el.removeEventListener('input', set);
      el.removeEventListener('change', set);
    },
  };
};
