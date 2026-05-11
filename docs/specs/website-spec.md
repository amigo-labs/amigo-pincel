# Pincel Website — Specification

> **Status:** Draft, Specification v0.1
> **Type:** Living Document
> **Last updated:** 2026-05-07
> **Owner:** Daniel Rück / amigo-labs
> **Sister-doc:** `docs/specs/pincel.md` (the editor itself)

---

## 1. Goals & Non-Goals

### 1.1 Goals

The Pincel website has four jobs, in priority order:

1. **Convert visitors into users in under 30 seconds.** "Open Editor" is the primary CTA on every page above the fold.
2. **Build trust with game-dev pros and pixel artists.** Honest comparisons, transparent roadmap, visible open-source posture.
3. **Pitch embedders.** A dedicated page targeting tool-makers who want `pincel-wasm` in their own apps.
4. **Reflect the product's identity.** Pixel-art-themed, retro-modern, recognizably part of the amigo-labs ecosystem.

### 1.2 Non-Goals

- No marketing fluff. No "revolutionary" / "cutting-edge" / "transform your workflow" copy.
- No login walls. The editor is usable without an account, ever.
- No tracking beyond privacy-respecting analytics (Plausible self-hosted or none at all).
- No "Sign up for the newsletter" modal.
- No cookie banner unless GDPR strictly requires it (analytics-free start: no banner needed).
- No marketplace, no asset store. Pincel is a tool, not a platform.

### 1.3 Success Criteria

- Lighthouse: Performance ≥ 95, Accessibility ≥ 95, Best Practices = 100, SEO ≥ 95
- Time to first meaningful paint < 1.5s on mid-tier mobile
- `/` to `/app` click-through rate ≥ 25% (instrument once analytics decision is made)
- Site bundle (excluding editor on `/app`) < 200 KB compressed

---

## 2. Audience & Personas

The site speaks to four reader-mindsets. Every page must have something for at least one of them above the fold.

### 2.1 The Indie Game Dev (primary)

- Builds a 2D game in Godot, Unity, Bevy, LÖVE, or amigo-engine
- Already uses or considered Aseprite; possibly priced out, possibly wants web-based
- Cares about: file format compatibility, animation tags, slices for hitboxes, tilemap support
- Skeptical of: "another half-baked tool"; needs to see feature parity quickly

### 2.2 The Pixel Artist (primary)

- Posts on Bluesky / Mastodon / Cohost / Pixeljoint
- Uses Aseprite, Piskel, or Photoshop
- Cares about: drawing feel, palette tools, animation timeline, tablet support
- Skeptical of: web tools that feel laggy or have weird input handling

### 2.3 The Tool Maker (secondary)

- Building a level editor, asset pipeline, game-jam framework, or no-code tool
- Wants to embed a pixel editor without writing one
- Cares about: stable API, small bundle, framework-agnostic, license clarity
- Skeptical of: vaporware embed APIs that don't actually work

### 2.4 The amigo-engine User (tertiary)

- Already in the amigo-labs ecosystem
- Cares about: integrated workflow, hot-reload, asset pipeline coherence
- Skeptical of: little — they're already convinced; just show them the path

---

## 3. Information Architecture

### 3.1 Routes

```
/                Home (hero + features + CTA)
/features        Feature deep-dive with screenshots and short clips
/app             The editor itself (loads pincel-wasm)
/embed           For tool makers; code samples, API reference summary
/about           Project story, open-source posture, amigo-labs context
/changelog       Release notes, version history (Phase 2)
/showcase        Community gallery (Phase 3, opt-in submissions)
/docs/*          User documentation (Phase 2)
```

### 3.2 Phase Scoping

**Phase 1 (launch)**
- `/`
- `/features`
- `/app`
- `/embed`
- `/about`

**Phase 2 (post-launch)**
- `/changelog`
- `/docs/*` — full user docs

**Phase 3 (community-driven)**
- `/showcase`

