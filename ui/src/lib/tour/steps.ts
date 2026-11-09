// The tour's steps per context (docs/specs/first-run-tutorial.md AC-05):
// at most seven each; the start screen's and an editor's lists together
// cover the two modes, creating or opening, the tool rail, the options bar,
// the panels, undo with the shortcut overview, and help. Anchors are found
// by role and accessible name or a `data-tour` mark, never by CSS class.

import type { MessageKey } from '../i18n/en';
import type { TourContext } from './tour-state.svelte';

/** How a step finds its anchor; `null` centres the callout. */
export type Anchor = { role: string; name?: MessageKey } | { tour: string } | null;

export interface TourStep {
  /** One sentence, at most 140 characters in English. */
  text: MessageKey;
  anchor: Anchor;
}

const EDITOR: readonly TourStep[] = [
  { text: 'tour.tools', anchor: { role: 'toolbar', name: 'frame.tools' } },
  { text: 'tour.options', anchor: { role: 'toolbar', name: 'frame.toolOptions' } },
  { text: 'tour.panels', anchor: { role: 'complementary', name: 'frame.panels' } },
  { text: 'tour.undo', anchor: { tour: 'quick' } },
  { text: 'tour.help', anchor: { role: 'menubar' } },
];

export const TOUR_STEPS: Record<TourContext, readonly TourStep[]> = {
  start: [
    { text: 'tour.modes', anchor: { tour: 'modes' } },
    { text: 'tour.create', anchor: { tour: 'open' } },
    { text: 'tour.samples', anchor: { tour: 'samples' } },
    { text: 'tour.startHelp', anchor: { tour: 'start-help' } },
  ],
  pixel: EDITOR,
  image: EDITOR,
};
