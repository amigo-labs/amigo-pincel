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
- Styled `404.html` served by Cloudflare Pages for unknown paths
- Custom pixel-art icons (hand-coded SVG grids; final set will be drawn in Pincel itself)
- Honest comparison table, feature grid, hero scene with torch flicker
- Cloudflare Pages deploy pipeline (see below)

Deferred to follow-up work:

- Hand-drawn-in-Pincel icon set replacing the SVG placeholders
- Hand-drawn Open Graph images per page (currently relies on `/og/default.svg`)
- Real screenshots in `/features` (currently shows placeholder frames)
- Live embed demo on `/embed` (currently shows placeholder)
- Editor mount on `/app` (waits on `@amigo-labs/pincel` publish)
- Service worker for the marketing site
- `/changelog`, `/docs`, `/showcase` (Phase 2/3)

## Cloudflare Pages deploy

The site deploys to Cloudflare Pages (project: `pincel-website`). Configuration:

- `wrangler.toml` — project name + `pages_build_output_dir = "./build"`
- `static/_headers` — long cache on hashed `_app/immutable/*`, short cache on HTML,
  baseline security headers
- `static/404.html` — Cloudflare Pages auto-serves this for unknown routes
- `.github/workflows/deploy-website.yml` — runs `pnpm check && pnpm lint && pnpm build`
  on every push/PR that touches `website/`, then `wrangler pages deploy build` for
  production (push to `main`) and preview (PRs)

Required GitHub repository secrets:

| Secret                  | Purpose                                  |
| ----------------------- | ---------------------------------------- |
| `CLOUDFLARE_API_TOKEN`  | Pages: Edit permission                   |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare account hosting the project   |

Manual deploy (from `website/`):

```bash
pnpm build
pnpm dlx wrangler pages deploy build --project-name=pincel-website
```

Production origin is set in `src/lib/config.ts` (currently `https://pincel.app`).
Update there if the domain decision in spec §6.1 lands on a different value.
