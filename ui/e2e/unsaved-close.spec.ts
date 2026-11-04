import { expect, test, type Locator, type Page } from '@playwright/test';
import { drag, imageCanvas, menu, pixelCanvas, waitForEditor } from './helpers';

// Desktop close guard and the OS hand-over of the unsaved-changes prompt
// (docs/specs/unsaved-changes-prompt.md AC-01, AC-08 – AC-11), against a
// mocked Tauri bridge: `__TAURI_INTERNALS__` answers the IPC calls the UI
// makes (event listen, window destroy, native dialogs, file reads and
// writes over a virtual disk) and lets the test emit events. The real
// window behavior on each OS stays a manual check (AC-09, AC-10).

type WriteMode = 'write' | 'fail' | 'hold';

interface TauriMock {
  emit(event: string, payload: unknown): void;
  destroyed: number;
  writeMode: WriteMode;
  release: (() => void) | null;
  savePick: string | null;
  disk: Record<string, number[]>;
}

type MockWindow = Window & { __mock: TauriMock };

test.beforeEach(async ({ page }) => {
  // A reload with unsaved work asks the browser's leave-page question.
  page.on('dialog', (d) => void d.accept());
  await page.addInitScript(() => {
    type Callback = (payload: unknown) => void;
    const callbacks = new Map<number, Callback>();
    const listeners = new Map<string, number[]>();
    let nextId = 1;
    const mock: TauriMock = {
      emit(event, payload) {
        for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
      },
      destroyed: 0,
      writeMode: 'write',
      release: null,
      savePick: '/docs/sprite.aseprite',
      disk: {},
    };
    const invoke = async (cmd: string, args: Record<string, unknown>): Promise<unknown> => {
      switch (cmd) {
        case 'plugin:event|listen': {
          const event = String(args.event);
          const handler = Number(args.handler);
          listeners.set(event, [...(listeners.get(event) ?? []), handler]);
          return handler;
        }
        case 'plugin:window|destroy':
          mock.destroyed += 1;
          return null;
        case 'plugin:dialog|save':
          return mock.savePick;
        case 'read_file_bytes': {
          const bytes = mock.disk[String(args.path)];
          if (!bytes) throw new Error('No such file or directory');
          return bytes;
        }
        case 'write_file_bytes':
          if (mock.writeMode === 'hold') {
            await new Promise<void>((resolve) => (mock.release = resolve));
          }
          if (mock.writeMode === 'fail') throw new Error('disk full');
          mock.disk[String(args.path)] = args.bytes as number[];
          return null;
        default:
          return null;
      }
    };
    Object.assign(window, {
      __mock: mock,
      __TAURI_INTERNALS__: {
        metadata: {
          currentWindow: { label: 'main' },
          currentWebview: { windowLabel: 'main', label: 'main' },
        },
        invoke,
        transformCallback: (cb: Callback) => {
          const id = nextId++;
          callbacks.set(id, cb);
          return id;
        },
        unregisterCallback: (id: number) => callbacks.delete(id),
        convertFileSrc: (path: string) => path,
      },
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => undefined },
    });
  });
});

/** A valid 1×1 PNG. */
const PNG_1X1 = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==',
  'base64',
);

function prompt(page: Page): Locator {
  return page.getByRole('alertdialog', { name: 'Unsaved changes' });
}

async function emit(page: Page, event: string, payload: unknown = null): Promise<void> {
  await page.evaluate(([e, p]) => (window as unknown as MockWindow).__mock.emit(e as string, p), [
    event,
    payload,
  ]);
}

async function closeWindow(page: Page): Promise<void> {
  await emit(page, 'tauri://close-requested');
}

async function destroyed(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as MockWindow).__mock.destroyed);
}

async function setMock(page: Page, patch: Partial<Pick<TauriMock, 'writeMode' | 'savePick'>>): Promise<void> {
  await page.evaluate((p) => Object.assign((window as unknown as MockWindow).__mock, p), patch);
}

