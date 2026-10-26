import type { Handle } from '@sveltejs/kit/hooks';

// Fill `<html lang="%lang%">` per page, prerender included (website spec §11).
import { langOf } from '#lib/i18n/index.ts';

export const handle: Handle = ({ event, resolve }) => {
  const lang = langOf(event.params.lang);
  return resolve(event, {
    transformPageChunk: ({ html }) => html.replace('%lang%', lang),
  });
};
