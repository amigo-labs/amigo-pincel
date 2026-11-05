import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import { hasNativeMenu, isTauri } from '../src/lib/platform';
import {
  clearClipboard,
  copiedImage,
  drag,
  gotoStart,
  imageCanvas,
  pixelCanvas,
  pixelOf,
  storedPref,
  waitForEditor,
  type ClipImage,
} from './helpers';
import { expectActive, expectDirty, tabNames } from './tab-helpers';

// The desktop shell as the shared UI sees it (docs/specs/desktop-app.md).
// src-tauri cannot run here, so a stand-in Tauri 2 bridge
// (`window.__TAURI_INTERNALS__`, as `@tauri-apps/api/mocks` builds it)
// answers the native commands, records every call and emits the events
// the Rust side would send (`open-file`, `menu`).

const LINUX_UA =
  'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';
const MAC_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';
const WINDOWS_UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';

const ASEPRITE = '../crates/pincel-core/tests/fixtures/aseprite-compat-check.aseprite';
const PNG = '../src-tauri/icons/32x32.png';

interface Bridge {
  /** `v2`: Tauri 2's `__TAURI_INTERNALS__`; `v1`: only Tauri 1's `__TAURI__`. */
  bridge?: 'v2' | 'v1';
  /** Files `read_file_bytes` can read, by path. */
  files?: Record<string, number[]>;
  /** What `updater_state` answers. */
  updater?: { supported: boolean; autoUpdate: boolean; ready: string | null };
}

interface Call {
  cmd: string;
  args: Record<string, unknown>;
}

/** Installs the stand-in desktop bridge before the app loads. */
async function desktop(page: Page, bridge: Bridge = {}): Promise<void> {
  await page.addInitScript((cfg: Bridge) => {
    type Handler = (data: unknown) => void;
    const callbacks = new Map<number, Handler>();
    const listeners = new Map<string, number[]>();
    const calls: { cmd: string; args: Record<string, unknown> }[] = [];
    let nextId = 1;
    const state = {
      updater: cfg.updater ?? { supported: false, autoUpdate: true, ready: null },
      /** Answers of the next `updater_check` calls: a version, `null`, or an error. */
      checks: [] as ({ version: string | null } | { error: string })[],
      /** What `updater_set_auto` stores; `null` stores the requested value. */
      storeAuto: null as boolean | null,
    };
    const invoke = async (cmd: string, args: Record<string, unknown> = {}): Promise<unknown> => {
      calls.push({ cmd, args: JSON.parse(JSON.stringify(args)) as Record<string, unknown> });
      switch (cmd) {
        case 'plugin:event|listen': {
          const event = args.event as string;
          listeners.set(event, [...(listeners.get(event) ?? []), args.handler as number]);
          return args.handler;
        }
        case 'plugin:event|unlisten':
          return null;
        case 'read_file_bytes': {
          const bytes = cfg.files?.[args.path as string];
          if (!bytes) throw `read ${String(args.path)}: No such file or directory (os error 2)`;
          return bytes;
        }
        case 'updater_state':
          return { ...state.updater };
        case 'updater_check': {
          const next = state.checks.shift() ?? { version: null };
          if ('error' in next) throw next.error;
          return next.version;
        }
        case 'updater_set_auto': {
          const stored = state.storeAuto ?? (args.enabled as boolean);
          state.updater.autoUpdate = stored;
          return stored;
        }
        default:
          return null;
      }
    };
    const mock = {
      calls,
      state,
      listening: (event: string) => (listeners.get(event) ?? []).length > 0,
      emit: (event: string, payload: unknown) => {
        for (const id of listeners.get(event) ?? []) {
          callbacks.get(id)?.({ event, id, payload });
        }
      },
    };
    const w = window as unknown as Record<string, unknown>;
    w.__desktopMock = mock;
    if (cfg.bridge === 'v1') {
      w.__TAURI__ = {};
      return;
    }
    w.__TAURI_INTERNALS__ = {
      invoke,
      transformCallback: (cb: Handler) => {
        const id = nextId++;
        callbacks.set(id, cb);
        return id;
      },
      unregisterCallback: (id: number) => callbacks.delete(id),
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
    };
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (event: string, id: number) => {
        listeners.set(
          event,
          (listeners.get(event) ?? []).filter((h) => h !== id),
        );
      },
    };
  }, bridge);
}

