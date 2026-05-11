import type { RequestHandler } from './$types';
import { siteUrl } from '$lib/config';

export const prerender = true;

export const GET: RequestHandler = () => {
  const body = `User-agent: *
Allow: /
Disallow: /app

Sitemap: ${siteUrl}/sitemap.xml
`;
  return new Response(body, {
    headers: { 'Content-Type': 'text/plain' },
  });
};
