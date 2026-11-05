import { expect, test, type Page } from '@playwright/test';
import { ALL_PAGES, COPY, heading, hydrated, localized } from './helpers';

// Pages, languages and links (docs/specs/website.md AC-01 – AC-05).

test.describe('without JavaScript', () => {
  test.use({ javaScriptEnabled: false });

  for (const { lang, path, url } of ALL_PAGES) {
    test(`AC-01/AC-02: ${url} is prerendered in ${lang}`, async ({ page }) => {
      const response = await page.goto(url);
      expect(response?.status()).toBe(200);
      expect(response?.headers()['content-type']).toContain('text/html');
      // Without JavaScript the heading can only come from the static HTML.
      expect(await page.locator('h1').innerText()).toBe(heading(path, lang));
      await expect(page.locator('html')).toHaveAttribute('lang', lang);
      const nav = page.getByRole('navigation', { name: COPY[lang].nav.primary });
      await expect(nav.getByRole('link')).toHaveText([
        COPY[lang].nav.home,
        COPY[lang].nav.features,
        COPY[lang].nav.download,
        COPY[lang].nav.devs,
        COPY[lang].nav.about,
      ]);
      await expect(page.getByRole('contentinfo')).toContainText(COPY[lang].footer.crt);
    });
  }

  test('AC-01: the pages carry no trailing slash in their links', async ({ page }) => {
    await page.goto('/');
    const hrefs = await internalHrefs(page);
    for (const href of hrefs) {
      if (href === '/' || href.startsWith('/app/')) continue;
      expect(href, href).not.toMatch(/\/(#.*)?$/);
    }
  });

  test('AC-01: only `de` is a language prefix', async ({ page }) => {
    const response = await page.goto('/fr/features');
    expect(response?.status()).toBe(404);
  });
});

/** Every same-origin link target on the page, as written in its `href`. */
async function internalHrefs(page: Page, scope = 'body'): Promise<string[]> {
  return page
    .locator(`${scope} a[href^="/"]`)
    .evaluateAll((links) => links.map((a) => a.getAttribute('href') ?? ''));
}

test.describe('language switch', () => {
  for (const { lang, path, url } of ALL_PAGES) {
    const other = lang === 'en' ? 'de' : 'en';
    test(`AC-03: ${url} switches to ${other} on the same page`, async ({ page }) => {
      await page.goto(url);
      await hydrated(page);
      const switcher = page.getByRole('group', { name: COPY[lang].nav.language });
      await expect(switcher.getByRole('link', { name: lang.toUpperCase() })).toHaveAttribute(
        'aria-current',
        'true',
      );
      await expect(switcher.getByRole('link', { name: other.toUpperCase() })).not.toHaveAttribute(
        'aria-current',
        /.*/,
      );
      await switcher.getByRole('link', { name: other.toUpperCase() }).click();
      await expect(page).toHaveURL(localized(path, other));
      // AC-02 holds after a client-side switch too, not only on a fresh load.
      await expect(page.locator('html')).toHaveAttribute('lang', other);
      await expect.poll(() => page.locator('h1').innerText()).toBe(heading(path, other));
      const switched = page.getByRole('group', { name: COPY[other].nav.language });
      await expect(switched.getByRole('link', { name: other.toUpperCase() })).toHaveAttribute(
        'aria-current',
        'true',
      );
    });
  }
});

test.describe('internal links', () => {
  for (const { lang, url } of ALL_PAGES) {
    test(`AC-04: links on ${url} stay in ${lang}`, async ({ page }) => {
      await page.goto(url);
      const hrefs = await internalHrefs(page);
      expect(hrefs.length).toBeGreaterThan(5);
      // The language switch is the one place that links across languages.
      const switchHrefs = await internalHrefs(page, '[role="group"]');
      for (const href of hrefs) {
        if (href.startsWith('/app/') || switchHrefs.includes(href)) continue;
        if (lang === 'de') expect(href, href).toMatch(/^\/de(\/|#|$)/);
        else expect(href, href).not.toMatch(/^\/de(\/|#|$)/);
      }
    });
  }

  test('AC-04: copy links on the home page follow its language', async ({ page }) => {
    // Links written in the copy markup (Rich.svelte) get the prefix too.
    await page.goto('/de');
    await expect(page.getByRole('link', { name: 'herunterladen', exact: true })).toHaveAttribute(
      'href',
      '/de/download',
    );
    await expect(page.getByRole('link', { name: 'einzubetten', exact: true })).toHaveAttribute(
      'href',
      '/de/embed',
    );
    await page.goto('/');
    await expect(page.getByRole('link', { name: 'download it', exact: true })).toHaveAttribute(
      'href',
      '/download',
    );
  });
});

test.describe('Open Editor', () => {
  const targets = [
    {
      name: 'header',
      pick: (p: Page) => p.getByRole('banner').getByRole('link', { name: 'Open Editor' }),
    },
    {
      name: 'hero',
      pick: (p: Page) => p.locator('.hero').getByRole('link', { name: 'Open Editor' }),
    },
    {
      name: 'final call to action',
      pick: (p: Page) => p.locator('main').getByRole('link', { name: 'Open Editor' }).last(),
    },
  ];
  for (const { name, pick } of targets) {
    test(`AC-05: the ${name} link loads /app/ as a new document`, async ({ page }) => {
      // The editor is not part of this build (scripts/cf-build.sh copies it in).
      await page.route('**/app/', (route) =>
        route.fulfill({ contentType: 'text/html', body: '<title>editor</title>' }),
      );
      await page.goto('/');
      await hydrated(page);
      await page.evaluate(() => {
        (window as Window & { marker?: boolean }).marker = true;
      });
      await pick(page).click();
      await expect(page).toHaveURL('/app/');
      await expect(page).toHaveTitle('editor');
      // A client-side navigation would keep the old window object.
      expect(await page.evaluate(() => (window as Window & { marker?: boolean }).marker)).toBe(
        undefined,
      );
    });
  }
});
