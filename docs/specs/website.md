---
status: todo
created: 2026-10-29
size: M
mode: website
design: [website-spec.md §3.1, website-spec.md §3.3, website-spec.md §4, website-spec.md §5.5.4, website-spec.md §6.2, website-spec.md §7, website-spec.md §11, pincel.md §15 2026-10-12, pincel.md §15 2026-10-13]
---

# Marketing website

## Context

Pincel's marketing site lives under `website/` (SvelteKit, fully prerendered with the static adapter) and is served by the same Cloudflare Worker as the editor: the site owns `/`, the editor `/app/` (`pincel.md` §15, 2026-10-12 / 2026-10-13). Its design spec is `docs/specs/website-spec.md`; this spec records what the deployed site does today. It is reverse-engineered from the code.

The site has no automated tests yet: CI runs lint, `svelte-check` and the build for `website/` and nothing else. Evidence for AC-01 – AC-12 comes from Playwright specs under `website/e2e/` (see Technical notes); AC-13 stays manual.

## Scope

### In scope

- The five pages Home, Features, Download, For developers (`/embed`), About, in English at the root and German under `/de`
- Header (navigation, language switch, Open Editor CTA), footer (GitHub link, CRT toggle)
- Links to the editor at `/app/`
- SEO head per page, JSON-LD on Home, `sitemap.xml`, `robots.txt`
- The Home comparison table as it is today
- The download button and the Download page's file list

### Out of scope

- Changing the comparison table (columns, rows, licence wording) — because that is the planned `website-comparison` spec; this spec only pins the current table
- The editor at `/app/` — because it is a separate build copied into the site, not a route of it
- Cloudflare response headers and the build script (`static/_headers`, `scripts/cf-build.sh`, `wrangler.toml`) — because they are hosting configuration, listed under Technical notes
- Phase 2 / 3 routes (`/changelog`, `/docs`, `/showcase`), per-page OG images, real screenshots and the live embed demo — because they are planned in `website-spec.md`, not built
- Copy wording beyond what the ACs name — because it changes freely with marketing edits

## Acceptance criteria

- [ ] AC-01 (always): The site serves the pages Home, Features, Download, For developers and About as prerendered static HTML at `/`, `/features`, `/download`, `/embed`, `/about` in English and at `/de`, `/de/features`, `/de/download`, `/de/embed`, `/de/about` in German, without trailing slashes.
- [ ] AC-02 (always): Every page's `<html lang>` is `en` at the root and `de` under `/de`, and its navigation, headings and body copy are in that language.
- [ ] AC-03 (event): When the visitor chooses `EN` or `DE` in the header's language switch, then the same page opens in that language, and the switch marks the current language with `aria-current`.
- [ ] AC-04 (always): Internal links on a German page — header navigation, footer and links inside the copy — point to German pages under `/de`; on an English page they carry no language prefix.
- [ ] AC-05 (event): When the visitor follows the header's Open Editor link, the hero's Open Editor button or the final call to action, then the browser loads `/app/` as a new document.
- [ ] AC-06 (always): Every page's head carries a `<title>` (suffixed `— Pincel` unless it already contains "Pincel"), a meta description, a canonical URL on `https://pincel.amigo-labs.dev` for the page itself, `hreflang` alternates for `en`, `de` and `x-default` (pointing to the English page), Open Graph tags (title, description, absolute 1200×630 PNG image, URL, type `website`, site name `Pincel`, locale `en_US` / `de_DE` plus the other as alternate) and a Twitter `summary_large_image` card.
- [ ] AC-07 (always): The Home page carries JSON-LD of type `SoftwareApplication` named `Pincel`, with the released version, the page's language, category `DesignApplication`, operating systems `Web, Windows, macOS, Linux, iPadOS`, price `0` and the absolute URL of the Download page in the same language.
- [ ] AC-08 (always): `/sitemap.xml` lists the ten pages of AC-01 with absolute `https://pincel.amigo-labs.dev` URLs, each with `xhtml:link` alternates for `en`, `de` and `x-default`; `/robots.txt` allows crawling, disallows `/app` and names the sitemap URL.
- [ ] AC-09 (always): The Home page shows the two comparison tables `website-comparison` specifies (AC-01 – AC-03: "Pixel art" with Pincel, Aseprite, Pixelorama, Piskel and "Image editing" with Pincel, Paint.NET, Pinta, Photopea), and under the Pixel art table the footnote that Aseprite has 14 years of features.
- [ ] AC-10 (event): When the page runs with JavaScript on a Windows, macOS or x64 Linux desktop, then the download button offers that system's installer (`pincel-windows-x64-setup.exe`, `pincel-macos-universal.dmg`, `pincel-linux-x64.AppImage`) from the latest GitHub release, labelled "Download for {system}"; on phones, tablets (iPad included), ChromeOS, ARM Linux and without JavaScript it links to the Download page in the page's language.
- [ ] AC-11 (always): The Download page shows the released version, lists per system the files of the latest release — Windows setup `.exe` and `.msi`, macOS `.dmg`, Linux `.AppImage`, `.deb` and `.rpm` — as `https://github.com/amigo-labs/amigo-pincel/releases/latest/download/<file>` links, and shows the AppImage install commands `chmod +x pincel-linux-x64.AppImage` and `./pincel-linux-x64.AppImage --install`.
- [ ] AC-12 (option): Where the visitor switches the CRT effect off with the footer toggle, it stays off on later visits in the same browser; without a stored choice it is on.
- [ ] AC-13 (error, manual: served by the Cloudflare deployment, not by the SvelteKit build): If a requested path matches no page or file — under `/app/` too — then the styled English 404 page is served.

