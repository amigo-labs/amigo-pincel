import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, menu, newDocument, waitForEditor } from './helpers';
import {
  commandCalls,
  emit,
  holdCommands,
  installTauriMock,
  launchDesktop,
  releaseCommands,
  reply,
  setMock,
} from './tauri-mock';

// View ▸ Image generation… (docs/specs/comfyui-connect.md). The backend
// (src-tauri/src/comfyui/, tested in Rust against fake servers) cannot run
// here: the mocked Tauri bridge answers its commands and emits its
// `comfyui:status` events, and these specs check what the dialog shows.

const MENU = 'Image Generation…';

function settings(over: Record<string, unknown> = {}): Record<string, unknown> {
  return { server: 'automatic', existingDir: null, customUrl: '', managedDir: null, ...over };
}

function connected(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    kind: 'connected',
    server: {
      url: 'http://127.0.0.1:8188',
      version: '0.3.40',
      gpu: { name: 'cuda:0 NVIDIA GeForce RTX 4070 : cudaMallocAsync', vramTotal: 12878086144 },
      checkpoints: ['sdxl.safetensors', 'sd15.safetensors'],
      loras: ['pixel.safetensors'],
      missingNodes: [],
      started: false,
      ...over,
    },
  };
}

/** Starts the desktop app with the backend answers in place, opens a
 *  document of `mode` and View ▸ Image generation…. */
async function openDialog(
  page: Page,
  answers: { settings?: unknown; connect?: unknown[] } = {},
  mode: 'pixel' | 'image' = 'pixel',
): Promise<Locator> {
  await launchDesktop(page);
  await reply(page, 'comfyui_settings', answers.settings ?? settings());
  await reply(page, 'comfyui_connect', ...(answers.connect ?? [{ kind: 'notFound' }]));
  await newDocument(page, mode);
  await menu(page, 'View', MENU);
  return page.getByRole('dialog', { name: 'Image generation' });
}

function status(dialog: Locator): Locator {
  return dialog.getByRole('status');
}

test.describe('browser', () => {
  test('the PWA has no View ▸ Image generation… (AC-01)', async ({ page }) => {
    for (const mode of ['pixel', 'image'] as const) {
      await createDocument(page, mode);
      await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
      const view = page.getByRole('menu', { name: 'View', exact: true });
      await expect(view.getByRole('menuitem', { name: 'Zoom In' })).toBeVisible();
      await expect(view.getByRole('menuitem', { name: MENU })).toHaveCount(0);
      await page.keyboard.press('Escape');
    }
  });
});

