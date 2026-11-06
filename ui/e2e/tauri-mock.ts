import { expect, type Page } from '@playwright/test';

// A mocked Tauri bridge for e2e specs of the desktop paths: an init script
// sets `__TAURI_INTERNALS__`, which answers the IPC calls the UI makes
// (event listen, window destroy, native dialogs, file reads and writes over
// a virtual disk, the clipboard-manager plugin's image read) and lets the
// test emit events. The real window behavior on each OS stays a manual check.

export type WriteMode = 'write' | 'fail' | 'hold';

/** An image on the mocked system clipboard, as the plugin hands it over. */
export interface ClipImage {
  width: number;
  height: number;
  /** Straight RGBA, row-major; its length need not match (a broken image). */
  rgba: number[];
}

/** The mocked system clipboard: an image, no image, or a refused read. */
export type ClipContent = ClipImage | null | 'error';

export interface TauriMock {
  emit(event: string, payload: unknown): void;
  destroyed: number;
  writeMode: WriteMode;
  release: (() => void) | null;
  savePick: string | null;
  /** Answers for the next save dialogs (`null` is a cancel); `savePick`
   *  once the queue is empty. */
  saveQueue: (string | null)[];
  /** Write modes for the next writes; `writeMode` once the queue is empty. */
  writeQueue: WriteMode[];
  /** The path the native open dialog answers with; `null` is a cancel. */
  openPick: string | null;
  disk: Record<string, number[]>;
  /** What `readImage()` finds; `null` (no image) by default. */
  clipboard: ClipContent;
  /** How often the UI read the clipboard image. */
  clipboardReads: number;
  /** While set, clipboard reads wait in `clipboardHeld` (in start order). */
  holdClipboard: boolean;
  clipboardHeld: (() => void)[];
  images: Record<number, ClipImage>;
}

export type MockWindow = Window & { __mock: TauriMock };

/** Installs the mocked bridge before the app's scripts run. */
export async function installTauriMock(page: Page): Promise<void> {
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
      saveQueue: [],
      writeQueue: [],
      openPick: null,
      disk: {},
      clipboard: null,
      clipboardReads: 0,
      holdClipboard: false,
      clipboardHeld: [],
      images: {},
    };
    let nextRid = 1;
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
          return mock.saveQueue.length > 0 ? mock.saveQueue.shift() : mock.savePick;
        case 'plugin:dialog|open':
          return mock.openPick;
        case 'read_file_bytes': {
          const bytes = mock.disk[String(args.path)];
          if (!bytes) throw new Error('No such file or directory');
          return bytes;
        }
        case 'write_file_bytes': {
          const queued = mock.writeQueue.shift();
          if ((queued ?? mock.writeMode) === 'hold') {
            await new Promise<void>((resolve) => (mock.release = resolve));
          }
          // A test may switch `writeMode` while the write is held.
          if ((queued ?? mock.writeMode) === 'fail') throw new Error('disk full');
          mock.disk[String(args.path)] = args.bytes as number[];
          return null;
        }
        case 'plugin:clipboard-manager|read_image': {
          mock.clipboardReads += 1;
          // The native read happens now; only its answer is held.
          const content = mock.clipboard;
          if (mock.holdClipboard) {
            await new Promise<void>((resolve) => mock.clipboardHeld.push(resolve));
          }
          if (content === 'error') throw new Error('clipboard unavailable');
          if (content === null) {
            throw new Error('The clipboard contents were not available in the requested format');
          }
          const rid = nextRid++;
          mock.images[rid] = content;
          return rid;
        }
        case 'plugin:image|size': {
          const img = mock.images[Number(args.rid)];
          if (!img) throw new Error('unknown resource');
          return { width: img.width, height: img.height };
        }
        case 'plugin:image|rgba': {
          const img = mock.images[Number(args.rid)];
          if (!img) throw new Error('unknown resource');
          return img.rgba;
        }
        case 'plugin:resources|close':
          delete mock.images[Number(args.rid)];
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
}

/** Emits a Tauri event to the UI's listeners. */
export async function emit(page: Page, event: string, payload: unknown = null): Promise<void> {
  await page.evaluate(([e, p]) => (window as unknown as MockWindow).__mock.emit(e as string, p), [
    event,
    payload,
  ]);
}

/** The window's close button (title-bar ×, Alt+F4, the taskbar). */
export async function closeWindow(page: Page): Promise<void> {
  await emit(page, 'tauri://close-requested');
}