## Technical notes

Hints that drift — the code is the truth.

**Routing and build:** `src/routes/[[lang=locale]]/` with the matcher in `src/params.ts` (accepts only `de`); `+layout.ts` sets `prerender = true`, `trailingSlash = 'never'`; `vite.config.ts` lists `prerender.entries: ['*', '/de', '/de/features', '/de/about', '/de/embed', '/de/download']`, adapter-static with `strict: true`. `hooks.server.ts` fills `<html lang="%lang%">`. The version comes from `ui/package.json` as `__PINCEL_VERSION__`.

**Config** (`src/lib/config.ts`): `siteUrl = 'https://pincel.amigo-labs.dev'`, `appPath = '/app/'` (links carry `rel="external"` so the client router does a full load and the prerender crawler skips them), `releasesUrl = 'https://github.com/amigo-labs/amigo-pincel/releases/latest'`, and the stable release file names in `downloads`.

**Language:** `src/lib/i18n/{en,de}.ts` (`de` typed against `en`), `m()` reads the current page's copy, `localized()` / `unlocalized()` add and strip `/de`. `Rich.svelte` renders `` `code` ``, `[text](url)`, `**bold**` and line breaks, prefixing internal links.

**Comparison table** (`ComparisonTable.svelte`, English copy) — current state:

| Feature | Pincel | Aseprite | Piskel |
| ------- | ------ | -------- | ------ |
| Aseprite file format (read+write) | ✅ | ✅ native | ❌ |
| Tilemaps | ✅ | ✅ | ❌ |
| Slices (9-patch, pivot) | ✅ | ✅ | ❌ |
| Animation timeline + tags | ✅ | ✅ | ✅ basic |
| Tablet / pen support | ✅ | ✅ | ❌ |
| Runs in browser | ✅ | ❌ | ✅ |
| Layered image editing (photos, screenshots) | ✅ Image mode | ❌ pixel art only | ❌ |
| Native desktop app | ✅ Tauri | ✅ | ✅ via NW.js, ageing |
| Embeddable as a library | ✅ | ❌ | ❌ |
| Lua scripting / extensions | ❌ Phase 2 | ✅ | ❌ |
| Custom brushes | ❌ Phase 2 | ✅ | ✅ |
| Price | Free | Paid | Free |
| Open source | ✅ MIT/Apache | 🟡 source-available, EULA forbids redistribution | ✅ Apache 2.0 |
| Active development | ✅ | ✅ | 🟡 no tagged release since 0.15.0 (2018); master still gets fixes |

**Platform detection** (`src/lib/platform.ts`, `detectPlatform`): `null` for `userAgentData.mobile`, Android / iPhone / iPad / iPod, a Mac UA with `maxTouchPoints > 1`, ChromeOS and `aarch64` / `arm64` / `armv` Linux; Windows on ARM gets the x64 build. Runs in the browser only; on the Download page the button appears only once a system is detected.

**CRT toggle:** `src/lib/stores/crt.svelte.ts`, key `pincel:crt-enabled` (`'1'` / `'0'`) in `localStorage`; storage errors fall back to "on".

**Tests:** `@playwright/test` as a `website/` devDependency, pinned to the exact version `ui/` uses (1.64.1, Apache-2.0; owner-approved). Specs live under `website/e2e/` and run against `vite preview` of the static build; the CI `website` job installs Chromium (`pnpm exec playwright install --with-deps chromium`) and runs them after the build. One browser harness covers the HTML, SEO and JS-driven ACs; AC-10 sets the user agent / `userAgentData` per case.