### 3.3 Global Navigation

Top bar, fixed, transparent over hero, solid below the fold:

```
PINCEL    Home  Features  For Devs  About    [Open Editor →]
```

Footer:

```
Pincel — by amigo-labs. MIT/Apache-2.0.
GitHub  Mastodon  Bluesky  RSS
Built with Pincel itself. ▲
```

The "Built with Pincel itself" line links to the meta page where we show that the site's pixel art was made in Pincel — visible dogfooding.

---

## 4. Page Specs

### 4.1 `/` Home

**Purpose:** sell the tool in 30 seconds.

Sections, top to bottom:

#### 4.1.1 Hero

- **Logo + Wordmark** (Pincel, pixel-font)
- **Tagline (one line, no marketing-speak):**
  *"A pixel-art editor for the web. Aseprite-compatible. Open source. Free."*
- **Sub-tagline (one short paragraph):**
  *"Make sprites, tilemaps, and animations in your browser. Save as `.aseprite`. Drop into your game. No account, no upload, no catch."*
- **Primary CTA:** `[ Open Editor → ]` — links to `/app`
- **Secondary CTA:** `[ View on GitHub ]` — links to repo
- **Hero visual:** an animated pixel-art scene (looped, ~2-4 seconds, ≤16 frames). Built in Pincel. Shows a small character running, a torch flickering, a tilemap-built room. No video — it's a sprite sheet, animated via CSS `steps()`. Demonstrates the product *with* the product.

Constraints:
- Hero must fit on one viewport on a 13" laptop without scrolling
- Hero must work without JavaScript (CTA buttons are real `<a>` tags)
- Hero animation falls back to a static image if `prefers-reduced-motion`

#### 4.1.2 The Pitch (one paragraph)

Short prose, no bullets. Roughly:

> Pincel is built for game developers and pixel artists who want a serious tool that runs in a browser tab. It reads and writes the Aseprite file format, so your work fits into existing pipelines. It supports tilemaps, slices, animation tags, palettes, and indexed color — the things you actually need. It runs offline as a PWA, and you can install it on iPad. It's open source under MIT or Apache 2.0, and it will stay that way.

#### 4.1.3 Feature Grid (9 tiles)

3×3 grid on desktop, 1-column on mobile. Each tile: pixel-art icon, headline, one-sentence description.

| Icon | Headline | Description |
|------|----------|-------------|
| 📁 (pixel) | Aseprite-Compatible | Read and write `.aseprite` files. Drop into your game pipeline. |
| 🧱 (pixel) | Tilemaps | First-class tilemap layers with tilesets. Stamp, swap, edit in place. |
| ✂️ (pixel) | Slices | Named regions, 9-patch, pivot points — for hitboxes, UI, and game logic. |
| 🎞️ (pixel) | Animation | Frames, tags, onion skin, ping-pong. Tag your states; export sheets. |
| 🎨 (pixel) | Palette Workflow | Indexed color, palette swaps, named colors. Pixel-art done right. |
| 📱 (pixel) | Tablet & Pen | Pointer events with pressure, tilt, pinch-to-zoom. Pencil-friendly. |
| ⚡ (pixel) | Offline PWA | Install it. Works without a connection. Your files stay on your device. |
| 🔌 (pixel) | Embeddable | npm package. Drop into your tool. Same editor, in your app. |
| 🧰 (pixel) | Open Source | MIT / Apache 2.0. Forkable, auditable, yours. |

#### 4.1.4 The Honest Comparison

A small comparison table. Honest about where Aseprite is still ahead. No "vs Aseprite, we win in every category" tables.