/** The native calls the page made so far. */
function calls(page: Page, cmd?: string): Promise<Call[]> {
  return page.evaluate((c) => {
    const all = (window as unknown as { __desktopMock: { calls: Call[] } }).__desktopMock.calls;
    return c ? all.filter((x) => x.cmd === c) : all;
  }, cmd);
}

/** Emits a Rust-side event once the page listens for it. */
async function emit(page: Page, event: string, payload: unknown): Promise<void> {
  await expect
    .poll(() =>
      page.evaluate(
        (e) =>
          (
            window as unknown as { __desktopMock: { listening: (e: string) => boolean } }
          ).__desktopMock.listening(e),
        event,
      ),
    )
    .toBe(true);
  await page.evaluate(
    ([e, p]) =>
      (
        window as unknown as { __desktopMock: { emit: (e: string, p: unknown) => void } }
      ).__desktopMock.emit(e, p),
    [event, payload] as const,
  );
}

/** The bytes of a repository file, as `read_file_bytes` returns them. */
async function bytes(path: string): Promise<number[]> {
  return Array.from(await readFile(path));
}

const assocDialog = (page: Page) =>
  page.getByRole('dialog', { name: 'Open sprites and images with Pincel' });

/** The start screen of a desktop copy, its file-association hint dismissed. */
async function desktopStart(page: Page, bridge: Bridge = {}): Promise<void> {
  await desktop(page, bridge);
  await gotoStart(page);
  await assocDialog(page).getByRole('button', { name: 'Got it' }).click();
  await expect(assocDialog(page)).toBeHidden();
}

const unsaved = (page: Page) => page.getByRole('alertdialog', { name: 'Unsaved changes' });
const docTitle = (page: Page, name: string) => page.getByTitle(name, { exact: true });

