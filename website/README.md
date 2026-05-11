# Pincel Website

Marketing site for [Pincel](../). Built per [`docs/specs/website-spec.md`](../docs/specs/website-spec.md).

SvelteKit + adapter-static. Phase 1 routes: `/`, `/features`, `/embed`, `/about`, plus a
no-indexed `/app` placeholder that will host the editor bundle once
`@amigo-labs/pincel` ships.

## Development

```bash
cd website
pnpm install
pnpm dev          # http://localhost:5173
pnpm build        # static output in build/
pnpm preview      # serve build/
pnpm check        # svelte-check
pnpm lint
```

## Layout

```
src/
  routes/
    +layout.svelte            site chrome (Header, Footer)
    +layout.ts                prerender = true
    +page.svelte              /
    features/+page.svelte
    embed/+page.svelte
    about/+page.svelte
    app/+page.svelte          editor mount point (placeholder)
    sitemap.xml/+server.ts
    robots.txt/+server.ts
  lib/
    components/               Header, Footer, Hero, ... (Svelte 5 runes)
    data/                     feature catalogue
    icons/                    custom pixel-art SVGs
    stores/                   crt.svelte.ts (CRT toggle)
    styles/app.css            design tokens (PICO-8 palette)
static/
  favicon.svg
```

## Design tokens

PICO-8 palette as documented in the spec. All as Tailwind v4 `@theme` variables in
`src/lib/styles/app.css`. Two web fonts (VT323 for display, Inter for body, JetBrains Mono
for code) loaded from Google Fonts with `display=swap`.

## Phase 1 vs deferred

Built today:

- `/`, `/features`, `/embed`, `/about`, `/app` (placeholder)
- PICO-8 design tokens, pixel-art components, CRT toggle
- SEO meta with absolute canonical/OG URLs, sitemap, robots, favicon
- Styled `404.html` served by Cloudflare for unknown paths
- Custom pixel-art icons (hand-coded SVG grids; final set will be drawn in Pincel itself)
- Honest comparison table, feature grid, hero scene with torch flicker
- Cloudflare Workers Builds deploy pipeline (see below)

Deferred to follow-up work:

- Hand-drawn-in-Pincel icon set replacing the SVG placeholders
- Hand-drawn Open Graph images per page (currently relies on `/og/default.svg`)
- Real screenshots in `/features` (currently shows placeholder frames)
- Live embed demo on `/embed` (currently shows placeholder)
- Editor mount on `/app` (waits on `@amigo-labs/pincel` publish)
- Service worker for the marketing site
- `/changelog`, `/docs`, `/showcase` (Phase 2/3)

## Cloudflare deploy

The site deploys to Cloudflare via the **Workers Builds Git integration**
(project: `amigo-pincel`). The repo is already connected in the Cloudflare
dashboard; every push and PR triggers a build there directly. No GitHub
Actions workflow is involved on the deploy side — Cloudflare clones the
repo, runs the build, and serves the result.

Configuration that makes the build work:

- `../wrangler.toml` (repo root) — Cloudflare reads this. It declares the
  `[build]` command that builds this directory, and an `[assets]` block
  pointing at `website/build` with `not_found_handling = "404-page"` so
  unknown routes hit our styled `static/404.html`.
- `static/_headers` — long cache on hashed `_app/immutable/*`, short cache
  on HTML, baseline security headers (X-Content-Type-Options,
  X-Frame-Options, Referrer-Policy, Permissions-Policy).
- `static/404.html` — self-contained styled 404 served by Cloudflare for
  unknown paths (works without JS).

Production origin is set in `src/lib/config.ts` (currently
`https://pincel.app`). Update there if the domain decision in
`docs/specs/website-spec.md` §6.1 lands on a different value.