| | Pincel | Aseprite | Piskel |
|---|---|---|---|
| Aseprite file format (read+write) | ✅ | ✅ native | ❌ |
| Tilemaps | ✅ | ✅ | ❌ |
| Slices (9-patch, pivot) | ✅ | ✅ | ❌ |
| Animation timeline + tags | ✅ | ✅ | ✅ (basic) |
| Tablet / pen support | ✅ | ✅ | ❌ |
| Runs in browser | ✅ | ❌ | ✅ |
| Native desktop app | ✅ Tauri | ✅ | ✅ (via NW.js, ageing) |
| Embeddable as a library | ✅ | ❌ | partial |
| Lua scripting / extensions | ❌ Phase 2 | ✅ | ❌ |
| Custom brushes | ❌ Phase 2 | ✅ | ✅ |
| Price | Free | Paid | Free |
| Open source | ✅ MIT/Apache | ✅ EULA-restricted | ✅ Apache 2.0 |
| Active development | ✅ | ✅ | 🟡 modernization in progress |

The footer of the table reads: *"Pincel is new and Aseprite has 14 years of features. We're catching up where it matters for game-dev workflows. Where it doesn't, we're not."*

#### 4.1.5 How It Works (3 steps)

Visual: three pixel-art panels with arrows.

1. **Open** — drag in a `.aseprite` file or start fresh. *Icon: file dropping into a window.*
2. **Edit** — paint, animate, build tilemaps, define slices. *Icon: cursor with brush.*
3. **Save** — save back to `.aseprite`. Your engine hot-reloads. *Icon: file going into a game window.*

#### 4.1.6 Final CTA

Big, centered, final push: `[ Open Editor → ]`

Below in smaller text: *"Or: `npm install @amigo-labs/pincel` to embed it →"* (links to `/embed`)

---

### 4.2 `/features` Feature Deep-Dive

Long-scroll page, one section per feature. Each section: title, one-paragraph description, screenshot or short clip, key keyboard shortcuts.

Sections (in order):

1. **The Canvas** — zoom, pan, grids, symmetry, reference layer
2. **Tools** — pencil, eraser, bucket, line, rect, ellipse, eyedropper, move, selection
3. **Layers** — image, group, blend modes, opacity, lock
4. **Frames & Animation** — timeline, tags, onion skin, durations
5. **Tilemaps** — tilesets, stamp, edit-in-place, flip/rotate flags
6. **Slices** — 9-patch, pivots, per-frame keys, naming for engine pickup
7. **Palette** — indexed mode, named entries, palette swap, import/export
8. **File Format** — Aseprite read/write, PNG export, sprite-sheet export with sidecar JSON
9. **Tablet & Pen** — pressure, tilt, two-finger pan, on-screen modifiers (iPad)
10. **PWA** — install instructions per browser, offline behavior, autosave
11. **What's Coming** — phase 2 features with rough timelines

Each section has:
- A short heading (3-5 words)
- A one-paragraph description
- A visual (screenshot, short MP4, or animated PNG)
- Optional: pull-quote keyboard shortcuts (e.g., `B` pencil, `E` eraser)

No section is more than ~120 words of prose. The page is dense visually, light textually.

---

### 4.3 `/app` The Editor

This route hosts the editor itself. It loads the same Svelte UI that the standalone PWA build uses, sharing components from `ui/`.

**Constraints:**

- The editor route is the dogfood test for the embed package. It imports `@amigo-labs/pincel` exactly the way an external embedder would.
- This route is not crawled by search engines (`<meta name="robots" content="noindex">`).
- Ships its own service worker for offline use.
- Has its own minimal chrome — no marketing nav. A small "Pincel" wordmark in the corner that links back to `/`.

The site's marketing JS is **not** loaded on this route. `/app` is a separate Vite entry point.

---

### 4.4 `/embed` For Developers

**Audience:** the tool maker who needs to know in 60 seconds whether Pincel will work in their app.

Page structure:

#### 4.4.1 Pitch (one paragraph)

> Pincel ships as `@amigo-labs/pincel`, a framework-agnostic npm package. Mount it in any DOM element. Read and write Aseprite files. Listen for changes. Built on `pincel-core` (Rust → WASM). Same editor, in your app, no iframe.