/** How often the UI destroyed the window (closed the app). */
export async function destroyed(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as MockWindow).__mock.destroyed);
}

export async function setMock(
  page: Page,
  patch: Partial<
    Pick<TauriMock, 'writeMode' | 'savePick' | 'openPick' | 'saveQueue' | 'writeQueue'>
  >,
): Promise<void> {
  await page.evaluate((p) => Object.assign((window as unknown as MockWindow).__mock, p), patch);
}

/** Puts `bytes` on the virtual disk at `path`; `null` removes the file. */
export async function putFile(page: Page, path: string, bytes: Buffer | null): Promise<void> {
  await page.evaluate(
    ([p, b]) => {
      const disk = (window as unknown as MockWindow).__mock.disk;
      if (b === null) delete disk[p as string];
      else disk[p as string] = b as number[];
    },
    [path, bytes === null ? null : Array.from(bytes)] as const,
  );
}

/** The paths written to the virtual disk. */
export async function diskPaths(page: Page): Promise<string[]> {
  return page.evaluate(() => Object.keys((window as unknown as MockWindow).__mock.disk));
}

/** Lets a held write (`writeMode: 'hold'`) finish. */
export async function releaseWrite(page: Page): Promise<void> {
  await page.waitForFunction(() => (window as unknown as MockWindow).__mock.release !== null);
  await page.evaluate(() => {
    const mock = (window as unknown as MockWindow).__mock;
    mock.release?.();
    mock.release = null;
  });
}

/** A `width`×`height` clipboard image of one straight RGBA color. */
export function solidImage(
  width: number,
  height: number,
  rgba: readonly [number, number, number, number] = [255, 0, 0, 255],
): ClipImage {
  const px: number[] = [];
  for (let i = 0; i < width * height; i += 1) px.push(...rgba);
  return { width, height, rgba: px };
}

/** Puts `content` on the mocked system clipboard (what the plugin reads). */
export async function setClipboard(page: Page, content: ClipContent): Promise<void> {
  await page.evaluate((c) => {
    (window as unknown as MockWindow).__mock.clipboard = c;
  }, content);
}

/** How often the UI read the mocked clipboard image. */
export async function clipboardReads(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as MockWindow).__mock.clipboardReads);
}

/** Holds the answers of the clipboard reads that start from now on. */
export async function holdClipboard(page: Page, hold: boolean): Promise<void> {
  await page.evaluate((h) => {
    (window as unknown as MockWindow).__mock.holdClipboard = h;
  }, hold);
}

/** Waits until `n` clipboard reads are held. */
export async function heldClipboardReads(page: Page, n: number): Promise<void> {
  await page.waitForFunction(
    (count) => (window as unknown as MockWindow).__mock.clipboardHeld.length >= count,
    n,
  );
}

/** Lets the held clipboard read at `index` (start order) answer. */
export async function releaseClipboardRead(page: Page, index: number): Promise<void> {
  await page.evaluate((i) => {
    const held = (window as unknown as MockWindow).__mock.clipboardHeld;
    const release = held[i];
    if (!release) throw new Error(`no held clipboard read ${i}`);
    held[i] = () => undefined;
    release();
  }, index);
}

/** The window gains focus (switched to, or restored from minimised). */
export async function focusWindow(page: Page): Promise<void> {
  await emit(page, 'tauri://focus');
}

/** Opens the app past the first-launch note (the mock makes it desktop). */
export async function launchDesktop(page: Page): Promise<void> {
  await page.goto('./');
  await page.getByRole('button', { name: 'Got it' }).click();
}

/** The bytes at `path` on the virtual disk, or `null`. */
export async function diskFile(page: Page, path: string): Promise<Buffer | null> {
  const bytes = await page.evaluate(
    (p) => (window as unknown as MockWindow).__mock.disk[p] ?? null,
    path,
  );
  return bytes === null ? null : Buffer.from(bytes);
}

/** Native File ▸ Save of a document whose target is `path` (or the save
 *  dialog's answer): waits until it wrote `path` and returns the bytes. */
export async function nativeSave(page: Page, path: string): Promise<Buffer> {
  await putFile(page, path, null);
  await emit(page, 'menu', 'menu:save');
  await expect.poll(async () => (await diskFile(page, path)) !== null).toBe(true);
  const bytes = await diskFile(page, path);
  if (!bytes) throw new Error(`${path} was not written`);
  return bytes;
}
