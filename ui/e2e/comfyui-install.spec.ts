import { expect, test, type Locator, type Page } from '@playwright/test';
import { menu, newDocument, waitForEditor } from './helpers';
import { commandCalls, emit, installTauriMock, launchDesktop, reply, setMock } from './tauri-mock';

// The managed ComfyUI install (docs/specs/comfyui-install.md). The backend
// (src-tauri/src/comfyui/managed.rs, tested in Rust against a local file
// server and a fake uv) cannot run here: the mocked Tauri bridge answers its
// commands and emits its `comfyui:install` / `comfyui:status` events, and
// these specs check what the wizard and the dialog show and send.

const GIB = 1024 ** 3;

function settings(over: Record<string, unknown> = {}): Record<string, unknown> {
  return { server: 'automatic', existingDir: null, customUrl: '', managedDir: null, ...over };
}

function pkg(
  id: string,
  name: string,
  size: number,
  license: string,
  minVram: number,
  minMemory: number,
  kind = 'checkpoint',
  requires: string | null = null,
): Record<string, unknown> {
  return {
    id,
    name,
    kind,
    file: `${id}.safetensors`,
    size: size * GIB,
    license,
    licenseUrl: `https://example.com/${id}`,
    minVram: minVram * GIB,
    minMemory: minMemory * GIB,
    requires,
  };
}

const CATALOG = [
  pkg('sdxl', 'SDXL base 1.0', 6.5, 'CreativeML Open RAIL++-M', 6, 16),
  pkg('sd15', 'SD 1.5', 2, 'CreativeML Open RAIL-M', 4, 8),
  pkg('flux-schnell', 'Flux.1 schnell', 16, 'Apache-2.0', 12, 32),
  pkg('pixel-art-xl', 'Pixel Art XL (LoRA)', 0.25, 'CreativeML Open RAIL-M', 6, 16, 'lora', 'sdxl'),
];

function nvidia(vramGib: number, slow = false): Record<string, unknown> {
  return {
    os: 'windows',
    device: { name: 'NVIDIA GeForce RTX 4070', memory: vramGib * GIB, backend: 'cuda' },
    unsupported: null,
    slow,
  };
}

function info(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    hardware: nvidia(12),
    catalog: CATALOG,
    baseSize: 7 * GIB,
    reserve: 5 * GIB,
    accepted: [],
    managed: { kind: 'none' },
    job: null,
    generating: false,
    ...over,
  };
}

function complete(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    kind: 'complete',
    dir: 'D:/AI/Pincel ComfyUI',
    comfyui: '0.43.0',
    models: ['sdxl', 'pixel-art-xl'],
    updateAvailable: false,
    ...over,
  };
}

function connected(): Record<string, unknown> {
  return {
    kind: 'connected',
    server: {
      url: 'http://127.0.0.1:50123',
      version: '0.43.0',
      gpu: { name: 'cuda:0 NVIDIA GeForce RTX 4070 : cudaMallocAsync', vramTotal: 12 * GIB },
      checkpoints: ['v1-5-pruned-emaonly-fp16.safetensors'],
      loras: [],
      missingNodes: [],
      started: true,
    },
  };
}

interface Answers {
  info?: unknown;
  settings?: unknown[];
  connect?: unknown[];
}