#### 4.4.2 Quick Start

```bash
npm install @amigo-labs/pincel
```

```typescript
import { Pincel, ColorMode } from '@amigo-labs/pincel';

const pincel = await Pincel.create({
  width: 64,
  height: 64,
  colorMode: ColorMode.Rgba,
});

pincel.mount(document.getElementById('editor'));

pincel.on('change', () => {
  console.log('document modified');
});

const bytes = await pincel.saveAseprite();
```

#### 4.4.3 Live Embed Demo

A sandboxed Pincel instance running in a small viewport on the page itself, with a "View source" toggle showing the actual mount code. Demonstrates that the embed actually works.

#### 4.4.4 What You Get

- Full editor UI, themeable (CSS variables documented)
- Public events: `change`, `save`, `selection`, `tool-change`, `palette-change`
- Imperative API: `loadFile`, `saveAseprite`, `exportPng`, `setActiveLayer`, etc.
- TypeScript types shipped
- ~XXX KB gzipped (real number, measured, updated per release)
- Works in any framework: React adapter, Svelte adapter, plain JS

#### 4.4.5 What It Costs

> Free, MIT or Apache 2.0. No telemetry, no licensing, no per-seat fees. If you ship a product using Pincel, you don't owe us anything. If you want to credit us, that's nice.

#### 4.4.6 Honest Limits

- Bundle is not tiny — it's an editor (~XXX KB). For small embeds, consider a read-only viewer (Phase 2).
- No headless / Node API in Phase 1 (browser-only). Phase 2 adds Node support for asset pipelines.
- Aseprite Lua scripts are not supported and not planned.

---

### 4.5 `/about`

Three paragraphs, no headshots, no startup-vibe.

> Pincel is part of amigo-labs, an open-source ecosystem of game-development tools built in Rust. The flagship is amigo-engine — a deterministic ECS-based 2D engine with a built-in level editor, AI asset pipelines, and chiptune music tooling. Pincel fills the pixel-art-editor gap in that ecosystem, and stands alone for anyone outside it.

> The project is open source under MIT or Apache 2.0 — your choice. The format we use is Aseprite's, which is documented and reimplementable. We are not affiliated with the Aseprite project, and we don't compete with it: Aseprite has 14 years of refinement and a deep feature set we won't catch up to overnight. Pincel exists for people who want a free, web-based, embeddable alternative.

> No funding round. No subscription tier coming. No "open core" gotcha. The plan is to keep the tool good and the codebase clean, and to maintain it for as long as people use it.

Below: a small section listing the team (probably just Daniel for now, with a note that contributors are welcome). Link to GitHub.

---

## 5. Visual Design System

### 5.1 Identity

**Theme:** retro-modern pixel art with CRT undertones. Influence map: itch.io, PICO-8 docs, Aseprite's website, Bitsy, classic JRPG menu UIs.

**What it is not:** edgy hacker theme, corporate SaaS gradient, glassmorphism, neumorphism.

### 5.2 Color Palette

Base palette is **PICO-8** — the de facto pixel-art community palette. Instantly recognized by the target audience. We use a subset for UI, and reserve the rest as accents.

```
─── Base UI ───
bg-base          #0d0d12   (slightly cooler than PICO-8 black, helps text contrast)
bg-elevated      #1d2b53   (pico-8 dark blue)
bg-card          #1a1a23
border-subtle    #2c2c38
border-strong    #5f574f   (pico-8 dark grey)

─── Foreground ───
fg-primary       #fff1e8   (pico-8 white)
fg-secondary     #c2c3c7   (pico-8 light grey)
fg-muted         #83769c   (pico-8 lavender)

─── Brand & Accents ───
brand-primary    #29adff   (pico-8 blue — Pincel's hero color; see §12 2026-05-11 decision)
accent-warn      #ffa300   (pico-8 orange)
accent-error     #ff004d   (pico-8 red)
accent-success   #00e436   (pico-8 green)
accent-highlight #ffec27   (pico-8 yellow — sparingly, for "new" badges)
```