test.describe('desktop on Linux', () => {
  test.use({ userAgent: LINUX_UA });

  test('a file the OS hands over opens as a tab of the mode of its format, without a prompt (AC-04, document-tabs AC-02, AC-03)', async ({
    page,
  }) => {
    await desktopStart(page, {
      files: {
        '/home/u/sprite.aseprite': await bytes(ASEPRITE),
        '/home/u/pic.png': await bytes(PNG),
        '/home/u/other.png': await bytes(PNG),
      },
    });

    // An .aseprite file opens in Pixel mode, read from its path.
    await emit(page, 'open-file', '/home/u/sprite.aseprite');
    await waitForEditor(page, 'pixel');
    await expect(docTitle(page, 'sprite.aseprite')).toBeVisible();
    expect(await calls(page, 'read_file_bytes')).toEqual([
      { cmd: 'read_file_bytes', args: { path: '/home/u/sprite.aseprite' } },
    ]);

    // A PNG opens in Image mode as a second tab; the sprite's tab stays.
    await emit(page, 'open-file', '/home/u/pic.png');
    await waitForEditor(page, 'image');
    await expectActive(page, 'pic.png');
    expect(await tabNames(page)).toEqual(['sprite.aseprite', 'pic.png']);
    await expect(unsaved(page)).toHaveCount(0);

    // Unsaved changes: a further file opens beside them, without a prompt,
    // and the unsaved tab keeps its changes.
    await page.keyboard.press('b');
    await drag(page, imageCanvas(page), [0.2, 0.2], [0.8, 0.8]);
    await expectDirty(page, 'pic.png', true);
    await emit(page, 'open-file', '/home/u/other.png');
    await expectActive(page, 'other.png');
    await expect(unsaved(page)).toHaveCount(0);
    expect(await tabNames(page)).toEqual(['sprite.aseprite', 'pic.png', 'other.png']);
    await expectDirty(page, 'pic.png', true);

    // The file of an open tab activates that tab instead of a second one.
    await emit(page, 'open-file', '/home/u/sprite.aseprite');
    await expectActive(page, 'sprite.aseprite');
    expect(await tabNames(page)).toEqual(['sprite.aseprite', 'pic.png', 'other.png']);
    await expect(unsaved(page)).toHaveCount(0);
  });

  test('a .psd the OS hands over opens in Image mode; the open dialog offers .psd (psd-import AC-11, AC-13)', async ({
    page,
  }) => {
    await desktopStart(page, {
      files: {
        '/home/u/art.psd': await bytes('../crates/pincel-image-core/tests/fixtures/sample.psd'),
      },
    });
    await page.getByRole('button', { name: 'Open…' }).click();
    await expect.poll(async () => (await calls(page, 'plugin:dialog|open')).length).toBe(1);
    const [dialog] = await calls(page, 'plugin:dialog|open');
    const filters = (dialog?.args.options as { filters: { extensions: string[] }[] }).filters;
    expect(filters[0]?.extensions).toContain('psd');
    expect(filters).toContainEqual({ name: 'Photoshop image', extensions: ['psd'] });

    await emit(page, 'open-file', '/home/u/art.psd');
    await waitForEditor(page, 'image');
    await expect(docTitle(page, 'art.psd')).toBeVisible();
    await expect(page.getByRole('button', { name: /^Hide / })).toHaveCount(4);
  });

  test('Help ▸ Check for Updates… reports the outcome and the stored auto-update value (AC-16)', async ({
    page,
  }) => {
    await desktopStart(page, { updater: { supported: true, autoUpdate: true, ready: null } });
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    await page
      .getByRole('form', { name: 'New pixel sprite' })
      .getByRole('button', { name: 'Create' })
      .click();
    await waitForEditor(page, 'pixel');

    const openHelp = async () => {
      await page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }).click();
      return page.getByRole('menu', { name: 'Help', exact: true });
    };
    const about = page.getByRole('dialog', { name: 'About Pincel' });
    const check = async (answer: { version: string | null } | { error: string }) => {
      await page.evaluate((a) => {
        (
          window as unknown as { __desktopMock: { state: { checks: unknown[] } } }
        ).__desktopMock.state.checks.push(a);
      }, answer);
      await (await openHelp()).getByRole('menuitem', { name: 'Check for Updates…' }).click();
      await expect(about).toBeVisible();
    };
    const close = async () => {
      await about.getByRole('button', { name: 'Close' }).click();
      await expect(about).toBeHidden();
    };

    await check({ version: null });
    await expect(about).toContainText('Pincel is up to date.');
    await close();
    await check({ error: 'network is unreachable' });
    await expect(about).toContainText('Checking for updates failed: network is unreachable');
    await close();
    await check({ version: '9.9.9' });
    await expect(about).toContainText(
      'Pincel 9.9.9 has been downloaded and is used from the next start.',
    );
    await close();
    expect(await calls(page, 'updater_check')).toHaveLength(3);

    // The checkbox shows what was stored, not what was clicked.
    // Each look opens the Help menu afresh (Escape closes one left open).
    const auto = async () => {
      await page.keyboard.press('Escape');
      return (await openHelp()).getByRole('menuitemcheckbox', {
        name: 'Install Updates Automatically',
      });
    };
    await expect(await auto()).toHaveAttribute('aria-checked', 'true');
    await page.evaluate(() => {
      (
        window as unknown as { __desktopMock: { state: { storeAuto: boolean | null } } }
      ).__desktopMock.state.storeAuto = true;
    });
    await (await auto()).click();
    await expect.poll(async () => (await calls(page, 'updater_set_auto')).length).toBe(1);
    expect((await calls(page, 'updater_set_auto'))[0]?.args).toEqual({ enabled: false });
    await expect(await auto()).toHaveAttribute('aria-checked', 'true');
    await page.evaluate(() => {
      (
        window as unknown as { __desktopMock: { state: { storeAuto: boolean | null } } }
      ).__desktopMock.state.storeAuto = null;
    });
    await (await auto()).click();
    await expect.poll(async () => (await calls(page, 'updater_set_auto')).length).toBe(2);
    await expect(await auto()).toHaveAttribute('aria-checked', 'false');
  });

  test('the native Open Recent submenu follows the recents; menu items reach the shell or the editor (AC-19)', async ({
    page,
  }) => {
    await desktopStart(page, {
      files: {
        '/home/u/sprite.aseprite': await bytes(ASEPRITE),
        '/home/u/pic.png': await bytes(PNG),
      },
    });
    const recentMenus = async () =>
      (await calls(page, 'set_recent_menu')).map((c) =>
        (c.args.items as { id: string; name: string }[]).map((i) => i.name),
      );
    // No recents yet: the submenu gets an empty list (its placeholder).
    await expect.poll(recentMenus).toContainEqual([]);

    // Opening a file records it; the submenu lists it.
    await emit(page, 'open-file', '/home/u/sprite.aseprite');
    await waitForEditor(page, 'pixel');
    await expect.poll(async () => (await recentMenus()).at(-1)).toEqual(['sprite.aseprite']);
    await emit(page, 'open-file', '/home/u/pic.png');
    await waitForEditor(page, 'image');
    await expect
      .poll(async () => [...((await recentMenus()).at(-1) ?? [])].sort())
      .toEqual(['pic.png', 'sprite.aseprite']);

    // A menu item the mounted editor does not serve does nothing.
    const before = (await calls(page)).length;
    await emit(page, 'menu', 'menu:exportSheet');
    await emit(page, 'menu', 'recent:_empty');
    await emit(page, 'menu', 'recent:no-such-entry');
    await page.waitForTimeout(500);
    expect((await calls(page)).slice(before)).toEqual([]);
    await expect(page.getByRole('dialog')).toHaveCount(0);
    await expect(docTitle(page, 'pic.png')).toBeVisible();

    // Choosing an entry opens that file from its path.
    const items = (await calls(page, 'set_recent_menu')).at(-1)?.args.items as {
      id: string;
      name: string;
    }[];
    const sprite = items.find((i) => i.name === 'sprite.aseprite');
    if (!sprite) throw new Error('sprite.aseprite is not in the recent menu');
    await emit(page, 'menu', `recent:${sprite.id}`);
    await waitForEditor(page, 'pixel');
    await expect(docTitle(page, 'sprite.aseprite')).toBeVisible();
    expect((await calls(page, 'read_file_bytes')).at(-1)?.args).toEqual({
      path: '/home/u/sprite.aseprite',
    });

    // A served item reaches the mounted editor: Pixel mode serves Export
    // Sprite Sheet.
    await emit(page, 'menu', 'menu:exportSheet');
    await expect(page.getByRole('dialog', { name: 'Export sprite sheet' })).toBeVisible();
  });

  test('the file-association hint shows on start until "Don\'t show this again" (AC-21)', async ({
    page,
  }) => {
    await desktop(page);
    await gotoStart(page);
    const dialog = assocDialog(page);
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText('.aseprite');
    await expect(dialog).toContainText(
      'Run xdg-mime default pincel.desktop application/x-aseprite.',
    );

    // "Got it" alone: it shows again on the next start.
    await dialog.getByRole('button', { name: 'Got it' }).click();
    await expect(dialog).toBeHidden();
    await page.reload();
    await expect(dialog).toBeVisible();

    // "Don't show this again": never again.
    await dialog.getByLabel('Don’t show this again').check();
    await dialog.getByRole('button', { name: 'Got it' }).click();
    await expect(dialog).toBeHidden();
    await expect.poll(() => storedPref(page, 'fileAssocPromptShown')).toBe(true);
    await page.reload();
    await expect(page.getByRole('heading', { name: 'Pincel', exact: true })).toBeVisible();
    await page.waitForTimeout(1000);
    await expect(dialog).toHaveCount(0);
  });
});

