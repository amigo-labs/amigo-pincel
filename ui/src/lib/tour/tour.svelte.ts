// The first-run tour (docs/specs/first-run-tutorial.md): which context's
// tour is open and at which step, and the stored choices that decide when
// the start-screen banner and the per-editor hints offer it. Prefs live in
// IndexedDB; without it the choices hold for the session only.

import { getPref, setPref } from '../idb/prefs';

/** Where a tour runs: the start screen or one of the editors. */
export type TourContext = 'start' | 'pixel' | 'image';

export const tour = $state({
  /** The open tour's context, `null` while none runs. */
  context: null as TourContext | null,
  /** Index of the shown step. */
  step: 0,
  /** The prefs below are read (the banner and hints wait for them). */
  loaded: false,
  /** The user chose Tour on the banner. */
  seen: false,
  /** The user chose No thanks on the banner. */
  declined: false,
  /** Each editor has offered its part once. */
  hinted: { pixel: false, image: false },
});

/** Reads the stored choices once at boot. */
export async function loadTourPrefs(): Promise<void> {
  const [seen, declined, pixel, image] = await Promise.all([
    getPref('tourSeen').catch(() => undefined),
    getPref('tourDeclined').catch(() => undefined),
    getPref('tourHintPixel').catch(() => undefined),
    getPref('tourHintImage').catch(() => undefined),
  ]);
  tour.seen = seen === true;
  tour.declined = declined === true;
  tour.hinted = { pixel: pixel === true, image: image === true };
  tour.loaded = true;
}

/** Whether the start screen offers the tour (AC-01). */
export function bannerShown(): boolean {
  return tour.loaded && !tour.seen && !tour.declined;
}

/** The banner's choice, stored (AC-03); Tour starts the start-screen tour. */
export function answerBanner(take: boolean): void {
  if (take) {
    tour.seen = true;
    void setPref('tourSeen', true).catch(() => {});
    startTour('start');
  } else {
    tour.declined = true;
    void setPref('tourDeclined', true).catch(() => {});
  }
}

/** Whether `mode`'s editor still offers its part of the tour (AC-10). */
export function hintDue(mode: 'pixel' | 'image'): boolean {
  return tour.loaded && !tour.declined && !tour.hinted[mode];
}

/** The editor showed its hint: never again for that editor (AC-10). */
export function markHinted(mode: 'pixel' | 'image'): void {
  tour.hinted = { ...tour.hinted, [mode]: true };
  void setPref(mode === 'pixel' ? 'tourHintPixel' : 'tourHintImage', true).catch(() => {});
}

/** Starts `context`'s tour at step 1 (AC-02, AC-11). */
export function startTour(context: TourContext): void {
  tour.context = context;
  tour.step = 0;
}

/** Ends the tour (Skip, Esc, Done). */
export function endTour(): void {
  tour.context = null;
  tour.step = 0;
}
