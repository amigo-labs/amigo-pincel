import type { RequestHandler } from './$types';

export const prerender = true;

export const GET: RequestHandler = () => {
  const body = `User-agent: *
Allow: /
Disallow: /app

Sitemap: https://pincel.app/sitemap.xml
`;
  return new Response(body, {
    headers: { 'Content-Type': 'text/plain' },
  });
};
