import type { Page } from '@playwright/test';
import { de } from '../src/lib/i18n/de';
import { en, type Messages } from '../src/lib/i18n/en';

/** The site's languages and their copy, read from the dictionaries the pages render. */
export const COPY: Record<'en' | 'de', Messages> = { en, de };

/** The production origin every absolute URL uses (`src/lib/config.ts`). */
export const SITE = 'https://pincel.amigo-labs.dev';

/** The five pages, unprefixed (docs/specs/website.md AC-01). */
export const PAGES = ['/', '/features', '/download', '/embed', '/about'] as const;

/** `path` in language `lang`: English at the root, German under `/de`. */
export function localized(path: string, lang: 'en' | 'de'): string {
  if (lang === 'en') return path;
  return path === '/' ? '/de' : `/de${path}`;
}

/** Every page in every language, as `{ lang, path, url }`. */
export const ALL_PAGES = (['en', 'de'] as const).flatMap((lang) =>
  PAGES.map((path) => ({ lang, path, url: localized(path, lang) })),
);

/** The `<h1>` copy of an unprefixed page in `lang`. */
export function heading(path: (typeof PAGES)[number], lang: 'en' | 'de'): string {
  const c = COPY[lang];
  switch (path) {
    case '/':
      return c.hero.title;
    case '/features':
      return c.features.title;
    case '/download':
      return c.download.title;
    case '/embed':
      return c.embed.title;
    case '/about':
      return c.about.title;
  }
}

/**
 * Waits until the page has hydrated: SvelteKit starts the client with
 * dynamic imports after the prerendered HTML has loaded, so a click before
 * they ran hits inert markup.
 */
export async function hydrated(page: Page): Promise<void> {
  await page.waitForLoadState('networkidle');
}