async function putFile(page: Page, path: string, bytes: Buffer | null): Promise<void> {
  await page.evaluate(
    ([p, b]) => {
      const disk = (window as unknown as MockWindow).__mock.disk;
      if (b === null) delete disk[p as string];
      else disk[p as string] = b as number[];
    },
    [path, bytes === null ? null : Array.from(bytes)] as const,
  );
}

async function releaseWrite(page: Page): Promise<void> {
  await page.waitForFunction(() => (window as unknown as MockWindow).__mock.release !== null);
  await page.evaluate(() => {
    const mock = (window as unknown as MockWindow).__mock;
    mock.release?.();
    mock.release = null;
  });
}

/** Opens the app (past the first-launch file-association note). */
async function start(page: Page): Promise<void> {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
  await expect(page.getByRole('heading', { name: 'Pincel' })).toBeVisible();
}

/** Creates a 16×16 document of `mode` from the start screen. */
async function create(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  const name = mode === 'pixel' ? 'New pixel sprite' : 'New image';
  await page.getByRole('button', { name }).click();
  const card = page.getByRole('form', { name });
  for (const label of ['Width', 'Height']) await card.getByLabel(label).fill('16');
  await card.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, mode);
}

async function paint(page: Page, mode: 'pixel' | 'image'): Promise<void> {
  if (mode === 'pixel') {
    await page.getByTitle('Pencil (B)').click();
    await drag(page, pixelCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  } else {
    await drag(page, imageCanvas(page), [0.2, 0.5], [0.8, 0.5]);
  }
}

test('closing with unsaved changes asks; Cancel keeps the app running (AC-09, AC-06)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await closeWindow(page);
  await expect(prompt(page)).toBeVisible();
  await expect(prompt(page)).toContainText('Save changes to "pincel.aseprite"?');
  // A second close while the prompt is open is ignored (AC-08).
  await closeWindow(page);
  await expect(prompt(page)).toHaveCount(1);
  await prompt(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(prompt(page)).toHaveCount(0);
  expect(await destroyed(page)).toBe(0);
  await expect(page.getByTitle('pincel.aseprite — unsaved changes')).toBeVisible();
});

test("Don't Save closes the app and recovery never offers the work (AC-09, AC-05)", async ({ page }) => {
  await page.clock.install();
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  // Let autosave write a snapshot first.
  await page.clock.fastForward(31_000);
  await page.waitForTimeout(500);
  await closeWindow(page);
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await expect.poll(() => destroyed(page)).toBe(1);

  // The next start offers nothing to recover.
  await start(page);
  await create(page, 'pixel');
  await page.waitForTimeout(500);
  await expect(page.getByRole('dialog', { name: 'Recover unsaved work?' })).toHaveCount(0);
});

test('Save closes the app once the save succeeded (AC-09, AC-03)', async ({ page }) => {
  await start(page);
  await create(page, 'image');
  await paint(page, 'image');
  await setMock(page, { savePick: '/docs/picture.ora' });
  await closeWindow(page);
  await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
  await expect.poll(() => destroyed(page)).toBe(1);
  expect(await page.evaluate(() => Object.keys((window as unknown as MockWindow).__mock.disk))).toEqual([
    '/docs/picture.ora',
  ]);
});

test('a dismissed save picker keeps the app open (AC-09, AC-04)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await setMock(page, { savePick: null });
  await closeWindow(page);
  await prompt(page).getByRole('button', { name: 'Save', exact: true }).click();
  await expect(prompt(page)).toHaveCount(0);
  await page.waitForTimeout(200);
  expect(await destroyed(page)).toBe(0);
  await expect(page.getByTitle('pincel.aseprite — unsaved changes')).toBeVisible();
});

test('without unsaved changes, or on the start screen, the app closes at once (AC-09, AC-07)', async ({
  page,
}) => {
  await start(page);
  await closeWindow(page);
  await expect.poll(() => destroyed(page)).toBe(1);
  await create(page, 'pixel');
  await closeWindow(page);
  await expect.poll(() => destroyed(page)).toBe(2);
  await expect(prompt(page)).toHaveCount(0);
});

test('macOS Quit (menu:quit) asks like a window close (AC-09)', async ({ page }) => {
  await start(page);
  await create(page, 'image');
  await paint(page, 'image');
  await emit(page, 'menu', 'menu:quit');
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await expect.poll(() => destroyed(page)).toBe(1);
});

