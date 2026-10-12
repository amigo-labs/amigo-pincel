// Site language (website spec §11): English at `/`, German under `/de`.
// The language comes from the optional `[[lang=locale]]` route parameter,
// so every page reads it from `page.params` and re-renders on navigation.

import { page } from '$app/state';
import { de } from './de';
import { en, type Messages } from './en';

export type { Messages } from './en';

/** Languages the site ships, the default first. */
export const LANGS = ['en', 'de'] as const;
export type Lang = (typeof LANGS)[number];

const MESSAGES: Record<Lang, Messages> = { en, de };

/** The language of a route parameter value. */
export function langOf(param: string | undefined): Lang {
  return param === 'de' ? 'de' : 'en';
}

/** The current page's language. */
export function lang(): Lang {
  return langOf(page.params.lang);
}

/** The current page's copy. */
export function m(): Messages {
  return MESSAGES[lang()];
}

/** `path` (unprefixed, e.g. `/features`) in language `l`. */
export function localized(path: string, l: Lang = lang()): string {
  if (l === 'en') return path;
  return path === '/' ? `/${l}` : `/${l}${path}`;
}

/** A pathname without its language prefix (`/de/about` → `/about`). */
export function unlocalized(pathname: string): string {
  const stripped = pathname.replace(/^\/de(?=\/|$)/, '');
  return stripped === '' ? '/' : stripped;
}