/** The desktop app with View ▸ Image generation… open. */
async function openDialog(page: Page, answers: Answers = {}): Promise<Locator> {
  await launchDesktop(page);
  await reply(page, 'comfyui_install_info', answers.info ?? info());
  await reply(page, 'comfyui_settings', ...(answers.settings ?? [settings()]));
  await reply(page, 'comfyui_connect', ...(answers.connect ?? [{ kind: 'notFound' }]));
  await newDocument(page, 'pixel');
  await menu(page, 'View', 'Image Generation…');
  const dialog = page.getByRole('dialog', { name: 'Image generation' });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** Opens the wizard from "No ComfyUI server found". */
async function openWizard(page: Page, answers: Answers = {}): Promise<Locator> {
  const dialog = await openDialog(page, answers);
  await dialog.getByRole('button', { name: 'Install ComfyUI…' }).click();
  const wizard = page.getByRole('dialog', { name: 'Install ComfyUI' });
  await expect(wizard).toBeVisible();
  return wizard;
}

/** Picks `path` as the location; the backend answers `check`. */
async function pickLocation(
  page: Page,
  wizard: Locator,
  path: string,
  check: Record<string, unknown> = {},
): Promise<void> {
  await reply(page, 'comfyui_check_location', {
    target: `${path}/Pincel ComfyUI`,
    free: 40 * GIB,
    writable: true,
    existing: 'none',
    ...check,
  });
  await setMock(page, { openPick: path });
  await wizard.getByRole('button', { name: 'Choose location…' }).click();
  await expect(wizard).toContainText(`Installs into ${path}/Pincel ComfyUI`);
}

test.describe('desktop', () => {
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
  });

  test('the wizard shows the GPU and whether it is supported (AC-01)', async ({ page }) => {
    const wizard = await openWizard(page);
    const hw = wizard.getByRole('region', { name: 'Hardware' });
    await expect(hw).toContainText('Windows · NVIDIA GeForce RTX 4070 · 12 GB VRAM');
    await expect(hw).toContainText('Supported');
    await expect(wizard.getByRole('button', { name: 'Install' })).toBeVisible();
  });

  test('an unsupported system gets the reason and the other ways in (AC-01)', async ({ page }) => {
    const wizard = await openWizard(page, {
      info: info({
        hardware: {
          os: 'windows',
          device: null,
          unsupported: { kind: 'otherGpu', name: 'AMD Radeon RX 7900 XTX' },
          slow: false,
        },
      }),
    });
    await expect(wizard).toContainText(
      'Not supported: AMD Radeon RX 7900 XTX is not an NVIDIA GPU; Pincel installs ComfyUI for NVIDIA GPUs and Apple Silicon Macs',
    );
    await expect(wizard.getByRole('button', { name: 'Install', exact: true })).toHaveCount(0);
    await expect(wizard.getByRole('button', { name: 'Use an existing installation…' })).toBeVisible();
    await reply(page, 'comfyui_set_server', settings({ server: 'custom' }));
    await wizard.getByRole('button', { name: 'Custom URL' }).click();
    const dialog = page.getByRole('dialog', { name: 'Image generation' });
    await expect(dialog.getByRole('radio', { name: 'Custom URL' })).toBeChecked();
    expect(await commandCalls(page, 'comfyui_set_server')).toEqual([
      { cmd: 'comfyui_set_server', args: { server: 'custom' } },
    ]);
  });

  test('an Intel Mac and a missing NVIDIA driver say why (AC-01)', async ({ page }) => {
    const wizard = await openWizard(page, {
      info: info({
        hardware: { os: 'macos', device: null, unsupported: { kind: 'intelMac' }, slow: false },
      }),
    });
    await expect(wizard).toContainText('Macs with an Intel processor are not supported');
    await expect(wizard.getByRole('button', { name: 'Custom URL' })).toBeVisible();
  });

  test('a small GPU gets the warning and may still install (AC-02)', async ({ page }) => {
    const wizard = await openWizard(page, { info: info({ hardware: nvidia(4, true) }) });
    await expect(wizard.getByRole('note')).toHaveText('Generation will be slow or fail on this hardware');
    await pickLocation(page, wizard, 'D:/AI');
    await expect(wizard.getByRole('button', { name: 'Install', exact: true })).toBeEnabled();
  });

  test('a Mac shows its chip and memory (AC-01, AC-02)', async ({ page }) => {
    const wizard = await openWizard(page, {
      info: info({
        hardware: {
          os: 'macos',
          device: { name: 'Apple M1', memory: 8 * GIB, backend: 'mps' },
          unsupported: null,
          slow: true,
        },
      }),
    });
    await expect(wizard.getByRole('region', { name: 'Hardware' })).toContainText('macOS · Apple M1 · 8 GB memory');
    await expect(wizard.getByRole('note')).toBeVisible();
    // Mac packages show their memory minimum (AC-04).
    await expect(wizard).toContainText('Needs 16 GB memory');
  });

  test('the location gets a subfolder, free and required space; short or read-only blocks Install (AC-03)', async ({
    page,
  }) => {
    const wizard = await openWizard(page);
    const install = wizard.getByRole('button', { name: 'Install', exact: true });
    await expect(install).toBeDisabled();
    await pickLocation(page, wizard, 'D:/AI');
    expect(await commandCalls(page, 'comfyui_check_location')).toEqual([
      { cmd: 'comfyui_check_location', args: { path: 'D:/AI' } },
    ]);
    // 7 GB base + 2 GB SD 1.5 + 5 GB reserve.
    await expect(wizard).toContainText('Free: 40.0 GB · Required: 14.0 GB');
    await expect(install).toBeEnabled();
    // Selecting Flux.1 schnell (16 GB) needs 30 GB.
    await wizard.getByRole('checkbox', { name: 'Flux.1 schnell' }).check();
    await expect(wizard).toContainText('Required: 30.0 GB');

    await pickLocation(page, wizard, 'E:/Small', { free: 20 * GIB });
    await expect(wizard).toContainText('Not enough free space');
    await expect(install).toBeDisabled();

    await pickLocation(page, wizard, 'C:/Program Files', { writable: false });
    await expect(wizard).toContainText('Pincel cannot write to this folder');
    await expect(install).toBeDisabled();
  });

  test('packages show size, license and minimum; the selection rules hold (AC-04)', async ({ page }) => {
    const wizard = await openWizard(page, { info: info({ hardware: nvidia(8) }) });
    const packages = wizard.getByRole('group', { name: 'Model packages' });
    await expect(packages).toContainText('SDXL base 1.0');
    await expect(packages).toContainText('6.5 GB · License: CreativeML Open RAIL++-M · Needs 6 GB VRAM');
    await expect(packages).toContainText('2.0 GB · License: CreativeML Open RAIL-M · Needs 4 GB VRAM');
    await expect(packages).toContainText('Pixel Art XL (LoRA)');
    await expect(packages).toContainText('256 MB');
    // Flux.1 schnell needs 12 GB: highlighted on this 8 GB GPU.
    await expect(packages.getByText('Needs 12 GB VRAM')).toHaveAttribute('data-below', 'true');
    await expect(packages.getByText('Needs 6 GB VRAM').first()).toHaveAttribute('data-below', 'false');

    const sd15 = wizard.getByRole('checkbox', { name: 'SD 1.5' });
    const sdxl = wizard.getByRole('checkbox', { name: 'SDXL base 1.0' });
    const lora = wizard.getByRole('checkbox', { name: 'Pixel Art XL (LoRA)' });
    // The only checkpoint cannot be deselected.
    await expect(sd15).toBeChecked();
    await expect(sd15).toBeDisabled();
    // The LoRA brings SDXL base; then SD 1.5 may go.
    await lora.check();
    await expect(sdxl).toBeChecked();
    await expect(sd15).toBeEnabled();
    await sd15.uncheck();
    await expect(sd15).not.toBeChecked();
    // Deselecting SDXL base would leave no checkpoint: refused.
    await expect(sdxl).toBeDisabled();
    await sd15.check();
    await sdxl.uncheck();
    await expect(lora).not.toBeChecked();
  });

  test('Install asks for each new license and names ComfyUI and PyTorch (AC-05)', async ({ page }) => {
    const wizard = await openWizard(page, { info: info({ accepted: ['sd15'] }) });
    await wizard.getByRole('checkbox', { name: 'SDXL base 1.0' }).check();
    await pickLocation(page, wizard, 'D:/AI');
    await wizard.getByRole('button', { name: 'Install', exact: true }).click();
    const licenses = wizard.getByRole('region', { name: 'Licenses' });
    await expect(licenses).toContainText('SDXL base 1.0: CreativeML Open RAIL++-M');
    await expect(licenses).not.toContainText('SD 1.5');
    await expect(wizard.getByRole('note')).toHaveText(
      'Pincel downloads and installs ComfyUI (GPL-3.0) and PyTorch (BSD-3-Clause) as separate programs. Pincel talks to ComfyUI over HTTP and contains none of their code.',
    );
    const go = wizard.getByRole('button', { name: 'Install', exact: true });
    await expect(go).toBeDisabled();
    expect(await commandCalls(page, 'comfyui_install')).toEqual([]);
    await reply(page, 'comfyui_accept_licenses', ['sd15', 'sdxl']);
    await wizard.getByRole('checkbox', { name: 'I accept the license of SDXL base 1.0' }).check();
    await go.click();
    await expect.poll(() => commandCalls(page, 'comfyui_install')).toEqual([
      { cmd: 'comfyui_install', args: { location: 'D:/AI', packages: ['sdxl', 'sd15'] } },
    ]);
    expect(await commandCalls(page, 'comfyui_accept_licenses')).toEqual([
      { cmd: 'comfyui_accept_licenses', args: { ids: ['sdxl'] } },
    ]);
  });

  /** The wizard on its progress page after Install. */
  async function installing(page: Page): Promise<Locator> {
    const wizard = await openWizard(page, { info: info({ accepted: ['sd15'] }) });
    await pickLocation(page, wizard, 'D:/AI');
    await wizard.getByRole('button', { name: 'Install', exact: true }).click();
    await wizard.getByRole('button', { name: 'Install', exact: true }).click();
    await expect(wizard.getByRole('status')).toHaveText('Preparing…');
    return wizard;
  }

  function progress(step: string, more: Record<string, unknown> = {}): Record<string, unknown> {
    return {
      kind: 'progress',
      job: 'install',
      progress: { step, package: null, done: null, total: null, line: null, ...more },
    };
  }

  test('installing shows the step, bytes or the latest line, and Cancel (AC-06)', async ({ page }) => {
    const wizard = await installing(page);
    const status = wizard.getByRole('status');
    await emit(page, 'comfyui:install', progress('python', { done: 5 * 1024 ** 2, total: 15 * 1024 ** 2 }));
    await expect(status).toContainText('Preparing Python');
    await expect(status).toContainText('5 MB of 15 MB');
    await emit(page, 'comfyui:install', progress('python', { line: 'Using CPython 3.12.16' }));
    await expect(status).toContainText('Using CPython 3.12.16');
    await emit(page, 'comfyui:install', progress('comfyui', { done: 6 * 1024 ** 2, total: 13 * 1024 ** 2 }));
    await expect(status).toContainText('Downloading ComfyUI');
    await emit(page, 'comfyui:install', progress('packages', { line: 'Downloading torch (1.9GiB)' }));
    await expect(status).toContainText('Installing PyTorch and dependencies');
    await expect(status).toContainText('Downloading torch (1.9GiB)');
    await emit(page, 'comfyui:install', progress('model', { package: 'sd15', done: GIB, total: 2 * GIB }));
    await expect(status).toContainText('Downloading SD 1.5');
    await expect(status).toContainText('1.0 GB of 2.0 GB');
    await wizard.getByRole('button', { name: 'Cancel' }).click();
    await expect.poll(async () => (await commandCalls(page, 'comfyui_install_cancel')).length).toBe(1);
    await emit(page, 'comfyui:install', { kind: 'failed', job: 'install', error: { kind: 'cancelled' } });
    await expect(wizard.getByRole('alert')).toHaveText('Installation cancelled');
  });

  test('a corrupted download stops with its name and Retry resumes (AC-07, AC-09)', async ({ page }) => {
    const wizard = await installing(page);
    await emit(page, 'comfyui:install', {
      kind: 'failed',
      job: 'install',
      error: { kind: 'corrupted', file: 'SD 1.5' },
    });
    await expect(wizard.getByRole('alert')).toHaveText('Download corrupted: SD 1.5');
    await wizard.getByRole('button', { name: 'Retry' }).click();
    await expect.poll(async () => (await commandCalls(page, 'comfyui_install_resume')).length).toBe(1);
    await expect(wizard.getByRole('status')).toBeVisible();
  });

  test('each failing step has its message; setup failures show their output (AC-08)', async ({ page }) => {
    const wizard = await installing(page);
    const alert = wizard.getByRole('alert');
    const cases: [Record<string, unknown>, string][] = [
      [{ kind: 'gone', file: 'ComfyUI' }, 'Download no longer available — update Pincel'],
      [{ kind: 'offline' }, 'No internet connection'],
      [{ kind: 'diskFull' }, 'Disk full'],
    ];
    for (const [error, text] of cases) {
      await emit(page, 'comfyui:install', { kind: 'failed', job: 'install', error });
      await expect(alert).toHaveText(text);
    }
    const log = Array.from({ length: 50 }, (_, i) => `uv line ${i}`);
    await emit(page, 'comfyui:install', { kind: 'failed', job: 'install', error: { kind: 'setupFailed', log } });
    await expect(alert).toContainText('Setting up Python or its packages failed');
    await expect(wizard.getByLabel('Last output')).toHaveText(log.join('\n'));
    await expect(wizard.getByRole('button', { name: 'Retry' })).toBeVisible();
  });

  test('a finished install shows the started managed server (AC-10)', async ({ page }) => {
    const wizard = await installing(page);
    await reply(page, 'comfyui_settings', settings({ server: 'managed', managedDir: 'D:/AI/Pincel ComfyUI' }));
    await reply(page, 'comfyui_install_info', info({ managed: complete({ models: ['sd15'] }) }));
    await emit(page, 'comfyui:install', { kind: 'done', job: 'install', status: connected() });
    await expect(wizard).toBeHidden();
    const dialog = page.getByRole('dialog', { name: 'Image generation' });
    await expect(dialog.getByRole('status')).toContainText('Connected to http://127.0.0.1:50123');
    await expect(dialog.getByRole('list', { name: 'Checkpoints' })).toContainText(
      'v1-5-pruned-emaonly-fp16.safetensors',
    );
    await expect(dialog.getByRole('radio', { name: 'Managed installation' })).toBeChecked();
    await expect(dialog.getByRole('region', { name: 'Managed installation' })).toContainText(
      'ComfyUI 0.43.0 in D:/AI/Pincel ComfyUI',
    );
  });

  test('a managed server that fails to start offers Retry start and the driver hint (AC-10, edge case)', async ({
    page,
  }) => {
    const wizard = await installing(page);
    await reply(page, 'comfyui_settings', settings({ server: 'managed', managedDir: 'D:/AI/Pincel ComfyUI' }));
    await emit(page, 'comfyui:install', {
      kind: 'done',
      job: 'install',
      status: {
        kind: 'stopped',
        log: ['RuntimeError: The NVIDIA driver on your system is too old (found version 12020).'],
      },
    });
    await expect(wizard).toBeHidden();
    const dialog = page.getByRole('dialog', { name: 'Image generation' });
    const status = dialog.getByRole('status');
    await expect(status).toContainText('ComfyUI stopped');
    await expect(status).toContainText('Update your NVIDIA driver');
    await reply(page, 'comfyui_connect', connected());
    const before = (await commandCalls(page, 'comfyui_connect')).length;
    await dialog.getByRole('button', { name: 'Retry start' }).click();
    await expect.poll(async () => (await commandCalls(page, 'comfyui_connect')).length).toBe(before + 1);
    await expect(status).toContainText('Connected to');
  });

  test('packages are added and removed later; SDXL takes the LoRA after asking (AC-11)', async ({ page }) => {
    const dialog = await openDialog(page, {
      info: info({ managed: complete(), accepted: ['sdxl', 'pixel-art-xl'] }),
      settings: [settings({ server: 'managed', managedDir: 'D:/AI/Pincel ComfyUI' })],
      connect: [connected()],
    });
    await dialog.getByRole('button', { name: 'Packages…' }).click();
    const wizard = page.getByRole('dialog', { name: 'Model packages' });
    const sdxl = wizard.getByRole('checkbox', { name: 'SDXL base 1.0' });
    const lora = wizard.getByRole('checkbox', { name: 'Pixel Art XL (LoRA)' });
    await expect(sdxl).toBeChecked();
    await expect(lora).toBeChecked();
    // The last checkpoint stays.
    await expect(sdxl).toBeDisabled();
    const apply = wizard.getByRole('button', { name: 'Apply' });
    await expect(apply).toBeDisabled();
    await wizard.getByRole('checkbox', { name: 'SD 1.5' }).check();
    // The box stays checked until the removal is confirmed.
    await sdxl.click();
    const ask = wizard.getByRole('alertdialog', { name: 'Remove package' });
    await expect(ask).toHaveText(/Removing SDXL base 1\.0 also removes Pixel Art XL \(LoRA\)/);
    await ask.getByRole('button', { name: 'Keep' }).click();
    await expect(sdxl).toBeChecked();
    // The box stays checked until the removal is confirmed.
    await sdxl.click();
    await wizard.getByRole('button', { name: 'Remove both' }).click();
    await expect(sdxl).not.toBeChecked();
    await expect(lora).not.toBeChecked();
    await apply.click();
    // SD 1.5's license is new.
    await reply(page, 'comfyui_accept_licenses', ['sdxl', 'pixel-art-xl', 'sd15']);
    await wizard.getByRole('checkbox', { name: 'I accept the license of SD 1.5' }).check();
    await wizard.getByRole('button', { name: 'Apply' }).click();
    await expect.poll(() => commandCalls(page, 'comfyui_change_packages')).toEqual([
      { cmd: 'comfyui_change_packages', args: { add: ['sd15'], remove: ['sdxl', 'pixel-art-xl'] } },
    ]);
    await emit(page, 'comfyui:install', {
      kind: 'progress',
      job: 'packages',
      progress: { step: 'model', package: 'sd15', done: GIB, total: 2 * GIB, line: null },
    });
    await expect(wizard.getByRole('status')).toContainText('Downloading SD 1.5');
  });

  test('a newer pinned ComfyUI is offered as Update ComfyUI (AC-12)', async ({ page }) => {
    const dialog = await openDialog(page, {
      info: info({ managed: complete({ updateAvailable: true }) }),
      settings: [settings({ server: 'managed', managedDir: 'D:/AI/Pincel ComfyUI' })],
      connect: [connected()],
    });
    const managed = dialog.getByRole('region', { name: 'Managed installation' });
    await expect(managed).toContainText('This Pincel version comes with a newer ComfyUI');
    await managed.getByRole('button', { name: 'Update ComfyUI' }).click();
    await expect.poll(async () => (await commandCalls(page, 'comfyui_update')).length).toBe(1);
    const wizard = page.getByRole('dialog', { name: 'Install ComfyUI' });
    await emit(page, 'comfyui:install', {
      kind: 'failed',
      job: 'update',
      error: { kind: 'updateNotStarted', log: ['Traceback (most recent call last):'] },
    });
    await expect(wizard.getByRole('alert')).toContainText(
      'The updated ComfyUI did not start; the previous version was restored',
    );
    await expect(wizard.getByLabel('Last output')).toHaveText('Traceback (most recent call last):');
  });

  test('Uninstall asks, deletes and falls back to Automatic (AC-13)', async ({ page }) => {
    const dialog = await openDialog(page, {
      info: info({ managed: complete() }),
      settings: [settings({ server: 'managed', managedDir: 'D:/AI/Pincel ComfyUI' }), settings()],
      connect: [connected(), { kind: 'notFound' }],
    });
    await dialog.getByRole('button', { name: 'Uninstall…' }).click();
    const ask = dialog.getByRole('alertdialog', { name: 'Uninstall ComfyUI' });
    await expect(ask).toContainText('Delete D:/AI/Pincel ComfyUI with all models?');
    expect(await commandCalls(page, 'comfyui_uninstall')).toEqual([]);
    await reply(page, 'comfyui_install_info', info());
    await ask.getByRole('button', { name: 'Uninstall' }).click();
    await expect.poll(async () => (await commandCalls(page, 'comfyui_uninstall')).length).toBe(1);
    await expect(dialog.getByRole('radio', { name: 'Automatic' })).toBeChecked();
    await expect(dialog.getByRole('radio', { name: 'Managed installation' })).toHaveCount(0);
    await expect(dialog.getByRole('status')).toContainText('No ComfyUI server found');
  });

  test('a running generation disables package changes, update and uninstall (AC-14)', async ({ page }) => {
    const dialog = await openDialog(page, {
      info: info({ managed: complete({ updateAvailable: true }), generating: true }),
      settings: [settings({ server: 'managed', managedDir: 'D:/AI/Pincel ComfyUI' })],
      connect: [connected()],
    });
    const managed = dialog.getByRole('region', { name: 'Managed installation' });
    for (const name of ['Packages…', 'Update ComfyUI', 'Uninstall…']) {
      await expect(managed.getByRole('button', { name })).toBeDisabled();
      await expect(managed.getByRole('button', { name })).toHaveAttribute('title', 'A generation is running');
    }
  });

  test('a vanished managed folder offers Install again… and another server (AC-15)', async ({ page }) => {
    const dialog = await openDialog(page, {
      info: info({ managed: { kind: 'missing', dir: 'D:/AI/Pincel ComfyUI' } }),
      settings: [settings({ server: 'managed', managedDir: 'D:/AI/Pincel ComfyUI' })],
      connect: [{ kind: 'installationNotFound', path: 'D:/AI/Pincel ComfyUI' }],
    });
    const status = dialog.getByRole('status');
    await expect(status).toContainText('Installation not found at D:/AI/Pincel ComfyUI');
    await reply(page, 'comfyui_check_location', {
      target: 'D:/AI/Pincel ComfyUI',
      free: 40 * GIB,
      writable: true,
      existing: 'none',
    });
    await status.getByRole('button', { name: 'Install again…' }).click();
    const wizard = page.getByRole('dialog', { name: 'Install ComfyUI' });
    await expect(wizard).toContainText('Installs into D:/AI/Pincel ComfyUI');
    expect(await commandCalls(page, 'comfyui_check_location')).toEqual([
      { cmd: 'comfyui_check_location', args: { path: 'D:/AI' } },
    ]);
    await wizard.getByRole('button', { name: 'Cancel' }).click();
    await reply(page, 'comfyui_set_server', settings({ managedDir: 'D:/AI/Pincel ComfyUI' }));
    await dialog.getByRole('button', { name: 'Choose another server' }).click();
    await expect.poll(() => commandCalls(page, 'comfyui_set_server')).toEqual([
      { cmd: 'comfyui_set_server', args: { server: 'automatic' } },
    ]);
  });

  test('an interrupted install offers Resume and Delete; a complete folder can be used (edge cases)', async ({
    page,
  }) => {
    const dialog = await openDialog(page, {
      info: info({ managed: { kind: 'incomplete', dir: 'D:/AI/Pincel ComfyUI', packages: ['sd15'] } }),
    });
    const managed = dialog.getByRole('region', { name: 'Managed installation' });
    await expect(managed).toContainText('Installation incomplete');
    await managed.getByRole('button', { name: 'Resume' }).click();
    await expect.poll(async () => (await commandCalls(page, 'comfyui_install_resume')).length).toBe(1);
    const wizard = page.getByRole('dialog', { name: 'Install ComfyUI' });
    await expect(wizard.getByRole('status')).toBeVisible();
    await wizard.getByRole('button', { name: 'Cancel' }).click();
    await emit(page, 'comfyui:install', { kind: 'failed', job: 'install', error: { kind: 'cancelled' } });
    await wizard.getByRole('button', { name: 'Close' }).click();
    await managed.getByRole('button', { name: 'Delete' }).click();
    await expect.poll(async () => (await commandCalls(page, 'comfyui_uninstall')).length).toBe(1);

    // A location that already holds a complete installation.
    await reply(page, 'comfyui_install_info', info());
    await page.keyboard.press('Escape');
    await menu(page, 'View', 'Image Generation…');
    await dialog.getByRole('button', { name: 'Install ComfyUI…' }).click();
    await pickLocation(page, wizard, 'E:/Models', { existing: 'complete' });
    await expect(wizard).toContainText('This folder already has a complete installation');
    await expect(wizard.getByRole('button', { name: 'Install', exact: true })).toBeDisabled();
    await reply(page, 'comfyui_use_installation', connected());
    await reply(page, 'comfyui_settings', settings({ server: 'managed', managedDir: 'E:/Models/Pincel ComfyUI' }));
    await wizard.getByRole('button', { name: 'Use this installation' }).click();
    await expect(wizard).toBeHidden();
    expect(await commandCalls(page, 'comfyui_use_installation')).toEqual([
      { cmd: 'comfyui_use_installation', args: { location: 'E:/Models' } },
    ]);
    await expect(dialog.getByRole('status')).toContainText('Connected to');
  });

  test('reopening the dialog during an install shows its progress', async ({ page }) => {
    await launchDesktop(page);
    await reply(
      page,
      'comfyui_install_info',
      info({
        job: {
          job: 'install',
          progress: { step: 'packages', package: null, done: null, total: null, line: 'Resolved 85 packages' },
        },
      }),
    );
    await reply(page, 'comfyui_settings', settings());
    await reply(page, 'comfyui_connect', { kind: 'notFound' });
    await newDocument(page, 'pixel');
    await menu(page, 'View', 'Image Generation…');
    const wizard = page.getByRole('dialog', { name: 'Install ComfyUI' });
    await expect(wizard.getByRole('status')).toContainText('Installing PyTorch and dependencies');
    await expect(wizard.getByRole('status')).toContainText('Resolved 85 packages');
  });
});