test.describe('desktop on Windows', () => {
  test.use({ userAgent: WINDOWS_UA });

  test('the file-association hint gives the Windows steps (AC-21)', async ({ page }) => {
    await desktop(page);
    await gotoStart(page);
    await expect(assocDialog(page)).toContainText('Open Settings → Apps → Default apps.');
    await expect(assocDialog(page)).not.toContainText('xdg-mime');
  });
});

// Ctrl+R / Ctrl+Shift+R rotate the whole image, in the desktop app only
// (docs/specs/power-user-shortcuts-coverage.md AC-08); a browser tab keeps
// them for reload (shortcuts-coverage.spec.ts checks the empty hints).
test.describe('desktop rotation keys', () => {
  test.use({ userAgent: LINUX_UA, permissions: ['clipboard-read', 'clipboard-write'] });

  test('Ctrl+R and Ctrl+Shift+R rotate the image 90° (power-user-shortcuts-coverage AC-08)', async ({
    page,
  }) => {
    await desktopStart(page);
    await page.getByRole('button', { name: 'New image' }).click();
    const card = page.getByRole('form', { name: 'New image' });
    await card.getByLabel('Width').fill('32');
    await card.getByLabel('Height').fill('16');
    await card.getByRole('button', { name: 'Create' }).click();
    await waitForEditor(page, 'image');
    const size = page.locator('footer');
    // Whole numbers only: "12332 × 16" must not pass for "32 × 16".
    const docSize = (w: number, h: number) => new RegExp(String.raw`(^|\D)${w} × ${h}(\D|$)`);
    await expect(size).toContainText(docSize(32, 16));

    await page.getByRole('menubar').getByRole('menuitem', { name: 'Image', exact: true }).click();
    const imageMenu = page.getByRole('menu', { name: 'Image', exact: true });
    await expect(imageMenu.getByRole('menuitem', { name: 'Rotate 90° CW' })).toContainText(
      'Ctrl+R',
    );
    await expect(imageMenu.getByRole('menuitem', { name: 'Rotate 90° CCW' })).toContainText(
      'Ctrl+Shift+R',
    );
    await page.keyboard.press('Escape');

    // A mark in the top-left corner shows the direction of each turn.
    await page.keyboard.press('b');
    await drag(page, imageCanvas(page), [0.02, 0.04], [0.06, 0.04]);
    const image = async (): Promise<ClipImage> => {
      await page.keyboard.press('ControlOrMeta+a');
      await clearClipboard(page);
      await page.keyboard.press('ControlOrMeta+c');
      await page.keyboard.press('ControlOrMeta+d');
      return copiedImage(page);
    };
    const opaque = (img: ClipImage, x: number, y: number) => (pixelOf(img, x, y)[3] ?? 0) > 0;
    expect(opaque(await image(), 0, 0)).toBe(true);

    // Clockwise: the top-left corner goes to the top-right.
    await page.keyboard.press('Control+r');
    await expect(size).toContainText(docSize(16, 32));
    const cw = await image();
    expect(opaque(cw, cw.width - 1, 0)).toBe(true);
    expect(opaque(cw, 0, 0)).toBe(false);

    // Counter-clockwise turns it back.
    await page.keyboard.press('Control+Shift+R');
    await expect(size).toContainText(docSize(32, 16));
    const back = await image();
    expect(opaque(back, 0, 0)).toBe(true);

    // Counter-clockwise once more: the top-left corner goes to the bottom-left.
    await page.keyboard.press('Control+Shift+R');
    await expect(size).toContainText(docSize(16, 32));
    const ccw = await image();
    expect(opaque(ccw, 0, ccw.height - 1)).toBe(true);
    expect(opaque(ccw, 0, 0)).toBe(false);
  });
});

