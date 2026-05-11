// Site-wide configuration. Edit here, not in components.
//
// `siteUrl` is the canonical production origin used for absolute URLs in
// `<link rel="canonical">`, Open Graph tags, sitemap.xml, and robots.txt. It
// must be an absolute URL with no trailing slash because routes are appended
// directly (e.g. `${siteUrl}/features`).
//
// During prerender SvelteKit uses `http://sveltekit-prerender/` as its dummy
// origin, which is not safe to emit. We hard-code the production origin and
// derive everything from it.
export const siteUrl = 'https://pincel.app';

export function absoluteUrl(pathname: string): string {
  if (!pathname.startsWith('/')) {
    return `${siteUrl}/${pathname}`;
  }
  return `${siteUrl}${pathname}`;
}