test.describe('German', () => {
  test.use({ locale: 'de-DE' });

  test('wizard, steps, license notice and errors are translated (AC-16)', async ({ page }) => {
    await installTauriMock(page);
    await page.goto('./');
    await page.getByRole('button', { name: 'Verstanden' }).click();
    await reply(page, 'comfyui_install_info', info({ hardware: nvidia(4, true) }));
    await reply(page, 'comfyui_settings', settings());
    await reply(page, 'comfyui_connect', { kind: 'notFound' });
    await page.getByRole('button', { name: 'Neues Pixel-Sprite' }).click();
    await page.getByRole('button', { name: 'Erstellen' }).click();
    await waitForEditor(page, 'pixel');
    await menu(page, 'Ansicht', 'Bildgenerierung…');
    await page.getByRole('button', { name: 'ComfyUI installieren…' }).click();
    const wizard = page.getByRole('dialog', { name: 'ComfyUI installieren' });
    await expect(wizard).toContainText('Unterstützt');
    await expect(wizard).toContainText('Die Generierung wird auf dieser Hardware langsam sein oder scheitern');
    await expect(wizard.getByRole('group', { name: 'Modellpakete' })).toContainText('Braucht 4 GB VRAM');
    await reply(page, 'comfyui_check_location', {
      target: 'D:/KI/Pincel ComfyUI',
      free: 40 * GIB,
      writable: true,
      existing: 'none',
    });
    await setMock(page, { openPick: 'D:/KI' });
    await wizard.getByRole('button', { name: 'Speicherort wählen…' }).click();
    await expect(wizard).toContainText('Installiert nach D:/KI/Pincel ComfyUI');
    await expect(wizard).toContainText('Frei: 40,0 GB · Benötigt: 14,0 GB');
    await wizard.getByRole('button', { name: 'Installieren', exact: true }).click();
    await expect(wizard.getByRole('note')).toContainText('Pincel lädt ComfyUI (GPL-3.0) und PyTorch (BSD-3-Clause)');
    await wizard.getByRole('checkbox', { name: 'Ich akzeptiere die Lizenz von SD 1.5' }).check();
    await wizard.getByRole('button', { name: 'Installieren', exact: true }).click();
    const status = wizard.getByRole('status');
    await emit(page, 'comfyui:install', {
      kind: 'progress',
      job: 'install',
      progress: { step: 'packages', package: null, done: null, total: null, line: 'x' },
    });
    await expect(status).toContainText('PyTorch und Abhängigkeiten werden installiert');
    await emit(page, 'comfyui:install', {
      kind: 'failed',
      job: 'install',
      error: { kind: 'corrupted', file: 'SD 1.5' },
    });
    await expect(wizard.getByRole('alert')).toHaveText('Download beschädigt: SD 1.5');
    await expect(wizard.getByRole('button', { name: 'Erneut versuchen' })).toBeVisible();
  });
});