test.describe('desktop', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
  });

  test('both editors offer View ▸ Image generation… (AC-01)', async ({ page }) => {
    const dialog = await openDialog(page, {}, 'image');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Close' }).click();
    await expect(dialog).toBeHidden();
  });

  test('the dialog shows the stored choice and stores a new one (AC-02)', async ({ page }) => {
    const dialog = await openDialog(page, {
      settings: settings({ server: 'custom', customUrl: 'http://gpu-box:8188' }),
      connect: [{ kind: 'unreachable', url: 'http://gpu-box:8188' }],
    });
    await expect(dialog.getByRole('radio', { name: 'Custom URL' })).toBeChecked();
    await expect(dialog.getByRole('textbox', { name: 'Server URL' })).toHaveValue('http://gpu-box:8188');
    await reply(page, 'comfyui_set_server', settings());
    await dialog.getByRole('radio', { name: 'Automatic' }).check();
    await expect.poll(() => commandCalls(page, 'comfyui_set_server')).toEqual([
      { cmd: 'comfyui_set_server', args: { server: 'automatic' } },
    ]);
    await expect(dialog.getByRole('radio', { name: 'Automatic' })).toBeChecked();
    // A fresh install: Automatic, and no managed choice before comfyui-install.
    await expect(dialog.getByRole('radio')).toHaveCount(3);
  });

  test('opening the dialog looks for a server; none found offers the ways on (AC-03)', async ({
    page,
  }) => {
    const dialog = await openDialog(page, { connect: [{ kind: 'notFound' }] });
    await expect(status(dialog)).toContainText('No ComfyUI server found');
    expect(await commandCalls(page, 'comfyui_connect')).toHaveLength(1);
    await expect(dialog.getByRole('button', { name: 'Install ComfyUI…' })).toBeVisible();
    // "Use an existing installation…" switches the choice and asks for the folder.
    await reply(page, 'comfyui_set_server', settings({ server: 'existing' }));
    await reply(page, 'comfyui_choose_folder', {
      kind: 'accepted',
      settings: settings({ server: 'existing', existingDir: 'D:/ComfyUI_windows_portable' }),
    });
    await setMock(page, { openPick: 'D:/ComfyUI_windows_portable' });
    await dialog.getByRole('button', { name: 'Use an existing installation…' }).click();
    await expect.poll(() => commandCalls(page, 'comfyui_choose_folder')).toEqual([
      { cmd: 'comfyui_choose_folder', args: { path: 'D:/ComfyUI_windows_portable' } },
    ]);
    await expect(dialog.getByRole('radio', { name: 'Existing installation' })).toBeChecked();
    await expect(dialog.getByText('D:/ComfyUI_windows_portable')).toBeVisible();
  });

  test('a connected server shows version, GPU and models (AC-04)', async ({ page }) => {
    const dialog = await openDialog(page, { connect: [connected()] });
    await expect(status(dialog)).toContainText('ComfyUI 0.3.40');
    await expect(status(dialog)).toContainText(
      'cuda:0 NVIDIA GeForce RTX 4070 : cudaMallocAsync · 12.0 GB VRAM',
    );
    await expect(dialog.getByRole('list', { name: 'Checkpoints' }).getByRole('listitem')).toHaveText([
      'sdxl.safetensors',
      'sd15.safetensors',
    ]);
    await expect(dialog.getByRole('list', { name: 'LoRAs' }).getByRole('listitem')).toHaveText([
      'pixel.safetensors',
    ]);
  });

  test('a picked folder is accepted or refused with the reason (AC-05)', async ({ page }) => {
    const dialog = await openDialog(page, {
      settings: settings({ server: 'existing' }),
      connect: [{ kind: 'notConfigured' }],
    });
    await expect(status(dialog)).toContainText('No folder chosen');
    const choose = dialog.getByRole('button', { name: 'Choose folder…' });
    await setMock(page, { openPick: 'C:/Users/me/AppData/Local/Programs/ComfyUI' });
    await reply(
      page,
      'comfyui_choose_folder',
      { kind: 'refused', problem: 'desktop' },
      { kind: 'refused', problem: 'noPython' },
      { kind: 'refused', problem: 'noComfyUi' },
      {
        kind: 'accepted',
        settings: settings({ server: 'existing', existingDir: 'D:/ComfyUI' }),
      },
    );
    await choose.click();
    await expect(dialog.getByRole('alert')).toHaveText(
      'Start ComfyUI Desktop — Pincel finds it automatically',
    );
    await choose.click();
    await expect(dialog.getByRole('alert')).toContainText('no Python environment');
    await choose.click();
    await expect(dialog.getByRole('alert')).toHaveText('No ComfyUI here: main.py is missing');
    await reply(page, 'comfyui_connect', connected({ started: true }));
    await choose.click();
    await expect(dialog.getByRole('alert')).toBeHidden();
    await expect(dialog.getByText('D:/ComfyUI')).toBeVisible();
    await expect(status(dialog)).toContainText('ComfyUI 0.3.40');
    // A cancelled picker changes nothing.
    await setMock(page, { openPick: null });
    await choose.click();
    expect(await commandCalls(page, 'comfyui_choose_folder')).toHaveLength(4);
  });

  test('a start shows "Starting ComfyUI…" until the server answers (AC-06)', async ({ page }) => {
    await launchDesktop(page);
    await reply(page, 'comfyui_settings', settings({ server: 'existing', existingDir: 'D:/ComfyUI' }));
    await reply(page, 'comfyui_connect', connected({ started: true }));
    await holdCommands(page, ['comfyui_connect']);
    await newDocument(page, 'pixel');
    await menu(page, 'View', MENU);
    const dialog = page.getByRole('dialog', { name: 'Image generation' });
    await expect(status(dialog)).toContainText('Connecting…');
    await emit(page, 'comfyui:status', { kind: 'starting' });
    await expect(status(dialog)).toHaveText('Starting ComfyUI…');
    await releaseCommands(page);
    await expect(status(dialog)).toContainText('ComfyUI 0.3.40');
  });

  test('a server that stops shows "ComfyUI stopped" with its output (AC-07)', async ({ page }) => {
    const log = Array.from({ length: 50 }, (_, i) => `line ${i + 10}`);
    const dialog = await openDialog(page, { connect: [{ kind: 'stopped', log }] });
    await expect(status(dialog)).toContainText('ComfyUI stopped');
    const output = dialog.getByLabel('Last output');
    await expect(output).toContainText('line 10');
    await expect(output).toContainText('line 59');
    // Later: a connected server whose process ends.
    await emit(page, 'comfyui:status', connected({ started: true }));
    await expect(status(dialog)).toContainText('ComfyUI 0.3.40');
    await emit(page, 'comfyui:status', { kind: 'stopped', log: ['CUDA out of memory'] });
    await expect(status(dialog)).toContainText('ComfyUI stopped');
    await expect(dialog.getByLabel('Last output')).toHaveText('CUDA out of memory');
  });

  test('Test connection reports each outcome; remote hosts get a notice (AC-08)', async ({
    page,
  }) => {
    const dialog = await openDialog(page, {
      settings: settings({ server: 'custom' }),
      connect: [{ kind: 'notConfigured' }],
    });
    const field = dialog.getByRole('textbox', { name: 'Server URL' });
    const test_ = dialog.getByRole('button', { name: 'Test connection' });
    const notice = dialog.getByText('Images and prompts are sent to this server');
    await field.fill('http://127.0.0.1:8188');
    await expect(notice).toBeHidden();
    await field.fill('https://gpu.example.com/comfy');
    await expect(notice).toBeVisible();
    await field.fill('http://[::1]:8188');
    await expect(notice).toBeHidden();
    await field.fill('http://192.168.1.20:8188');
    await expect(notice).toBeVisible();

    await reply(
      page,
      'comfyui_test_url',
      { kind: 'invalidUrl' },
      { kind: 'unreachable', url: 'http://192.168.1.20:8188' },
      { kind: 'notComfy', url: 'http://192.168.1.20:8188' },
      { kind: 'refused', status: 401 },
      connected({ url: 'http://192.168.1.20:8188' }),
    );
    for (const text of [
      'Not a valid URL',
      'Not reachable',
      'Not a ComfyUI server',
      'The server refused access (401)',
      'ComfyUI 0.3.40',
    ]) {
      await test_.click();
      await expect(status(dialog)).toContainText(text);
    }
    const calls = await commandCalls(page, 'comfyui_test_url');
    expect(calls.map((c) => c.args)).toEqual(
      Array.from({ length: 5 }, () => ({ url: 'http://192.168.1.20:8188' })),
    );
    // Enter in the field tests too.
    await field.press('Enter');
    await expect.poll(async () => (await commandCalls(page, 'comfyui_test_url')).length).toBe(6);
  });

  test('a server lacking core node types is too old (AC-11)', async ({ page }) => {
    const dialog = await openDialog(page, {
      connect: [connected({ missingNodes: ['VAEEncodeForInpaint', 'SetLatentNoiseMask'] })],
    });
    await expect(status(dialog)).toContainText('This ComfyUI is too old — update it');
    await expect(status(dialog)).toContainText(
      'Missing node types: VAEEncodeForInpaint, SetLatentNoiseMask',
    );
  });

  test('a server without checkpoints has no model installed (AC-12)', async ({ page }) => {
    const dialog = await openDialog(page, { connect: [connected({ checkpoints: [] })] });
    await expect(status(dialog)).toContainText('No model installed');
    await expect(dialog.getByRole('list', { name: 'Checkpoints' })).toHaveCount(0);
  });

  test('a vanished installation folder asks for another (AC-13)', async ({ page }) => {
    const dialog = await openDialog(page, {
      settings: settings({ server: 'existing', existingDir: 'E:/ComfyUI' }),
      connect: [{ kind: 'installationNotFound', path: 'E:/ComfyUI' }],
    });
    await expect(status(dialog)).toContainText('Installation not found at E:/ComfyUI');
    await reply(page, 'comfyui_choose_folder', {
      kind: 'accepted',
      settings: settings({ server: 'existing', existingDir: 'D:/ComfyUI' }),
    });
    await reply(page, 'comfyui_connect', connected({ started: true }));
    await setMock(page, { openPick: 'D:/ComfyUI' });
    await status(dialog).getByRole('button', { name: 'Choose another folder…' }).click();
    await expect(status(dialog)).toContainText('ComfyUI 0.3.40');
  });

  test('the dialog keeps the keyboard from the editor', async ({ page }) => {
    const dialog = await openDialog(page);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
  });
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('dialog, status texts and errors are translated (AC-14)', async ({ page }) => {
    await installTauriMock(page);
    await page.goto('./');
    await page.getByRole('button', { name: 'Verstanden' }).click();
    await reply(page, 'comfyui_settings', settings());
    await reply(page, 'comfyui_connect', { kind: 'notFound' });
    await page.getByRole('button', { name: 'Neues Pixel-Sprite' }).click();
    await page.getByRole('button', { name: 'Erstellen' }).click();
    await waitForEditor(page, 'pixel');
    await menu(page, 'Ansicht', 'Bildgenerierung…');
    const dialog = page.getByRole('dialog', { name: 'Bildgenerierung' });
    await expect(status(dialog)).toContainText('Kein ComfyUI-Server gefunden');
    await expect(dialog.getByRole('button', { name: 'Vorhandene Installation verwenden…' })).toBeVisible();
    await expect(dialog.getByRole('radio', { name: 'Automatisch' })).toBeChecked();
    await emit(page, 'comfyui:status', { kind: 'starting' });
    await expect(status(dialog)).toHaveText('ComfyUI wird gestartet…');
    await emit(page, 'comfyui:status', { kind: 'stopped', log: [] });
    await expect(status(dialog)).toHaveText('ComfyUI wurde beendet');
    await emit(page, 'comfyui:status', connected({ checkpoints: [], missingNodes: ['KSampler'] }));
    await expect(status(dialog)).toContainText('Dieses ComfyUI ist zu alt – aktualisiere es');
    await expect(status(dialog)).toContainText('Fehlende Knotentypen: KSampler');
    await expect(status(dialog)).toContainText('Kein Modell installiert');
    await emit(page, 'comfyui:status', { kind: 'refused', status: 403 });
    await expect(status(dialog)).toHaveText('Der Server hat den Zugriff verweigert (403)');
    await emit(page, 'comfyui:status', { kind: 'installationNotFound', path: 'E:/ComfyUI' });
    await expect(status(dialog)).toContainText('Keine Installation unter E:/ComfyUI gefunden');
    await expect(dialog.getByRole('button', { name: 'Schließen' })).toBeVisible();
  });
});
