import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { COPY } from './helpers';

// Download button and Download page (docs/specs/website.md AC-10, AC-11).

const LATEST = 'https://github.com/amigo-labs/amigo-pincel/releases/latest/download';

const version = (
  JSON.parse(readFileSync(new URL('../../ui/package.json', import.meta.url), 'utf8')) as {
    version: string;
  }
).version;

/** A browser to emulate: its user agent, `navigator.userAgentData` and touch points. */
interface Visitor {
  name: string;
  userAgent: string;
  uad?: { mobile: boolean; platform: string };
  touch?: number;
  /** The installer offered, or `null` for the Download-page link. */
  file: string | null;
  system?: 'windows' | 'macos' | 'linux';
}

const CHROME = 'AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';
const visitors: Visitor[] = [
  {
    name: 'Windows',
    userAgent: `Mozilla/5.0 (Windows NT 10.0; Win64; x64) ${CHROME}`,
    uad: { mobile: false, platform: 'Windows' },
    file: 'pincel-windows-x64-setup.exe',
    system: 'windows',
  },
  {
    name: 'Windows on ARM',
    userAgent: `Mozilla/5.0 (Windows NT 10.0; ARM64) ${CHROME}`,
    uad: { mobile: false, platform: 'Windows' },
    file: 'pincel-windows-x64-setup.exe',
    system: 'windows',
  },
  {
    name: 'macOS Safari',
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15',
    file: 'pincel-macos-universal.dmg',
    system: 'macos',
  },
  {
    name: 'x64 Linux',
    userAgent: `Mozilla/5.0 (X11; Linux x86_64) ${CHROME}`,
    uad: { mobile: false, platform: 'Linux' },
    file: 'pincel-linux-x64.AppImage',
    system: 'linux',
  },
  {
    name: 'iPhone',
    userAgent:
      'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1',
    touch: 5,
    file: null,
  },
  {
    name: 'Android phone',
    userAgent: `Mozilla/5.0 (Linux; Android 15; Pixel 9) ${CHROME}`,
    uad: { mobile: true, platform: 'Android' },
    touch: 5,
    file: null,
  },
  {
    name: 'iPad with a macOS user agent',
    userAgent:
      'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15',
    touch: 5,
    file: null,
  },
  {
    name: 'ChromeOS',
    userAgent: `Mozilla/5.0 (X11; CrOS x86_64 16181.61.0) ${CHROME}`,
    uad: { mobile: false, platform: 'Chrome OS' },
    file: null,
  },
  {
    name: 'ARM Linux',
    userAgent: `Mozilla/5.0 (X11; Linux aarch64) ${CHROME}`,
    uad: { mobile: false, platform: 'Linux' },
    file: null,
  },
];

/** Presents `visitor`'s `userAgentData` and touch points to the page's scripts. */
async function emulate(page: Page, visitor: Visitor): Promise<void> {
  await page.addInitScript(
    ({ uad, touch }) => {
      Object.defineProperty(Navigator.prototype, 'userAgentData', {
        configurable: true,
        get: () => uad,
      });
      Object.defineProperty(Navigator.prototype, 'maxTouchPoints', {
        configurable: true,
        get: () => touch,
      });
    },
    { uad: visitor.uad, touch: visitor.touch ?? 0 },
  );
}

for (const visitor of visitors) {
  test.describe(visitor.name, () => {
    test.use({ userAgent: visitor.userAgent });

    for (const lang of ['en', 'de'] as const) {
      test(`AC-10: the ${lang} hero offers ${visitor.file ?? 'the Download page'}`, async ({
        page,
      }) => {
        await emulate(page, visitor);
        await page.goto(lang === 'en' ? '/' : '/de');
        const label = visitor.system
          ? COPY[lang].download.for[visitor.system]
          : COPY[lang].nav.download;
        const button = page.locator('.hero').getByRole('link', { name: label, exact: true });
        await expect(button).toHaveAttribute(
          'href',
          visitor.file ? `${LATEST}/${visitor.file}` : lang === 'en' ? '/download' : '/de/download',
        );
      });
    }

    test('AC-10: the Download page shows a system button only once one is detected', async ({
      page,
    }) => {
      await emulate(page, visitor);
      await page.goto('/download');
      const ctas = page.locator('.ctas');
      await expect(
        ctas.getByRole('link', { name: 'All releases and release notes' }),
      ).toBeVisible();
      if (visitor.system && visitor.file) {
        await expect(
          ctas.getByRole('link', { name: COPY.en.download.for[visitor.system] }),
        ).toHaveAttribute('href', `${LATEST}/${visitor.file}`);
      }
      // Detection runs after hydration; the count settles once it has.
      await page.waitForLoadState('networkidle');
      await expect(ctas.getByRole('link')).toHaveCount(visitor.file ? 2 : 1);
    });
  });
}

test.describe('without JavaScript', () => {
  test.use({ javaScriptEnabled: false });

  for (const lang of ['en', 'de'] as const) {
    test(`AC-10: the ${lang} hero links to the Download page`, async ({ page }) => {
      await page.goto(lang === 'en' ? '/' : '/de');
      await expect(
        page.locator('.hero').getByRole('link', { name: COPY[lang].nav.download, exact: true }),
      ).toHaveAttribute('href', lang === 'en' ? '/download' : '/de/download');
      await page.goto(lang === 'en' ? '/download' : '/de/download');
      await expect(page.locator('.ctas').getByRole('link')).toHaveCount(1);
    });

    test(`AC-11: the ${lang} Download page lists every release file`, async ({ page }) => {
      await page.goto(lang === 'en' ? '/download' : '/de/download');
      await expect(page.locator('.lede')).toContainText(version);
      const files = {
        windows: ['pincel-windows-x64-setup.exe', 'pincel-windows-x64.msi'],
        macos: ['pincel-macos-universal.dmg'],
        linux: ['pincel-linux-x64.AppImage', 'pincel-linux-x64.deb', 'pincel-linux-x64.rpm'],
      };
      for (const [system, names] of Object.entries(files)) {
        const hrefs = await page
          .locator(`#${system} .files a`)
          .evaluateAll((links) => links.map((a) => a.getAttribute('href')));
        expect(hrefs, system).toEqual(names.map((n) => `${LATEST}/${n}`));
      }
      const code = await page.locator('#linux pre').innerText();
      expect(code.split('\n')).toEqual([
        'chmod +x pincel-linux-x64.AppImage',
        './pincel-linux-x64.AppImage --install',
      ]);
    });
  }
});
