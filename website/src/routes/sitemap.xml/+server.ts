import type { RequestHandler } from './$types';
import { siteUrl } from '#lib/config.ts';
import { LANGS, localized } from '#lib/i18n/index.ts';

export const prerender = true;

const BASE = siteUrl;

const routes = ['/', '/features', '/download', '/embed', '/about'];

// Every page in every language, each listing its translations
// (`xhtml:link` hreflang alternates, website spec §7.3 / §11).
function alternates(path: string): string {
  return [...LANGS, 'x-default' as const]
    .map((l) => {
      const href = `${BASE}${localized(path, l === 'x-default' ? 'en' : l)}`;
      return `    <xhtml:link rel="alternate" hreflang="${l}" href="${href}" />`;
    })
    .join('\n');
}

export const GET: RequestHandler = () => {
  const today = new Date().toISOString().slice(0, 10);
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${LANGS.flatMap((l) =>
  routes.map(
    (path) => `  <url>
    <loc>${BASE}${localized(path, l)}</loc>
    <lastmod>${today}</lastmod>
    <changefreq>weekly</changefreq>
${alternates(path)}
  </url>`,
  ),
).join('\n')}
</urlset>
`;

  return new Response(body, {
    headers: { 'Content-Type': 'application/xml' },
  });
};