**Hosting:** root `wrangler.toml` serves `website/build` with `not_found_handling = "404-page"` (`static/404.html`); `scripts/cf-build.sh` copies the editor build to `build/app/`. `static/_headers` sets immutable caching on `/_app/immutable/*` and `/app/assets/*`, revalidation on HTML, `/app/sw.js` and the manifest, and `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy: interest-cohort=()` on every path.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| JavaScript disabled | Download button links to the Download page; the Download page shows no detected-system button; all pages render (prerendered) |
| iPad presenting a macOS user agent | No installer offered; the button links to the Download page |
| Windows on ARM | Offered the x64 setup |
| Storage blocked in the browser | CRT effect on; toggling works for the visit, nothing is stored |
| Language switch on a deep page (`/de/features`) | Leads to `/features`, not to the home page |
| Unknown language prefix (`/fr/features`) | No such page — only `de` is a language prefix |
| Viewport narrower than 540 px | Header navigation is hidden; logo, language switch and Open Editor stay |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The released version shown on the site is `ui/package.json`'s version at build time | release-please bumps it with every release and the site rebuilds on push | A deploy between version bump and release publish shows a version whose files are not downloadable yet |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `website/e2e/pages.spec.ts`: "AC-01/AC-02: … is prerendered in …", "AC-01: the pages carry no trailing slash in their links", "AC-01: only `de` is a language prefix" | built |        |
| AC-02 | `website/e2e/pages.spec.ts`: "AC-01/AC-02: … is prerendered in …"; `lang` after a switch in "AC-03: … switches to … on the same page" | built |        |
| AC-03 | `website/e2e/pages.spec.ts`: "AC-03: … switches to … on the same page" (all ten pages) | built |        |
| AC-04 | `website/e2e/pages.spec.ts`: "AC-04: links on … stay in …", "AC-04: copy links on the home page follow its language" | built |        |
| AC-05 | `website/e2e/pages.spec.ts`: "AC-05: the header / hero / final call to action link loads /app/ as a new document" | built |        |
| AC-06 | `website/e2e/seo.spec.ts`: "AC-06: … carries title, description, canonical, hreflang, Open Graph and Twitter tags" (all ten pages), "AC-06: the Open Graph image is a 1200×630 PNG" | built |        |
| AC-07 | `website/e2e/seo.spec.ts`: "AC-07: the en / de home page carries SoftwareApplication JSON-LD" | built |        |
| AC-08 | `website/e2e/seo.spec.ts`: "AC-08: sitemap.xml lists every page with its language alternates", "AC-08: robots.txt allows crawling, keeps out /app and names the sitemap" | built |        |
| AC-09 | `website/e2e/comparison.spec.ts`: "website-comparison AC-01: two tables, Pixel art and Image editing, with their columns", "website-comparison AC-02: …", "website-comparison AC-03: …", "website-comparison AC-08: each table names its snapshot and its sources" (honesty footnote) | built | |
| AC-10 | `website/e2e/download.spec.ts`: "AC-10: the en / de hero offers …" and "AC-10: the Download page shows a system button only once one is detected" per visitor (Windows, Windows on ARM, macOS, x64 Linux, iPhone, Android, iPad with a macOS UA, ChromeOS, ARM Linux), "AC-10: the en / de hero links to the Download page" (without JavaScript) | built |        |
| AC-11 | `website/e2e/download.spec.ts`: "AC-11: the en / de Download page lists every release file" | built |        |
| AC-12 | `website/e2e/crt.spec.ts`: "AC-12: the CRT effect is on without a stored choice", "AC-12: switched off, the CRT effect stays off on later visits", "AC-12: with storage blocked the effect is on and toggles for the visit" | built |        |
| AC-13 | manual (Cloudflare deployment): `static/404.html` and `not_found_handling = "404-page"` are unchanged; sign-off open | open |        |

## Decisions

- 2026-10-29: Extracted from the code (reverse engineering). AC-09 pins today's comparison table; the planned `website-comparison` spec will revise it (and this AC) when it is built.
- 2026-10-29: Evidence for AC-01 – AC-12 comes from Playwright: `@playwright/test` becomes a `website/` devDependency pinned to `ui/`'s version (1.64.1), specs under `website/e2e/` run against `vite preview` of the static build in the CI `website` job; AC-13 stays manual (§15 row recorded with this batch) — the tool is already approved and used in this repo, and one browser harness covers the HTML, SEO and JS-driven ACs (owner accepted the recommendation).
- 2026-11-04: The language-switch e2e found a bug: after a client-side switch `<html lang>` kept the old language, because only `hooks.server.ts` set it for the prerendered page; the root layout now sets `document.documentElement.lang` from the route (red run: all ten AC-03 cases failed with `lang="en"` on `/de/…` and vice versa) — the smallest fix that makes AC-02 hold after the switch AC-03 describes.
- 2026-11-04: Apart from that bug this spec pins behavior that already ships, so its tests were green on their first run; they were checked against deliberate breakage instead (a weakened iPad / ARM-Linux rule in `platform.ts` failed six AC-10 cases) — a reverse-engineered spec has no red phase for existing behavior.
- 2026-11-04: The website e2e preview port is `PINCEL_SITE_E2E_PORT` (default 4174), not the editor's `PINCEL_E2E_PORT` / 4173 — with `reuseExistingServer` a shared port would let one suite silently test the other's build (AGENTS.md Gotchas).
- 2026-11-04: AC-05 stubs `/app/` with `page.route` — `vite preview` of the site has no editor (`scripts/cf-build.sh` copies it in at deploy), and the test proves a full document load by a window marker that a client-side navigation would keep.
- 2026-11-04: `website-comparison` AC-06 / AC-07 revised two cells of the pinned table (Technical notes, AC-09 test): Aseprite's license is 🟡 "source-available, EULA forbids redistribution", Piskel's activity 🟡 "no tagged release since 0.15.0 (2018); master still gets fixes" — as the 2026-10-29 Decision foresaw.
- 2026-11-05: AC-09 now defers to `website-comparison`, which replaced the single 14-row table with the Pixel art and Image editing tables, as the 2026-10-29 Decision foresaw; the 14-row list under Technical notes stays as history.