test('a close during a running save waits for it, then closes (AC-10)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await setMock(page, { writeMode: 'hold' });
  await page.keyboard.press('Control+s');
  await page.waitForFunction(() => (window as unknown as MockWindow).__mock.release !== null);
  await closeWindow(page);
  await page.waitForTimeout(200);
  await expect(prompt(page)).toHaveCount(0);
  expect(await destroyed(page)).toBe(0);
  await releaseWrite(page);
  await expect.poll(() => destroyed(page)).toBe(1);
  await expect(prompt(page)).toHaveCount(0);
});

test('a close during a save that fails asks after it (AC-10)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await setMock(page, { writeMode: 'hold' });
  await page.keyboard.press('Control+s');
  await page.waitForFunction(() => (window as unknown as MockWindow).__mock.release !== null);
  await closeWindow(page);
  await setMock(page, { writeMode: 'fail' });
  await releaseWrite(page);
  await expect(prompt(page)).toBeVisible();
  expect(await destroyed(page)).toBe(0);
});

test('a file handed over by the OS asks first (AC-01)', async ({ page }) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  await putFile(page, '/docs/dot.png', PNG_1X1);
  await emit(page, 'open-file', '/docs/dot.png');
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await waitForEditor(page, 'image');
  await expect(page.getByTitle('dot.png', { exact: true })).toBeVisible();
});

test('a handed-over file that cannot be read shows its error and no prompt (AC-11)', async ({ page }) => {
  await start(page);
  await create(page, 'image');
  await paint(page, 'image');
  await emit(page, 'open-file', '/docs/missing.png');
  await expect(page.getByText(/Open failed: .*No such file/)).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await expect(page.getByTitle('image.ora — unsaved changes')).toBeVisible();
});

test('Open Recent asks first, from both editors; a missing recent shows its error (AC-01, AC-11)', async ({
  page,
}) => {
  await start(page);
  await create(page, 'pixel');
  await paint(page, 'pixel');
  // Saving to a path records the recent.
  await page.keyboard.press('Control+s');
  await expect(page.getByTitle('sprite.aseprite', { exact: true })).toBeVisible();
  await paint(page, 'pixel');

  // Pixel mode's own File ▸ Open Recent.
  await menu(page, 'File', 'Open Recent', 'sprite.aseprite');
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await expect(page.getByTitle('sprite.aseprite', { exact: true })).toBeVisible();

  // Image mode routes Open Recent through the shell.
  await menu(page, 'File', 'New…');
  const dialog = page.getByRole('dialog', { name: 'New document' });
  await dialog.getByRole('radio', { name: /^Image/ }).click();
  await dialog.getByRole('button', { name: 'Create' }).click();
  await waitForEditor(page, 'image');
  await paint(page, 'image');
  await menu(page, 'File', 'Open Recent', 'sprite.aseprite');
  await expect(prompt(page)).toBeVisible();
  await prompt(page).getByRole('button', { name: "Don't Save" }).click();
  await waitForEditor(page, 'pixel');
  await expect(page.getByTitle('sprite.aseprite', { exact: true })).toBeVisible();

  // A recent whose file is gone: error, no prompt.
  await paint(page, 'pixel');
  await putFile(page, '/docs/sprite.aseprite', null);
  // A recent re-opened through the shell is listed again under a new id
  // (as before this spec); either row points at the missing file.
  await page.getByRole('menubar').getByRole('menuitem', { name: 'File', exact: true }).click();
  await page.getByRole('menu', { name: 'File', exact: true }).getByRole('menuitem', { name: 'Open Recent' }).click();
  await page
    .getByRole('menu', { name: 'Open Recent', exact: true })
    .getByRole('menuitem', { name: 'sprite.aseprite', exact: true })
    .first()
    .click();
  await expect(page.getByText(/sprite\.aseprite.*No such file/)).toBeVisible();
  await expect(prompt(page)).toHaveCount(0);
  await expect(page.getByTitle('sprite.aseprite — unsaved changes')).toBeVisible();
});