test.describe('desktop on macOS', () => {
  test.use({ userAgent: MAC_UA });

  test('the file-association hint gives the macOS steps (AC-21)', async ({ page }) => {
    await desktop(page);
    await gotoStart(page);
    await expect(assocDialog(page)).toContainText(
      'Right-click an .aseprite file in Finder → Get Info.',
    );
  });

  test('the native menu bar replaces the in-app Help menu only in the desktop app (AC-20)', async ({
    page,
  }) => {
    await desktopStart(page);
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    await page
      .getByRole('form', { name: 'New pixel sprite' })
      .getByRole('button', { name: 'Create' })
      .click();
    await waitForEditor(page, 'pixel');
    const menubar = page.getByRole('menubar');
    await expect(menubar.getByRole('menuitem', { name: 'File', exact: true })).toBeVisible();
    await expect(menubar.getByRole('menuitem', { name: 'Help', exact: true })).toHaveCount(0);
  });

  test('a browser on macOS keeps the in-app Help menu (AC-20)', async ({ page }) => {
    await gotoStart(page);
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    await page
      .getByRole('form', { name: 'New pixel sprite' })
      .getByRole('button', { name: 'Create' })
      .click();
    await waitForEditor(page, 'pixel');
    await expect(
      page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }),
    ).toBeVisible();
  });
});

