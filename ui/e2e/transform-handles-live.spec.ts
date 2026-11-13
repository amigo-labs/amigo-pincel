import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { clearClipboard, copiedImage, menu, waitForEditor } from './helpers';
import { openHistory, rowNames } from './history-helpers';
import { emit, installTauriMock, type MockWindow } from './tauri-mock';
import { box, centre, dragBy, paintScene, pivot, scene } from './transform-helpers';

// What a live selection transform does to the rest of the editor
// (docs/specs/image-transform-handles.md): Export and Copy see it as
// applied without committing it (AC-18), an MCP call commits it first
// (AC-08). The core side is in crates/pincel-image-core/tests/live_transform.rs.

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

/** Runs `action` and returns the bytes of the download it triggers. */
async function download(page: Page, action: () => Promise<void>): Promise<Buffer> {
  const [d] = await Promise.all([page.waitForEvent('download'), action()]);
  return readFile(await d.path());
}

/** A turned transform: live, with pixels nothing else has produced. */
async function turn(page: Page): Promise<void> {
  const at = await centre(pivot(page));
  await dragBy(page, [at[0] + 60, at[1]], 0, 50, 'right');
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
}

test('Export and Copy while live hold the applied pixels and neither commit nor add a step (AC-18)', async ({
  page,
}) => {
  // No File System Access pickers: every export is a download.
  await page.addInitScript(() => {
    const w = window as { showSaveFilePicker?: unknown; showOpenFilePicker?: unknown };
    delete w.showSaveFilePicker;
    delete w.showOpenFilePicker;
  });
  await scene(page);
  await openHistory(page);
  await turn(page);
  const rows = await rowNames(page);

  const livePng = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  expect(await rowNames(page)).toEqual(rows);
  await clearClipboard(page);
  await page.keyboard.press('ControlOrMeta+c');
  const liveCopy = await copiedImage(page);
  await expect(box(page)).toHaveAttribute('data-transform-box', 'live');
  expect(await rowNames(page)).toEqual(rows);

  // Committed, the document holds what both already showed.
  await page.keyboard.press('Enter');
  await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
  const committedPng = await download(page, () => menu(page, 'File', 'Export', 'PNG'));
  expect(committedPng.equals(livePng)).toBe(true);
  await clearClipboard(page);
  await page.keyboard.press('ControlOrMeta+c');
  expect(await copiedImage(page)).toEqual(liveCopy);
});

test.describe('in the desktop app', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
  });

  /** Forwards the MCP tool call `tool` to the UI and waits for its reply. */
  async function mcpCall(
    page: Page,
    id: number,
    tool: string,
    args: Record<string, unknown> = {},
  ): Promise<{ ok: boolean; error?: string }> {
    await emit(page, 'mcp-call', { id, tool, args, chat: null });
    await page.waitForFunction((i) => i in (window as unknown as MockWindow).__mock.mcpReplies, id);
    return page.evaluate(
      (i) => (window as unknown as MockWindow).__mock.mcpReplies[i],
      id,
    ) as Promise<{ ok: boolean; error?: string }>;
  }

  test('an MCP call commits the live transform first (AC-08)', async ({ page }) => {
    await page.goto('./');
    await page.getByRole('button', { name: 'Got it' }).click();
    await page.getByRole('button', { name: 'New image' }).click();
    const card = page.getByRole('form', { name: 'New image' });
    for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('64');
    await card.getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'image');
    await paintScene(page);
    await openHistory(page);
    const rows = await rowNames(page);
    // Two drags: two live steps.
    await turn(page);
    const at = await centre(pivot(page));
    await dragBy(page, [at[0] + 10, at[1] + 10], 20, 0);
    await expect
      .poll(() => rowNames(page))
      .toEqual([...rows, 'Move Selected Pixels', 'Move Selected Pixels']);

    // Even a call that only reads commits it: one step for the transform.
    const r = await mcpCall(page, 1, 'doc.info');
    expect(r.ok, r.error).toBe(true);
    await expect(box(page)).toHaveAttribute('data-transform-box', 'idle');
    await expect.poll(() => rowNames(page)).toEqual([...rows, 'Move Selected Pixels']);
  });
});
