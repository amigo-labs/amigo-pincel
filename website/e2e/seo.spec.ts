import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { ALL_PAGES, COPY, PAGES, SITE, localized } from './helpers';

// SEO head, JSON-LD, sitemap and robots (docs/specs/website.md AC-06 – AC-08).

const version = (
  JSON.parse(readFileSync(new URL('../../ui/package.json', import.meta.url), 'utf8')) as {
    version: string;
  }
).version;

/** The `seoTitle` / `seoDescription` copy of an unprefixed page. */
function seo(path: (typeof PAGES)[number], lang: 'en' | 'de') {
  const c = COPY[lang];
  const key = (
    {
      '/': 'home',
      '/features': 'features',
      '/download': 'download',
      '/embed': 'embed',
      '/about': 'about',
    } as const
  )[path];
  return { title: c[key].seoTitle, description: c[key].seoDescription };
}

test.describe('head', () => {
  // The head must be in the prerendered HTML, which crawlers read.
  test.use({ javaScriptEnabled: false });

  for (const { lang, path, url } of ALL_PAGES) {
    test(`AC-06: ${url} carries title, description, canonical, hreflang, Open Graph and Twitter tags`, async ({
      page,
    }) => {
      await page.goto(url);
      const { title, description } = seo(path, lang);
      const fullTitle = title.includes('Pincel') ? title : `${title} — Pincel`;
      const canonical = `${SITE}${url}`;
      const meta = (attr: string, value: string) =>
        page.locator(`head meta[${attr}="${value}"]`).first();
      const other = lang === 'en' ? 'de_DE' : 'en_US';

      await expect(page).toHaveTitle(fullTitle);
      await expect(meta('name', 'description')).toHaveAttribute('content', description);
      await expect(page.locator('head link[rel="canonical"]')).toHaveAttribute('href', canonical);
      for (const [hreflang, l] of [
        ['en', 'en'],
        ['de', 'de'],
        ['x-default', 'en'],
      ] as const) {
        await expect(
          page.locator(`head link[rel="alternate"][hreflang="${hreflang}"]`),
        ).toHaveAttribute('href', `${SITE}${localized(path, l)}`);
      }
      await expect(meta('property', 'og:title')).toHaveAttribute('content', fullTitle);
      await expect(meta('property', 'og:description')).toHaveAttribute('content', description);
      await expect(meta('property', 'og:image')).toHaveAttribute(
        'content',
        `${SITE}/og/default.png`,
      );
      await expect(meta('property', 'og:image:type')).toHaveAttribute('content', 'image/png');
      await expect(meta('property', 'og:image:width')).toHaveAttribute('content', '1200');
      await expect(meta('property', 'og:image:height')).toHaveAttribute('content', '630');
      await expect(meta('property', 'og:url')).toHaveAttribute('content', canonical);
      await expect(meta('property', 'og:type')).toHaveAttribute('content', 'website');
      await expect(meta('property', 'og:site_name')).toHaveAttribute('content', 'Pincel');
      await expect(meta('property', 'og:locale')).toHaveAttribute(
        'content',
        COPY[lang].meta.locale,
      );
      await expect(page.locator('head meta[property="og:locale:alternate"]')).toHaveAttribute(
        'content',
        other,
      );
      await expect(meta('name', 'twitter:card')).toHaveAttribute('content', 'summary_large_image');
      await expect(meta('name', 'twitter:title')).toHaveAttribute('content', fullTitle);
    });
  }

  test('AC-06: the Open Graph image is a 1200×630 PNG', async ({ request }) => {
    const response = await request.get('/og/default.png');
    expect(response.status()).toBe(200);
    const png = await response.body();
    expect(png.subarray(1, 4).toString('ascii')).toBe('PNG');
    // IHDR: width and height as big-endian u32 at bytes 16 and 20.
    expect(png.readUInt32BE(16)).toBe(1200);
    expect(png.readUInt32BE(20)).toBe(630);
  });

  for (const lang of ['en', 'de'] as const) {
    test(`AC-07: the ${lang} home page carries SoftwareApplication JSON-LD`, async ({ page }) => {
      await page.goto(localized('/', lang));
      const scripts = page.locator('script[type="application/ld+json"]');
      await expect(scripts).toHaveCount(1);
      const data = JSON.parse((await scripts.textContent()) ?? '') as Record<string, unknown>;
      expect(data).toMatchObject({
        '@context': 'https://schema.org',
        '@type': 'SoftwareApplication',
        name: 'Pincel',
        softwareVersion: version,
        inLanguage: lang,
        applicationCategory: 'DesignApplication',
        operatingSystem: 'Web, Windows, macOS, Linux, iPadOS',
        offers: { '@type': 'Offer', price: '0' },
        downloadUrl: `${SITE}${localized('/download', lang)}`,
      });
    });
  }
});

test('AC-08: sitemap.xml lists every page with its language alternates', async ({ request }) => {
  const response = await request.get('/sitemap.xml');
  expect(response.status()).toBe(200);
  const xml = await response.text();
  const entries = [...xml.matchAll(/<url>([\s\S]*?)<\/url>/g)].map((m) => m[1] ?? '');
  const locs = entries.map((e) => /<loc>([^<]+)<\/loc>/.exec(e)?.[1]);
  expect(locs.sort()).toEqual(ALL_PAGES.map(({ url }) => `${SITE}${url}`).sort());
  for (const entry of entries) {
    const loc = /<loc>([^<]+)<\/loc>/.exec(entry)?.[1] ?? '';
    const path = loc.slice(SITE.length).replace(/^\/de(?=\/|$)/, '') || '/';
    const links = [
      ...entry.matchAll(/<xhtml:link rel="alternate" hreflang="([^"]+)" href="([^"]+)"/g),
    ].map((m) => [m[1], m[2]]);
    expect(links, loc).toEqual([
      ['en', `${SITE}${localized(path, 'en')}`],
      ['de', `${SITE}${localized(path, 'de')}`],
      ['x-default', `${SITE}${localized(path, 'en')}`],
    ]);
  }
});

test('AC-08: robots.txt allows crawling, keeps out /app and names the sitemap', async ({
  request,
}) => {
  const response = await request.get('/robots.txt');
  expect(response.status()).toBe(200);
  const lines = (await response.text()).split('\n').map((l) => l.trim());
  expect(lines).toContain('User-agent: *');
  expect(lines).toContain('Allow: /');
  expect(lines).toContain('Disallow: /app');
  expect(lines).toContain(`Sitemap: ${SITE}/sitemap.xml`);
});