test.describe('capability detection', () => {
  test.use({ userAgent: LINUX_UA });

  test('the Tauri 2 and Tauri 1 bridges count as the desktop app, a browser does not (AC-20)', async ({
    browser,
  }) => {
    for (const bridge of ['v2', 'v1', null] as const) {
      const page = await browser.newPage({ userAgent: LINUX_UA });
      if (bridge) await desktop(page, { bridge });
      await gotoStart(page);
      if (bridge) {
        // Desktop-only behaviour: the file-association hint.
        await expect(assocDialog(page)).toBeVisible();
      } else {
        await page.waitForTimeout(1000);
        await expect(assocDialog(page)).toHaveCount(0);
      }
      // On Linux the in-app Help menu stays, desktop or not.
      if (bridge) await assocDialog(page).getByRole('button', { name: 'Got it' }).click();
      await page.getByRole('button', { name: 'New pixel sprite' }).click();
      await page
        .getByRole('form', { name: 'New pixel sprite' })
        .getByRole('button', { name: 'Create' })
        .click();
      await expect(pixelCanvas(page)).toHaveAttribute('data-ready', 'true', { timeout: 60_000 });
      await expect(
        page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }),
      ).toBeVisible();
      await page.close();
    }
  });

  test('isTauri / hasNativeMenu outside a page: server-side rendering and each bridge (AC-20)', () => {
    const g = globalThis as Record<string, unknown>;
    const saved = { window: g.window, navigator: Object.getOwnPropertyDescriptor(g, 'navigator') };
    try {
      // Server-side rendering: no `window`.
      delete g.window;
      expect(isTauri()).toBe(false);
      expect(hasNativeMenu()).toBe(false);

      Object.defineProperty(g, 'navigator', { value: { userAgent: MAC_UA }, configurable: true });
      g.window = {};
      expect(isTauri()).toBe(false);
      expect(hasNativeMenu()).toBe(false);
      g.window = { __TAURI_INTERNALS__: {} };
      expect(isTauri()).toBe(true);
      expect(hasNativeMenu()).toBe(true);
      g.window = { __TAURI__: {} };
      expect(isTauri()).toBe(true);
      expect(hasNativeMenu()).toBe(true);

      for (const ua of [LINUX_UA, WINDOWS_UA]) {
        Object.defineProperty(g, 'navigator', { value: { userAgent: ua }, configurable: true });
        g.window = { __TAURI_INTERNALS__: {} };
        expect(isTauri()).toBe(true);
        expect(hasNativeMenu()).toBe(false);
      }
    } finally {
      if (saved.window === undefined) delete g.window;
      else g.window = saved.window;
      if (saved.navigator) Object.defineProperty(g, 'navigator', saved.navigator);
    }
  });
});
