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
export const siteUrl = 'https://pincel.amigo-labs.dev';

// The editor is a separate Worker (`ui/`, project `pincel-app`) routed at
// `/app*` on the same host — it is not a SvelteKit route of this site, so
// links to it carry `rel="external"`: the client router does a full page
// load and the prerender crawler skips them. docs/specs/pincel.md §15, 2026-10-11.
export const appPath = '/app/';

export function absoluteUrl(pathname: string): string {
  if (!pathname.startsWith('/')) {
    return `${siteUrl}/${pathname}`;
  }
  return `${siteUrl}${pathname}`;
}
