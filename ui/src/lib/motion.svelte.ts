// "Reduce motion" (docs/specs/reduced-motion.md, spec §9.4, Decision Log
// 2026-10-29): one flag for both editors and the shell. Motion is reduced
// while the system asks for it (`prefers-reduced-motion: reduce`, followed
// live) or Settings ▸ Reduce motion is checked (an IndexedDB pref); the option
// can only add the request, never force motion on. Reduced motion stills
// the marching selection outlines and finishes UI transitions instantly
// (`app.css`, through `data-reduce-motion` on <html>); frame playback and
// the animation preview are content and keep running.

import { isIdbAvailable } from './idb/db';
import { getPref, setPref } from './idb/prefs';

/** IndexedDB pref holding Settings ▸ Reduce motion (`true` = always reduce). */
export const REDUCE_MOTION_PREF = 'reduceMotion';

const QUERY = '(prefers-reduced-motion: reduce)';

class Motion {
  /** The system or browser asks for reduced motion. */
  system = $state(false);
  /** View ▸ Reduce Motion is checked. */
  option = $state(false);
  /** Motion is reduced: the system asks for it or the option is on. */
  readonly reduced = $derived(this.system || this.option);
}

/** The shared reduced-motion state. */
export const motion = new Motion();

let listening = false;

/** Mirrors the option on `<html>` for the CSS that the media query can't see. */
function applyAttribute(): void {
  if (typeof document === 'undefined') return;
  if (motion.option) document.documentElement.dataset.reduceMotion = '';
  else delete document.documentElement.dataset.reduceMotion;
}

/** Starts following the system setting (once). */
function listen(): void {
  if (listening || typeof window === 'undefined' || !window.matchMedia) return;
  listening = true;
  const media = window.matchMedia(QUERY);
  motion.system = media.matches;
  media.addEventListener('change', (e) => {
    motion.system = e.matches;
  });
}

listen();

/** Follows the system setting and restores the saved option. */
export async function initMotion(): Promise<void> {
  listen();
  if (isIdbAvailable()) {
    try {
      motion.option = (await getPref(REDUCE_MOTION_PREF)) === true;
    } catch (err) {
      console.error('getPref reduceMotion failed', err);
    }
  }
  applyAttribute();
}

/** Checks or unchecks View ▸ Reduce Motion and remembers it. */
export function setReduceMotion(on: boolean): void {
  motion.option = on;
  applyAttribute();
  if (isIdbAvailable()) {
    setPref(REDUCE_MOTION_PREF, on).catch((err: unknown) => {
      console.error('setPref reduceMotion failed', err);
    });
  }
}

