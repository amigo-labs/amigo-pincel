import { expect, test } from '@playwright/test';
import { hydrated } from './helpers';

// The footer's CRT toggle (docs/specs/website.md AC-12).

test('AC-12: the CRT effect is on without a stored choice', async ({ page }) => {
  await page.goto('/');
  await hydrated(page);
  await expect(page.getByRole('button', { name: /CRT effect/ })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.locator('.root')).toHaveClass(/crt-enabled/);
});

test('AC-12: switched off, the CRT effect stays off on later visits', async ({ page, context }) => {
  await page.goto('/');
  await hydrated(page);
  const toggle = page.getByRole('button', { name: /CRT effect/ });
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await expect(toggle).toContainText('off');
  await expect(page.locator('.root')).not.toHaveClass(/crt-enabled/);

  // A later visit in the same browser, on another page.
  const later = await context.newPage();
  await later.goto('/features');
  await hydrated(later);
  await expect(later.getByRole('button', { name: /CRT effect/ })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  await expect(later.locator('.root')).not.toHaveClass(/crt-enabled/);
});

test('AC-12: with storage blocked the effect is on and toggles for the visit', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'localStorage', {
      configurable: true,
      get: () => {
        throw new DOMException('blocked', 'SecurityError');
      },
    });
  });
  await page.goto('/');
  await hydrated(page);
  const toggle = page.getByRole('button', { name: /CRT effect/ });
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
  await toggle.click();
  await expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await page.reload();
  await hydrated(page);
  await expect(toggle).toHaveAttribute('aria-pressed', 'true');
});