All values exposed as CSS variables in `:root`. Dark theme is the default; light theme is Phase 2 (and stays opt-in).

### 5.3 Typography

Two font families. Pixel for personality, modern for legibility.

- **Display / Pixel font:** [VT323](https://fonts.google.com/specimen/VT323) for hero titles and section headers. Open license. Distinct CRT character.
- **Body font:** [Inter](https://rsms.me/inter/) for body, navigation, and dense text. Loaded with `display=swap`, subset to Latin + European extended.
- **Code:** [JetBrains Mono](https://www.jetbrains.com/lp/mono/) for code blocks on `/embed`.

Type scale (mobile-first, scales up on larger viewports):

```
display-1   VT323 56px/1.1   (hero only)
display-2   VT323 40px/1.15  (page titles)
heading-1   Inter 600 28px/1.3
heading-2   Inter 600 22px/1.35
heading-3   Inter 600 18px/1.4
body-lg     Inter 400 18px/1.6  (intro paragraphs)
body        Inter 400 16px/1.65
body-sm     Inter 400 14px/1.5
caption     Inter 500 12px/1.4 (uppercase tracking +0.05em for labels)
code        JetBrains Mono 14px/1.5
pixel-label VT323 18px/1     (chunky retro labels)
```

### 5.4 Spacing & Grid

Pixel-grid alignment. Base unit: 4px. All spacing is a multiple. Common values: 4, 8, 12, 16, 24, 32, 48, 64, 96.

Container max-width: `1200px`. Sections breathe with 96px vertical rhythm on desktop, 48px on mobile.

### 5.5 Components

#### 5.5.1 Buttons

Hard-edged 2px borders, no rounded corners. Press-state shifts the entire button 2px down-right (classic pixel-button feel).

```
[ PRIMARY BUTTON ]   bg: brand-primary   border: bg-base 2px   shadow: 4px 4px bg-base
[ secondary ]        transparent bg      border: border-strong 2px
[ tertiary text ]    no border           hover underline
```

Hover: brightness +10% on primary; underline on tertiary.
Active: shifts +2px x and +2px y; shadow disappears.
Focus: outline 2px accent-highlight, offset 2px.

#### 5.5.2 Cards

```
bg-card with border-subtle 1px
optional accent-color top border (4px, brand-primary)
no rounded corners
hover: border becomes border-strong, slight lift via shadow
```

#### 5.5.3 Pixel Decorations

Small repeating sprites used as section dividers and hover effects. Drawn at 8×8 or 16×16, scaled with `image-rendering: pixelated`. Inventory:

- Brush stroke (Pincel's logo motif)
- Pixel-art arrow (replaces `→` in CTAs at large sizes)
- Star sparkle (decorative, for "new" badges)
- Cursor (for "click here" hints)

All custom, drawn in Pincel itself, exported as SVG-with-pixel-grid or PNG at 1× and 2×.

#### 5.5.4 The CRT Effect (Optional, Toggle-able)

A subtle scan-line overlay and minor chromatic-aberration on the hero only, never on body text. Implementation: a CSS `::before` pseudo-element with a repeating linear-gradient at 1px intervals, mix-blend-mode `overlay`, opacity `0.04`.

Toggle in footer: *"CRT effect: [on / off]"* — preference saved in localStorage. Default: on.

Respects `prefers-reduced-motion` and `prefers-reduced-transparency`: disabled if either is set.

### 5.6 Iconography

All icons are **custom, drawn at 16×16 or 24×24 in Pincel itself**. No icon-font, no Lucide. Style: 1px lines, limited palette, slightly chunky. Exported as SVG with pixel-perfect grid-snapping.

This is a deliberate choice: every icon on the site is dogfood proof.

### 5.7 Motion

- Use `steps()` timing functions for animations that should feel pixel-y (sprite loops, hover bounces)
- Use `ease-out` only for layout transitions (panel slides, page transitions)
- Default duration: 200ms. Hero animation is its own thing (looped sprite, ~2-4s)
- All motion respects `prefers-reduced-motion: reduce` — disables decorative animation, keeps only essential transitions

---

## 6. Tech Stack

### 6.1 Stack

- **Framework:** SvelteKit (Svelte 5 + adapter-static)
- **Build:** Vite
- **Styling:** Tailwind 4 with custom theme matching the design tokens above
- **Components:** shadcn-svelte where needed; custom-built for anything pixel-themed
- **Hosting:** Cloudflare static edge (the repo's existing project uses Workers Builds with `[assets]`; see §12 2026-05-11). Alternative: GitHub Pages if zero-vendor-lock is preferred.
- **Domain:** `pincel.app` if available, else `pincel.amigo-labs.dev`. (Decision deferred; check DNS.)

### 6.2 Build & Deploy

- Static site generation. No SSR runtime.
- All pages prerendered at build time.
- The `/app` route is a separate Vite entry; its bundle is fully isolated from the marketing pages so the marketing site loads fast and the editor doesn't contaminate it.
- Deploys are triggered by Cloudflare's Workers Builds Git integration on every push and PR (no GitHub Actions on the deploy side; see §12 2026-05-11).
- Preview deploys for every PR.

### 6.3 Performance Budgets

Hard limits, enforced in CI:

| Resource | Budget |
|----------|--------|
| Marketing page HTML+CSS+JS (excluding `/app`) | < 200 KB compressed |
| Largest contentful image | < 100 KB |
| Total page weight (marketing) | < 500 KB |
| Time to interactive (3G simulated) | < 2.5 s |
| Lighthouse Performance | ≥ 95 |

`/app` has its own budget (the editor itself, see editor spec).

### 6.4 Repo Layout

```
amigo-pincel/
├── crates/
│   ├── pincel-core/
│   ├── aseprite-writer/
│   └── pincel-wasm/
├── ui/                       Shared editor UI
├── src-tauri/                Native desktop
├── website/                  ← THIS PROJECT
│   ├── src/
│   │   ├── routes/
│   │   │   ├── +page.svelte           /
│   │   │   ├── features/+page.svelte
│   │   │   ├── embed/+page.svelte
│   │   │   ├── about/+page.svelte
│   │   │   └── app/+page.svelte       imports @amigo-labs/pincel
│   │   ├── lib/
│   │   │   ├── components/   pixel-themed building blocks
│   │   │   ├── icons/        custom Pincel-drawn SVGs
│   │   │   └── styles/       design tokens, global CSS
│   │   └── app.html
│   ├── static/
│   │   ├── sprites/          hero animation, decorative pixel art
│   │   └── og/               Open Graph images per page
│   ├── svelte.config.js
│   ├── vite.config.ts
│   └── package.json
└── docs/specs/
    ├── pincel.md
    └── website.md            ← this file
```

The `website/` directory is its own pnpm workspace package, sibling to `ui/`. It depends on `@amigo-labs/pincel` (which is built from `pincel-wasm`) for the `/app` route.

---

## 7. SEO & Meta

### 7.1 Per-Page Meta

Every page has:

- `<title>` — descriptive, brand-suffixed: *"Tilemaps in Pincel — Pincel"*
- `<meta name="description">` — 150-160 chars, written manually per page
- Open Graph: `og:title`, `og:description`, `og:image`, `og:url`, `og:type`
- Twitter card: `twitter:card=summary_large_image`, all OG fields mirror

### 7.2 Open Graph Images

One per page, 1200×630 PNG, generated at build time. Design: pixel-art style with the page title in VT323 over a Pincel-themed scene. Built with the same pixel-art tooling as the rest of the site.

For `/app`, the OG image shows a screenshot of the editor itself.

### 7.3 Sitemap & Robots

- `sitemap.xml` auto-generated, includes all marketing pages, excludes `/app`
- `robots.txt` allows all crawling on marketing pages, disallows `/app/*`

### 7.4 Structured Data

JSON-LD on `/`:

- `SoftwareApplication` schema with name, description, applicationCategory (`DesignApplication`), operatingSystem (`Web, Windows, macOS, Linux, iPadOS`), offers (`Free`)

---

## 8. Accessibility

WCAG 2.2 AA target. Specific commitments:

- Color contrast: every foreground/background combination must pass AA. The PICO-8 palette has some tricky pairs (yellow on white, pink on dark) — those combos are forbidden in body text. Verified in CI via a contrast-checker script over the design tokens.
- Keyboard: every interactive element reachable via Tab, with visible focus rings (2px accent-highlight).
- Screen readers: semantic HTML — `<nav>`, `<main>`, `<article>`, real `<button>` elements. ARIA used only where HTML semantics fall short.
- Pixel decorations are `aria-hidden="true"`. The hero animation has a static fallback and respects `prefers-reduced-motion`.
- The CRT effect respects `prefers-reduced-transparency` and `prefers-reduced-motion`.
- Alt text on every meaningful image. Decorative pixel art has empty alt.
- Language tags: `<html lang="en">` for the English site. (German variant deferred to Phase 2.)

---

## 9. Analytics

**Default: no analytics.** The site ships analytics-free at launch.

If analytics are added later, the only acceptable option is **self-hosted Plausible** or equivalent: no cookies, no PII, no third-party data sharing, GDPR-clean without a banner. Decision goes through the Decision Log first.

We never instrument the editor (`/app`) beyond crash reporting (which itself is opt-in and Phase 2).

---

## 10. Content Tone

Voice principles:

- **Direct.** No "we believe that" or "in today's fast-paced world."
- **Honest.** Where Aseprite is better, say so. Where Pincel is rough, say so.
- **Plain.** Sentences ≤ 25 words on average. Active voice.
- **Slightly playful.** This is a pixel-art tool, not enterprise compliance software. A bit of personality is correct.
- **No emojis in body copy.** Pixel-art icons fill that role.

Forbidden words: "cutting-edge", "revolutionary", "leverage", "empower", "ecosystem-defining", "next-generation", "transformative", "best-in-class".

Accepted patterns:

- *"It does X. It doesn't do Y."* — direct, declarative
- *"Built for X. If you don't need X, that's fine — it works for Y too."* — inclusive without bloat
- *"Aseprite has Z. We don't, yet."* — honest comparison

---

## 11. Internationalization

**Phase 1: English only.** No i18n infrastructure.

**Phase 2:** German translation as the second language. Implementation: SvelteKit's built-in i18n via route prefixes (`/de/`, default English at `/`). Translations live in `src/lib/i18n/<lang>.json`.

No machine translation. Every translation is reviewed by a native speaker.

---

## 12. Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-05-07 | Multi-page site with `/app` as a route, not a separate subdomain | Single deploy, shared design tokens, dogfoods the embed package |
| 2026-05-07 | Pixel-art-themed visual identity using PICO-8 palette | Instant recognition by target audience; avoids generic SaaS look; ties to amigo-labs ecosystem |
| 2026-05-07 | SvelteKit + adapter-static, Cloudflare Pages | Same stack as the editor UI; static deploy is fast and cheap; matches amigo-labs tooling |
| 2026-05-07 | No analytics by default; if added, self-hosted Plausible only | Privacy-first stance; GDPR clean without banners; aligns with no-tracking-no-account product principle |
| 2026-05-07 | All icons drawn in Pincel itself | Dogfooding proof; custom identity; no icon-font dependency |
| 2026-05-07 | English-only at launch; German Phase 2 | Audience is English-speaking primarily; German is the natural second locale via amigo-labs context |
| 2026-05-07 | Honest comparison table including where Aseprite wins | Trust-building with the skeptical audience; "we beat them at everything" tables are widely distrusted |
| 2026-05-07 | `/app` is a separate Vite entry, isolated bundle | Marketing site stays fast; editor route doesn't drag down the home page |
| 2026-05-11 | `brand-primary` is PICO-8 blue (#29adff), not PICO-8 pink (#ff77a8); `brand-secondary` retired | Owner preference. Blue keeps the PICO-8 identity, gives strong contrast on the dark base, and avoids overlap with the pink-saturated pixel-art tooling space (Aseprite, Piskel branding). `brand-secondary` was unused — single brand accent simplifies the token set. |
| 2026-05-11 | Deploy via Cloudflare Workers Builds Git integration (project `amigo-pincel`), not Cloudflare Pages with a GitHub Actions workflow | The repo was already wired to a Workers project in the dashboard. Workers Static Assets serves prerendered SvelteKit output identically to Pages for our use case, honors the same `_headers` / `_redirects` / `404.html` conventions, and avoids running two deploy pipelines against the same project. A root-level `wrangler.toml` (`[build]` + `[assets]`) handles the build server-side; no GitHub Actions deploy workflow is involved. |

---

## 13. Phase 1 Milestones

| # | Milestone | Exit Criterion |
|---|-----------|----------------|
| W1 | SvelteKit scaffold + design tokens + base layout components | `pnpm dev` runs; design tokens produce expected output on a sample page |
| W2 | `/` home page with hero (static image first), feature grid, comparison, footer | Lighthouse ≥ 90 on all categories |
| W3 | Custom icon set drawn in Pincel itself; integrated into all pages | All Phase 1 pages have icons; no Lucide / icon-font dep present |
| W4 | `/features` page with screenshots and content | Content review passed; all visuals are real Pincel screenshots |
| W5 | `/embed` page with working live demo (mounting `@amigo-labs/pincel`) | Live demo works in 3 browsers; copy-paste code from page works in a fresh project |
| W6 | `/about` page; copy review; final tone pass | Owner sign-off on copy |
| W7 | Hero animation: animated pixel-art scene, looping, in-Pincel-built | Hero animation in place, respects `prefers-reduced-motion` |
| W8 | OG images per page; SEO meta; sitemap; robots | OG images render correctly on Bluesky / Mastodon / Twitter / Discord previews |
| W9 | Accessibility pass: keyboard nav, screen reader, contrast | WCAG 2.2 AA verified manually + automated check in CI |
| W10 | Performance pass: bundle audit, image optimization, font loading | All Section 6.3 budgets met; Lighthouse ≥ 95 across the board |
| W11 | Domain, deploy pipeline, preview deploys, analytics decision | Site live on production domain with PR previews working |
| W12 | Launch — coordinate with editor M4 milestone | Site goes live concurrent with first usable editor demo |

---

## 14. Open Questions

1. **Domain:** `pincel.app` (likely taken or expensive) vs. `pincel.amigo-labs.dev` (subdomain, free, less brandable). Check availability.
2. **Hero animation source:** is it a hand-crafted scene, or a stylized rendering of "the editor itself in motion"? Both work; the choice affects how technical the hero feels vs. how artsy.
3. **Showcase / community gallery:** Phase 3, but should we collect submissions from launch (low-key Form / Bluesky tag) so we have material when we build it?
4. **Newsletter / RSS:** RSS is cheap (just expose `/changelog` as a feed); newsletter has ongoing maintenance cost. RSS yes, newsletter no, until proven needed.
5. **Self-hosted analytics:** if we add Plausible later, do we run our own instance or use the hosted version? Self-hosted is more work, hosted is third-party.
6. **OG image generation:** at build time (Satori or similar) or hand-drawn per page (more work, more on-brand)? Recommend hand-drawn for the major pages, Satori for `/changelog` entries.

---

*End of specification v0.1. Living document — amend in place via PR. Significant decisions go in Section 12.*
