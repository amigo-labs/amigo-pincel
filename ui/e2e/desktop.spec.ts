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
  updater?: Partial<UpdaterAnswer>;
  /** Files the OS handed over before the page listened (launch argument);
   *  the first `take_open_files` returns them, later calls nothing. */
  launchFiles?: string[];
}

/** What `updater_state` answers (src-tauri/src/updater.rs `UpdaterInfo`). */
interface UpdaterAnswer {
  supported: boolean;
  mode: 'auto' | 'notify' | 'off';
  chosen: boolean;
  ready: string | null;
  announce: string | null;
  checking: boolean;
  unsupported: 'package' | 'appimage' | null;
}

/** An answer the stand-in updater gives to a check. */
type CheckAnswer =
  | { state: 'upToDate' }
  | { state: 'available'; version: string }
  | { state: 'ready'; version: string }
  | { error: string };

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
      updater: {
        supported: false,
        mode: 'auto',
        chosen: true,
        ready: null,
        announce: null,
        checking: false,
        unsupported: null,
        ...cfg.updater,
      } as UpdaterAnswer,
      /** Answers of the next `updater_check` calls (default: up to date). */
      checks: [] as CheckAnswer[],
      /** Answers of the next `updater_check_on_open` calls (default: up to date). */
      opens: [] as CheckAnswer[],
      /** Whether `updater_set_mode` manages to store the choice. */
      storeMode: true,
      launchFiles: [...(cfg.launchFiles ?? [])],
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
        case 'take_open_files':
          return state.launchFiles.splice(0);
        case 'updater_state':
          return { ...state.updater };
        case 'updater_check':
        case 'updater_check_on_open': {
          if (cmd === 'updater_check_on_open' && state.updater.mode === 'off') return null;
          const queue = cmd === 'updater_check' ? state.checks : state.opens;
          const next = queue.shift() ?? { state: 'upToDate' };
          if ('error' in next) throw next.error;
          if (next.state === 'ready') {
            state.updater = { ...state.updater, ready: next.version, announce: null };
          }
          return next;
        }
        case 'updater_mark_announced':
          state.updater = { ...state.updater, announce: null };
          return null;
        case 'updater_arm_restart':
          if (state.updater.ready === null) throw 'no update is downloaded';
          return null;
        case 'updater_set_mode': {
          const mode = args.mode as UpdaterAnswer['mode'];
          state.updater = { ...state.updater, mode, chosen: true };
          return { mode, saved: state.storeMode };
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
const question = (page: Page) =>
  page.getByRole('dialog', { name: 'How should Pincel handle updates?' });
const aboutDialog = (page: Page) => page.getByRole('dialog', { name: 'About Pincel' });
const notice = (page: Page) => page.getByRole('region', { name: 'Update available' });

/** The three update choices, as the question and the About dialog name them. */
const CHOICE = {
  auto: 'Install automatically (recommended)',
  notify: 'Notify me when an update is available',
  off: 'Never check for updates',
} as const;

/** Changes what `updater_state` answers and sends `updater-changed`. */
async function setUpdater(page: Page, patch: Partial<UpdaterAnswer>): Promise<void> {
  await page.evaluate((p) => {
    const mock = (
      window as unknown as { __desktopMock: { state: { updater: Record<string, unknown> } } }
    ).__desktopMock;
    mock.state.updater = { ...mock.state.updater, ...p };
  }, patch);
  await emit(page, 'updater-changed', null);
}

/** Queues the answer of the next `updater_check` (`checks`) or
 *  `updater_check_on_open` (`opens`). */
async function pushAnswer(
  page: Page,
  queue: 'checks' | 'opens',
  answer: CheckAnswer,
): Promise<void> {
  await page.evaluate(
    ([q, a]) => {
      (
        window as unknown as { __desktopMock: { state: Record<string, unknown[]> } }
      ).__desktopMock.state[q]?.push(a);
    },
    [queue, answer] as const,
  );
}

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

  test('a file passed at launch, before the page listens, opens instead of the start screen (AC-04)', async ({
    page,
  }) => {
    await desktop(page, {
      files: { '/home/u/pic.png': await bytes(PNG) },
      launchFiles: ['/home/u/pic.png'],
    });
    await page.goto('./');
    await waitForEditor(page, 'image');
    await expect(docTitle(page, 'pic.png')).toBeVisible();
    expect(await calls(page, 'read_file_bytes')).toEqual([
      { cmd: 'read_file_bytes', args: { path: '/home/u/pic.png' } },
    ]);
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

  test('the open dialog offers TGA, DDS and ICO under all supported files and images (image-formats-more AC-14)', async ({
    page,
  }) => {
    await desktopStart(page);
    await page.getByRole('button', { name: 'Open…' }).click();
    await expect.poll(async () => (await calls(page, 'plugin:dialog|open')).length).toBe(1);
    const [dialog] = await calls(page, 'plugin:dialog|open');
    const filters = (dialog?.args.options as { filters: { name: string; extensions: string[] }[] })
      .filters;
    const images = filters.find((f) => f.name === 'Images');
    for (const ext of ['tga', 'dds', 'ico']) {
      expect(filters[0]?.extensions).toContain(ext);
      expect(images?.extensions).toContain(ext);
    }
  });

  test('Help ▸ Check for Updates… checks whatever the update choice and reports the outcome; no auto-update checkbox (AC-16)', async ({
    page,
  }) => {
    // "never check" (AC-27): the manual check still checks.
    await desktopStart(page, { updater: { supported: true, mode: 'off' } });
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
    const check = async (answer: CheckAnswer) => {
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

    await check({ state: 'upToDate' });
    await expect(about).toContainText('Pincel is up to date.');
    await close();
    await check({ error: 'network is unreachable' });
    await expect(about).toContainText('Checking for updates failed: network is unreachable');
    await close();
    await check({ state: 'ready', version: '9.9.9' });
    await expect(about).toContainText(
      'Pincel 9.9.9 has been downloaded and is used from the next start.',
    );
    await close();
    expect(await calls(page, 'updater_check')).toHaveLength(3);
    // Opening About through Check for Updates… starts no second check (AC-31).
    expect(await calls(page, 'updater_check_on_open')).toHaveLength(0);

    // The update choice replaced the Install Updates Automatically checkbox.
    await page.keyboard.press('Escape');
    const help = await openHelp();
    await expect(help.getByRole('menuitem', { name: 'Check for Updates…' })).toBeVisible();
    await expect(help.getByRole('menuitemcheckbox')).toHaveCount(0);
    await expect(help.getByText('Install Updates Automatically')).toHaveCount(0);
  });

  test('the first-start question follows the file-association hint, stores only a confirmed choice and holds the background check (AC-22, AC-23, AC-24)', async ({
    page,
  }) => {
    await desktop(page, { updater: { supported: true, chosen: false } });
    const ask = async () => {
      await gotoStart(page);
      // The file-association hint comes first; the question waits for it.
      await expect(assocDialog(page)).toBeVisible();
      await expect(question(page)).toHaveCount(0);
      await assocDialog(page).getByRole('button', { name: 'Got it' }).click();
      await expect(question(page)).toBeVisible();
      expect(await calls(page, 'updater_release')).toHaveLength(0);
    };

    await ask();
    const q = question(page);
    await expect(q.getByRole('radio', { name: CHOICE.auto })).toBeChecked();
    await expect(q.getByRole('radio', { name: CHOICE.notify })).not.toBeChecked();
    await expect(q.getByRole('radio', { name: CHOICE.off })).not.toBeChecked();
    // Escape stores nothing and releases the background check.
    await page.keyboard.press('Escape');
    await expect(q).toBeHidden();
    await expect.poll(async () => (await calls(page, 'updater_release')).length).toBe(1);
    expect(await calls(page, 'updater_set_mode')).toHaveLength(0);

    // The next start asks again; the close button stores nothing either.
    await page.reload();
    await ask();
    await q.getByRole('button', { name: 'Close' }).click();
    await expect(q).toBeHidden();
    await expect.poll(async () => (await calls(page, 'updater_release')).length).toBe(1);
    expect(await calls(page, 'updater_set_mode')).toHaveLength(0);

    // Confirm stores the choice made.
    await page.reload();
    await ask();
    await q.getByRole('radio', { name: CHOICE.notify }).check();
    await q.getByRole('button', { name: 'Confirm' }).click();
    await expect(q).toBeHidden();
    await expect
      .poll(async () => (await calls(page, 'updater_set_mode')).map((c) => c.args))
      .toEqual([{ mode: 'notify' }]);
    await expect.poll(async () => (await calls(page, 'updater_release')).length).toBe(1);
  });

  test('no first-start question once a choice is stored, nor in a copy that cannot update itself (AC-23, AC-25)', async ({
    page,
  }) => {
    await desktopStart(page, { updater: { supported: true, chosen: true } });
    await expect.poll(async () => (await calls(page, 'updater_release')).length).toBe(1);
    await expect(question(page)).toHaveCount(0);

    await desktopStart(page, { updater: { supported: false, chosen: false } });
    await expect(page.getByRole('button', { name: 'About Pincel' })).toBeVisible();
    await expect(question(page)).toHaveCount(0);
  });

  test('a version found under "notify" shows a notice; Later, Escape, close and Install settle it (AC-26)', async ({
    page,
  }) => {
    await desktopStart(page, { updater: { supported: true, mode: 'notify', announce: '2.0.0' } });
    const card = notice(page);
    await expect(card).toContainText('Pincel 2.0.0 is available');
    // A card that takes no focus.
    expect(
      await page.evaluate(() => document.activeElement?.closest('[data-update-notice]') ?? null),
    ).toBeNull();
    const marked = async () =>
      (await calls(page, 'updater_mark_announced')).map((c) => c.args.version);

    await card.getByRole('button', { name: 'Later' }).click();
    await expect(card).toBeHidden();
    await expect.poll(marked).toEqual(['2.0.0']);

    // Escape with the focus in the notice counts as Later.
    await setUpdater(page, { announce: '2.1.0' });
    await expect(card).toContainText('Pincel 2.1.0 is available');
    await card.getByRole('button', { name: 'Install' }).focus();
    await page.keyboard.press('Escape');
    await expect(card).toBeHidden();
    await expect.poll(marked).toEqual(['2.0.0', '2.1.0']);

    // So does its close button.
    await setUpdater(page, { announce: '2.2.0' });
    await card.getByRole('button', { name: 'Close notice' }).click();
    await expect(card).toBeHidden();
    await expect.poll(marked).toEqual(['2.0.0', '2.1.0', '2.2.0']);

    // Install reports in the About dialog; a failed one keeps the notice.
    const about = aboutDialog(page);
    await setUpdater(page, { announce: '2.3.0' });
    await pushAnswer(page, 'checks', { error: 'network is unreachable' });
    await card.getByRole('button', { name: 'Install' }).click();
    await expect(about).toContainText('Checking for updates failed: network is unreachable');
    await about.getByRole('button', { name: 'Close' }).click();
    await expect(card).toContainText('Pincel 2.3.0 is available');

    await pushAnswer(page, 'checks', { state: 'ready', version: '2.3.0' });
    await card.getByRole('button', { name: 'Install' }).click();
    await expect(about).toContainText(
      'Pincel 2.3.0 has been downloaded and is used from the next start.',
    );
    await expect(card).toBeHidden();
    expect(await marked()).toEqual(['2.0.0', '2.1.0', '2.2.0']);
    expect(await calls(page, 'updater_check')).toHaveLength(2);
    expect(await calls(page, 'updater_check_on_open')).toHaveLength(0);
  });

  test('the About dialog shows the update choices and a Check for Updates button; a choice applies at once (AC-28)', async ({
    page,
  }) => {
    await desktopStart(page, { updater: { supported: true, mode: 'auto' } });
    await page.getByRole('button', { name: 'About Pincel' }).click();
    const about = aboutDialog(page);
    await expect(about.getByText(/^Version /)).toBeVisible();
    const choices = about.getByRole('radiogroup', { name: 'Updates' });
    await expect(choices.getByRole('radio', { name: CHOICE.auto })).toBeChecked();

    await choices.getByRole('radio', { name: CHOICE.off }).check();
    await expect
      .poll(async () => (await calls(page, 'updater_set_mode')).map((c) => c.args))
      .toEqual([{ mode: 'off' }]);
    await expect(about).not.toContainText('could not be saved');

    await page.evaluate(() => {
      (
        window as unknown as { __desktopMock: { state: { storeMode: boolean } } }
      ).__desktopMock.state.storeMode = false;
    });
    await choices.getByRole('radio', { name: CHOICE.notify }).check();
    await expect(about).toContainText(
      'The update choice could not be saved; it applies until Pincel closes.',
    );
    await expect(choices.getByRole('radio', { name: CHOICE.notify })).toBeChecked();

    await pushAnswer(page, 'checks', { state: 'upToDate' });
    await about.getByRole('button', { name: 'Check for Updates' }).click();
    await expect(about).toContainText('Pincel is up to date.');
    expect(await calls(page, 'updater_check')).toHaveLength(1);
  });

  test('a copy that cannot update itself names the reason instead of the choices (AC-28)', async ({
    page,
  }) => {
    for (const [unsupported, text] of [
      ['package', 'Updates come from your package manager.'],
      ['appimage', 'Install with --install to receive updates.'],
    ] as const) {
      await desktopStart(page, { updater: { supported: false, unsupported } });
      await page.getByRole('button', { name: 'About Pincel' }).click();
      const about = aboutDialog(page);
      await expect(about).toContainText(text);
      await expect(about.getByRole('radiogroup')).toHaveCount(0);
      await expect(about.getByRole('button', { name: 'Check for Updates' })).toHaveCount(0);
    }
    expect(await calls(page, 'updater_check_on_open')).toHaveLength(0);
  });

  test('opening About checks under "install automatically" and "notify", not under "never check" (AC-31)', async ({
    page,
  }) => {
    await desktopStart(page, { updater: { supported: true, mode: 'notify' } });
    const about = aboutDialog(page);
    const open = async () => {
      await page.getByRole('button', { name: 'About Pincel' }).click();
      await expect(about).toBeVisible();
    };
    const close = async () => {
      await about.getByRole('button', { name: 'Close' }).click();
      await expect(about).toBeHidden();
    };

    // "notify": the dialog names the version and offers Install.
    await pushAnswer(page, 'opens', { state: 'available', version: '3.0.0' });
    await open();
    await expect(about).toContainText('Pincel 3.0.0 is available.');
    await close();
    // Closing it is not Later.
    expect(await calls(page, 'updater_mark_announced')).toHaveLength(0);
    await pushAnswer(page, 'opens', { state: 'available', version: '3.0.0' });
    await open();
    await pushAnswer(page, 'checks', { state: 'ready', version: '3.0.0' });
    await about.getByRole('button', { name: 'Install' }).click();
    await expect(about).toContainText(
      'Pincel 3.0.0 has been downloaded and is used from the next start.',
    );
    await close();
    expect(await calls(page, 'updater_check_on_open')).toHaveLength(2);

    // A downloaded version is reported at once, without a new check.
    await setUpdater(page, { mode: 'auto' });
    await open();
    await expect(about).toContainText(
      'Pincel 3.0.0 has been downloaded and is used from the next start.',
    );
    await close();
    expect(await calls(page, 'updater_check_on_open')).toHaveLength(2);

    // "install automatically" downloads.
    await setUpdater(page, { ready: null });
    await pushAnswer(page, 'opens', { state: 'ready', version: '3.1.0' });
    await open();
    await expect(about).toContainText(
      'Pincel 3.1.0 has been downloaded and is used from the next start.',
    );
    await close();
    expect(await calls(page, 'updater_check_on_open')).toHaveLength(3);

    // "never check": no check starts; the button still checks.
    await setUpdater(page, { mode: 'off', ready: null });
    await open();
    await expect(about.getByRole('status')).toHaveCount(0);
    expect(await calls(page, 'updater_check_on_open')).toHaveLength(3);
    await about.getByRole('button', { name: 'Check for Updates' }).click();
    await expect(about).toContainText('Pincel is up to date.');
  });

  test('Restart Now restarts at once without unsaved changes (AC-32)', async ({ page }) => {
    await desktopStart(page, { updater: { supported: true, mode: 'off', ready: '4.0.0' } });
    await page.getByRole('button', { name: 'About Pincel' }).click();
    const about = aboutDialog(page);
    await expect(about).toContainText(
      'Pincel 4.0.0 has been downloaded and is used from the next start.',
    );
    await about.getByRole('button', { name: 'Restart Now' }).click();
    await expect.poll(async () => (await calls(page, 'plugin:window|destroy')).length).toBe(1);
    expect(await calls(page, 'updater_arm_restart')).toHaveLength(1);
  });

  test('Restart Now asks about unsaved changes first; Cancel keeps Pincel running (AC-32)', async ({
    page,
  }) => {
    await desktopStart(page, { updater: { supported: true, mode: 'off', ready: '4.0.0' } });
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    await page
      .getByRole('form', { name: 'New pixel sprite' })
      .getByRole('button', { name: 'Create' })
      .click();
    await waitForEditor(page, 'pixel');
    await drag(page, pixelCanvas(page), [0.2, 0.2], [0.8, 0.8]);
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }).click();
    await page
      .getByRole('menu', { name: 'Help', exact: true })
      .getByRole('menuitem', { name: 'About Pincel' })
      .click();
    const about = aboutDialog(page);
    const restart = about.getByRole('button', { name: 'Restart Now' });

    await restart.click();
    await expect(unsaved(page)).toBeVisible();
    await unsaved(page).getByRole('button', { name: 'Cancel' }).click();
    await expect(unsaved(page)).toBeHidden();
    expect(await calls(page, 'updater_arm_restart')).toHaveLength(0);
    expect(await calls(page, 'plugin:window|destroy')).toHaveLength(0);

    await restart.click();
    await unsaved(page).getByRole('button', { name: "Don't Save" }).click();
    await expect.poll(async () => (await calls(page, 'plugin:window|destroy')).length).toBe(1);
    expect(await calls(page, 'updater_arm_restart')).toHaveLength(1);
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

  test('the About dialog says a downloaded version is installed when Pincel exits (AC-28)', async ({
    page,
  }) => {
    await desktopStart(page, { updater: { supported: true, mode: 'off', ready: '2.0.0' } });
    await page.getByRole('button', { name: 'About Pincel' }).click();
    await expect(aboutDialog(page)).toContainText(
      'Pincel 2.0.0 has been downloaded and is installed when Pincel exits.',
    );
  });

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

  test('the native menu About and Check for Updates… open the in-app About dialog (AC-17, AC-31)', async ({
    page,
  }) => {
    await desktopStart(page, { updater: { supported: true, mode: 'auto' } });
    const about = aboutDialog(page);
    await emit(page, 'menu', 'menu:about');
    await expect(about).toBeVisible();
    await expect.poll(async () => (await calls(page, 'updater_check_on_open')).length).toBe(1);
    await about.getByRole('button', { name: 'Close' }).click();
    await expect(about).toBeHidden();

    await emit(page, 'menu', 'menu:check-updates');
    await expect(about).toContainText('Pincel is up to date.');
    expect(await calls(page, 'updater_check')).toHaveLength(1);
    expect(await calls(page, 'updater_check_on_open')).toHaveLength(1);
  });

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

test.describe('desktop in German', () => {
  test.use({ userAgent: LINUX_UA, locale: 'de-DE' });

  test('the first-start question, the notice and the About update part speak German (AC-30)', async ({
    page,
  }) => {
    await desktop(page, {
      updater: { supported: true, chosen: false, mode: 'notify', announce: '2.0.0' },
    });
    await gotoStart(page);
    await page
      .getByRole('dialog', { name: 'Sprites und Bilder mit Pincel öffnen' })
      .getByRole('button', { name: 'Verstanden' })
      .click();
    const q = page.getByRole('dialog', { name: 'Wie soll Pincel mit Updates umgehen?' });
    for (const name of [
      'Automatisch installieren (empfohlen)',
      'Benachrichtigen, wenn ein Update verfügbar ist',
      'Nie nach Updates suchen',
    ]) {
      await expect(q.getByRole('radio', { name })).toBeVisible();
    }
    await expect(q.getByRole('button', { name: 'Schließen' })).toBeVisible();
    await q.getByRole('button', { name: 'Bestätigen' }).click();

    const card = page.getByRole('region', { name: 'Update verfügbar' });
    await expect(card).toContainText('Pincel 2.0.0 ist verfügbar');
    await expect(card.getByRole('button', { name: 'Später' })).toBeVisible();
    await expect(card.getByRole('button', { name: 'Hinweis schließen' })).toBeVisible();

    await page.getByRole('button', { name: 'Über Pincel' }).click();
    const about = page.getByRole('dialog', { name: 'Über Pincel' });
    await expect(about.getByRole('radiogroup', { name: 'Updates' })).toBeVisible();
    await expect(about.getByRole('button', { name: 'Nach Updates suchen' })).toBeVisible();
  });
});
