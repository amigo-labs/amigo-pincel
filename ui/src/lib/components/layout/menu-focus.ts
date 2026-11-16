// The shared menu fills the item under the pointer and the item marked
// `data-active`. `use:activeOnFocus` marks a menu item while it has the
// focus, so the item the keyboard is on is filled like a hovered one.
// With a parameter, the item is also marked while the parameter is true
// (the row of an open submenu, the flyout entry under a long-press finger).

import type { Action } from 'svelte/action';

/** Svelte action for a menu item: sets `data-active` while it is focused
 *  or while `also` is true. */
export const activeOnFocus: Action<HTMLElement, boolean | undefined> = (el, also) => {
  let focused = document.activeElement === el;
  let forced = also === true;
  const apply = (): void => {
    if (focused || forced) el.dataset.active = '';
    else delete el.dataset.active;
  };
  const on = (): void => {
    focused = true;
    apply();
  };
  const off = (): void => {
    focused = false;
    apply();
  };
  apply();
  el.addEventListener('focus', on);
  el.addEventListener('blur', off);
  return {
    update: (next) => {
      forced = next === true;
      apply();
    },
    destroy: () => {
      el.removeEventListener('focus', on);
      el.removeEventListener('blur', off);
    },
  };
};
