// The palette of the shared amigo app theme (docs/specs/amigo-theme.md):
// Settings ▸ Theme chooses Dark (the default), Light or System, kept as an
// IndexedDB pref. `theme/tokens.css` is dark on :root, light under
// `data-theme="light"`, and follows the system when <html> carries no
// `data-theme`; so Dark and Light set the attribute and System removes it.

import { isIdbAvailable } from './idb/db';
import { getPref, setPref } from './idb/prefs';

/** What the user can choose in Settings ▸ Theme. */
export type ThemeChoice = 'dark' | 'light' | 'system';

/** The choices in the order Settings shows them. */
export const THEME_CHOICES: readonly ThemeChoice[] = ['dark', 'light', 'system'];

/** IndexedDB pref holding the theme choice. */
export const THEME_PREF = 'theme';

/** How long the first render waits for the stored choice before starting dark. */
const READ_LIMIT_MS = 400;

const QUERY = '(prefers-color-scheme: light)';

/** Whether `value` is a theme choice; anything else counts as no choice. */
export function isThemeChoice(value: unknown): value is ThemeChoice {
  return value === 'dark' || value === 'light' || value === 'system';
}

class Theme {
  /** The user's choice; Dark while none is stored. */
  choice = $state<ThemeChoice>('dark');
  /** The system prefers a light palette. */
  systemLight = $state(false);
  /** The palette on screen: what the choice resolves to. */
  readonly palette = $derived<'dark' | 'light'>(
    this.choice === 'system' ? (this.systemLight ? 'light' : 'dark') : this.choice,
  );
}

/** The shared theme state. */
export const theme = new Theme();

/** Puts the choice on <html> and keeps the page's `theme-color` on the ground colour. */
function apply(): void {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;
  if (theme.choice === 'system') delete root.dataset.theme;
  else root.dataset.theme = theme.choice;
  const ground = getComputedStyle(root).getPropertyValue('--am-bg').trim();
  if (ground) document.querySelector('meta[name="theme-color"]')?.setAttribute('content', ground);
}

let listening = false;

/** Starts following the system preference (once). */
function listen(): void {
  if (listening || typeof window === 'undefined' || !window.matchMedia) return;
  listening = true;
  const media = window.matchMedia(QUERY);
  theme.systemLight = media.matches;
  media.addEventListener('change', (e) => {
    theme.systemLight = e.matches;
    apply();
  });
}

function adopt(saved: unknown): void {
  if (isThemeChoice(saved)) theme.choice = saved;
  apply();
}

/**
 * Restores the stored choice. Resolves once it is applied or after a short
 * limit, so the first render never shows the other palette (AC-08) and a
 * slow or broken store cannot hold the app back; a choice that arrives
 * late still applies.
 */
export async function initTheme(): Promise<void> {
  listen();
  apply();
  if (!isIdbAvailable()) return;
  const read = getPref(THEME_PREF).then(adopt, (err: unknown) => {
    console.error('getPref theme failed', err);
  });
  await Promise.race([read, new Promise<void>((resolve) => setTimeout(resolve, READ_LIMIT_MS))]);
}

/** Switches the palette and remembers the choice. */
export function setThemeChoice(next: ThemeChoice): void {
  theme.choice = next;
  apply();
  if (isIdbAvailable()) {
    setPref(THEME_PREF, next).catch((err: unknown) => {
      console.error('setPref theme failed', err);
    });
  }
}
