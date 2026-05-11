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
- SEO meta, sitemap, robots, favicon
- Custom pixel-art icons (hand-coded SVG grids; final set will be drawn in Pincel itself)
- Honest comparison table, feature grid, hero scene with torch flicker

Deferred to follow-up work:

- Hand-drawn-in-Pincel icon set replacing the SVG placeholders
- Hand-drawn Open Graph images per page (currently relies on `/og/default.png` once added)
- Real screenshots in `/features` (currently shows placeholder frames)
- Live embed demo on `/embed` (currently shows placeholder)
- Editor mount on `/app` (waits on `@amigo-labs/pincel` publish)
- Service worker for the marketing site
- `/changelog`, `/docs`, `/showcase` (Phase 2/3)
