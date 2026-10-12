// Fill `<html lang="%lang%">` per page, prerender included (website spec §11).
import type { Handle } from '@sveltejs/kit';
import { langOf } from '$lib/i18n';

export const handle: Handle = ({ event, resolve }) => {
  const lang = langOf(event.params.lang);
  return resolve(event, {
    transformPageChunk: ({ html }) => html.replace('%lang%', lang),
  });
};
