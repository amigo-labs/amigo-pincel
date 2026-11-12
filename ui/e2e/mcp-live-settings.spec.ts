import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, storedPref, switchLanguage, waitForEditor } from './helpers';
import { emit, installTauriMock, type MockWindow } from './tauri-mock';

// View ▸ AI agents… and the "AI connected" indicator
// (docs/specs/mcp-live-desktop.md), against the mocked Tauri bridge, whose
// `mcp_*` commands stand in for the endpoint in src-tauri/src/mcp.

async function commands(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as MockWindow).__mock.mcpCommands);
}

function dialog(page: Page, title = 'AI Agents'): Locator {
  return page.getByRole('dialog', { name: title });
}

async function viewMenu(page: Page, view = 'View'): Promise<void> {
  await page.getByRole('menubar').getByRole('menuitem', { name: view, exact: true }).click();
}

/** The desktop app (past the first-launch note) with a 16×16 document. */
async function start(page: Page, mode: 'pixel' | 'image' = 'pixel'): Promise<void> {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

async function openAgents(page: Page): Promise<Locator> {
  await viewMenu(page);
  await page.getByRole('menuitem', { name: 'AI Agents…' }).click();
  const d = dialog(page);
  await expect(d).toBeVisible();
  return d;
}

test.describe('desktop app', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
  });

  test('AC-01 a fresh install starts no endpoint', async ({ page }) => {
    await start(page);
    await page.waitForTimeout(300);
    expect((await commands(page)).filter((c) => c.startsWith('mcp_enable'))).toEqual([]);
    await expect(page.getByText('AI connected')).toHaveCount(0);
  });

  test('AC-02 AC-03 AC-04 the switch starts the endpoint with a new token, keeps it on, and stops it', async ({
    page,
  }) => {
    await start(page);
    const d = await openAgents(page);
    const toggle = d.getByRole('checkbox', { name: 'Allow AI agents to control Pincel' });
    await expect(toggle).not.toBeChecked();
    await toggle.check();
    await expect(d.getByLabel('Client command')).toHaveValue(
      'claude mcp add --transport http pincel http://127.0.0.1:39217/mcp --header "Authorization: Bearer token-1"',
    );
    await expect(d.getByLabel('Token')).toHaveValue('token-1');
    expect(await commands(page)).toContain('mcp_enable:true');
    await expect.poll(() => storedPref(page, 'aiAgents')).toBe(true);
    // A restart starts the endpoint with the stored token (AC-03).
    await page.reload();
    await page.getByRole('button', { name: 'Got it' }).click();
    await expect.poll(() => commands(page)).toContain('mcp_enable:false');
    expect(await commands(page)).not.toContain('mcp_enable:true');
    // Switching it off stops it (AC-04).
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    await page.getByRole('form', { name: 'New pixel sprite' }).getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'pixel');
    const again = await openAgents(page);
    await again.getByRole('checkbox', { name: 'Allow AI agents to control Pincel' }).uncheck();
    await expect(again.getByLabel('Client command')).toHaveCount(0);
    expect(await commands(page)).toContain('mcp_disable');
    await expect.poll(() => storedPref(page, 'aiAgents')).toBe(false);
  });

  test('AC-20 Regenerate token shows the new token and command', async ({ page }) => {
    await start(page, 'image');
    const d = await openAgents(page);
    await d.getByRole('checkbox', { name: 'Allow AI agents to control Pincel' }).check();
    await expect(d.getByLabel('Token')).toHaveValue('token-1');
    await d.getByRole('button', { name: 'Regenerate token' }).click();
    await expect(d.getByLabel('Token')).toHaveValue('token-2');
    await expect(d.getByLabel('Client command')).toHaveValue(/Bearer token-2"$/);
    expect(await commands(page)).toContain('mcp_regenerate_token');
  });

  test('AC-21 a changed port is announced with the updated command', async ({ page }) => {
    await start(page);
    await page.evaluate(() => {
      const m = (window as unknown as MockWindow).__mock.mcp;
      m.port = 50123;
      m.portChanged = true;
    });
    const d = await openAgents(page);
    await d.getByRole('checkbox', { name: 'Allow AI agents to control Pincel' }).check();
    await expect(d.getByText('Port changed — update your client config')).toBeVisible();
    await expect(d.getByLabel('Client command')).toHaveValue(/127\.0\.0\.1:50123\/mcp/);
  });

  test('AC-12 the status bar shows AI connected while a session is open and disconnects it', async ({
    page,
  }) => {
    await start(page);
    await emit(page, 'mcp-sessions', 1);
    const indicator = page.getByRole('status').filter({ hasText: 'AI connected' });
    await expect(indicator).toBeVisible();
    await indicator.getByRole('button', { name: 'Disconnect' }).click();
    await expect(indicator).toHaveCount(0);
    expect(await commands(page)).toContain('mcp_disconnect');
    await emit(page, 'mcp-sessions', 2);
    await expect(indicator).toBeVisible();
    await emit(page, 'mcp-sessions', 0);
    await expect(indicator).toHaveCount(0);
  });

  test('AC-14 the dialog and the indicator are German in the German UI', async ({ page }) => {
    await start(page);
    await switchLanguage(page, 'Deutsch');
    await viewMenu(page, 'Ansicht');
    await page.getByRole('menuitem', { name: 'KI-Agenten…' }).click();
    const d = dialog(page, 'KI-Agenten');
    await expect(d).toBeVisible();
    await d.getByRole('checkbox', { name: 'KI-Agenten erlauben, Pincel zu steuern' }).check();
    await expect(d.getByLabel('Befehl für den Client')).toBeVisible();
    await expect(d.getByRole('button', { name: 'Token neu erzeugen' })).toBeVisible();
    await d.getByRole('button', { name: 'Schließen' }).click();
    await emit(page, 'mcp-sessions', 1);
    const indicator = page.getByRole('status').filter({ hasText: 'KI verbunden' });
    await expect(indicator.getByRole('button', { name: 'Trennen' })).toBeVisible();
  });
});

test('AC-13 the web app shows neither the setting nor the indicator', async ({ page }) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  await viewMenu(page);
  await expect(page.getByRole('menuitem', { name: 'Zoom In' })).toBeVisible();
  await expect(page.getByRole('menuitem', { name: 'AI Agents…' })).toHaveCount(0);
  await expect(page.getByText('AI connected')).toHaveCount(0);
});
