import { expect, test, type Page } from '@playwright/test';
import { createDocument, drag, newDocument, pixelCanvas, byText } from './helpers';
import { PNG_1X1, tab } from './tab-helpers';
import {
  commandCalls,
  emit,
  installTauriMock,
  launchDesktop,
  putFile,
  reply,
  setMock,
  timedCommandCalls,
} from './tauri-mock';

// docs/specs/macos-native-menu.md: in the macOS desktop app the editor
// draws no menus; the native menu bar mirrors them. The bridge is mocked
// (tauri-mock.ts): `set_editor_menu` calls are recorded, native picks are
// `menu` events. How AppKit routes keys stays the manual AC-16.

const MAC_UA =
  'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';
const WINDOWS_UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';

type Node =
  | { kind: 'item'; id: string; label: string; enabled: boolean; checked?: boolean; accelerator?: string }
  | { kind: 'submenu'; label: string; enabled: boolean; items: Node[] }
  | { kind: 'separator' }
  | { kind: 'system'; role: string; label: string };

/** The menus of every `set_editor_menu` call so far (`null`: start screen). */
async function pushes(page: Page): Promise<(Node[] | null)[]> {
  const calls = await commandCalls(page, 'set_editor_menu');
  return calls.map((c) => (c.args.menus ?? null) as Node[] | null);
}

/** The newest menus sent, once some arrived. */
async function lastMenus(page: Page): Promise<Node[]> {
  let menus: Node[] | null = null;
  await expect
    .poll(async () => {
      menus = (await pushes(page)).at(-1) ?? null;
      return menus !== null;
    })
    .toBe(true);
  return menus as unknown as Node[];
}

function top(menus: Node[], label: string): Node[] {
  const m = menus.find((n) => n.kind === 'submenu' && n.label === label);
  if (!m || m.kind !== 'submenu') throw new Error(`no ${label} menu in ${tops(menus).join(', ')}`);
  return m.items;
}

function tops(menus: Node[]): string[] {
  return menus.map((n) => (n.kind === 'submenu' ? n.label : n.kind));
}

function rows(items: Node[]): string[] {
  return items.map((n) =>
    n.kind === 'separator' ? '---' : n.kind === 'system' ? `<${n.role}>` : n.label,
  );
}

/** The entry `label` of menu `menuLabel` (searching its submenus too). */
function entry(menus: Node[], menuLabel: string, label: string): Node {
  const walk = (items: Node[]): Node | undefined => {
    for (const n of items) {
      if (n.kind !== 'separator' && n.label === label) return n;
      if (n.kind === 'submenu') {
        const hit = walk(n.items);
        if (hit) return hit;
      }
    }
    return undefined;
  };
  const hit = walk(top(menus, menuLabel));
  if (!hit) throw new Error(`no ${menuLabel} ▸ ${label}`);
  return hit;
}

function item(menus: Node[], menuLabel: string, label: string) {
  const n = entry(menus, menuLabel, label);
  if (n.kind !== 'item') throw new Error(`${label} is a ${n.kind}`);
  return n;
}

/** Every item below `items`, submenus included. */
function allItems(items: Node[]): Extract<Node, { kind: 'item' }>[] {
  return items.flatMap((n) =>
    n.kind === 'item' ? [n] : n.kind === 'submenu' ? allItems(n.items) : [],
  );
}

/** Picks a mirrored entry, as a click in the native menu does. */
async function pick(page: Page, menuLabel: string, label: string): Promise<void> {
  await emit(page, 'menu', item(await lastMenus(page), menuLabel, label).id);
}

/** Waits for a push whose menus satisfy `check`. */
async function menusWhere(page: Page, check: (menus: Node[]) => boolean): Promise<Node[]> {
  let found: Node[] | null = null;
  await expect
    .poll(async () => {
      const last = (await pushes(page)).at(-1) ?? null;
      found = last !== null && check(last) ? last : null;
      return found !== null;
    })
    .toBe(true);
  return found as unknown as Node[];
}

async function stroke(page: Page): Promise<void> {
  await page.getByRole('button', { name: /^Pencil/ }).first().click();
  await drag(page, pixelCanvas(page), [0.2, 0.5], [0.8, 0.5]);
}

const undoButton = (page: Page) =>
  page.locator('[data-tour="quick"]').getByRole('button', { name: 'Undo', exact: true });

/** Stores keymap overrides and reloads into a new Pixel document. */
async function withKeymap(page: Page, overrides: Record<string, unknown>): Promise<void> {
  await page.evaluate(async (value) => {
    await new Promise<void>((resolve, reject) => {
      const open = indexedDB.open('pincel');
      open.onerror = () => reject(open.error);
      open.onsuccess = () => {
        const tx = open.result.transaction('prefs', 'readwrite');
        tx.objectStore('prefs').put({ key: 'keymap', value });
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      };
    });
  }, overrides);
  await page.reload();
  await page.getByRole('button', { name: 'Got it' }).click();
  await newDocument(page, 'pixel', { width: 32, height: 32 });
}

test.describe('macOS desktop app', () => {
  test.use({ userAgent: MAC_UA });
  test.beforeEach(async ({ page }) => {
    await installTauriMock(page);
    await launchDesktop(page);
  });

  test('no in-window menus at any width; title, mode and quick buttons stay (AC-01)', async ({
    page,
  }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await expect(page.getByRole('menubar')).toHaveCount(0);
    const header = page.locator('header').first();
    await expect(header.getByText('Pincel', { exact: true })).toBeVisible();
    await expect(header.getByText('Pixel', { exact: true })).toBeVisible();
    await expect(header.getByTitle('pincel.aseprite', { exact: true })).toBeVisible();
    await expect(undoButton(page)).toBeVisible();
    await page.setViewportSize({ width: 520, height: 800 });
    await expect(page.getByRole('menubar')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Menu', exact: true })).toHaveCount(0);
  });

  test('pixel: the native bar lists the editor menus in order, Help last (AC-03, AC-08)', async ({
    page,
  }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    const menus = await lastMenus(page);
    expect(tops(menus)).toEqual([
      'File',
      'Edit',
      'Select',
      'Image',
      'Layer',
      'Frame',
      'Effects',
      'Adjustments',
      'View',
      'Tools',
      'Help',
    ]);
    // Submenus come along (Open Recent, Export); dimmed state too.
    expect(entry(menus, 'File', 'Export').kind).toBe('submenu');
    expect(entry(menus, 'File', 'Open Recent').kind).toBe('submenu');
    expect(item(menus, 'Edit', 'Undo').enabled).toBe(false);
    expect(item(menus, 'Select', 'Deselect').enabled).toBe(false);
    // Settings… moved to the App menu, About and updates too (AC-07, AC-08).
    expect(rows(top(menus, 'Edit'))).not.toContain('Settings…');
    expect(rows(top(menus, 'Help'))).toEqual([
      'Keyboard Shortcuts',
      'Tour',
      'Frame-Time Probe',
    ]);
    expect(item(menus, 'Help', 'Frame-Time Probe').checked).toBe(false);
  });

  test('image: Help has no probe; View holds its own aids (AC-03, AC-08)', async ({ page }) => {
    await newDocument(page, 'image', { width: 32, height: 32 });
    const menus = await lastMenus(page);
    expect(tops(menus)).not.toContain('Frame');
    expect(rows(top(menus, 'Help'))).toEqual(['Keyboard Shortcuts', 'Tour']);
    expect(rows(top(menus, 'View'))).toContain('Rulers');
  });

  test('a picked entry runs its action once; a state change reaches the bar (AC-04, AC-05)', async ({
    page,
  }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await pick(page, 'Layer', 'New Layer');
    await expect(page.getByRole('button', { name: 'Layer 2', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Layer 3', exact: true })).toHaveCount(0);

    // A check mark flips within 200 ms of the change.
    const before = await page.evaluate(() => performance.now());
    await pick(page, 'Help', 'Frame-Time Probe');
    await menusWhere(page, (m) => item(m, 'Help', 'Frame-Time Probe').checked === true);
    const calls = await timedCommandCalls(page, 'set_editor_menu');
    const flipped = calls.find(
      (c) =>
        c.t > before &&
        item(c.args.menus as Node[], 'Help', 'Frame-Time Probe').checked === true,
    );
    expect(flipped).toBeDefined();
    expect((flipped?.t ?? Infinity) - before).toBeLessThan(200);

    // Undo becomes usable after a stroke.
    await stroke(page);
    await menusWhere(page, (m) => item(m, 'Edit', 'Undo').enabled);
  });

  test('switching to a tab of the other mode switches the menus (AC-05)', async ({ page }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await menusWhere(page, (m) => tops(m).includes('Frame'));
    await putFile(page, '/b.png', PNG_1X1);
    await setMock(page, { openPick: '/b.png' });
    // The shell's Open routes a PNG to Image mode.
    await emit(page, 'menu', 'menu:open');
    await menusWhere(page, (m) => !tops(m).includes('Frame') && tops(m).includes('Effects'));
    await tab(page, 'pincel.aseprite').click();
    await menusWhere(page, (m) => tops(m).includes('Frame'));
  });

  test('the language follows into the native bar (AC-05)', async ({ page }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await emit(page, 'menu', 'menu:settings');
    const dialog = page.getByRole('dialog', { name: /^(Settings|Einstellungen)$/ });
    await dialog.getByRole('combobox', { name: 'Language' }).selectOption({ label: 'Deutsch' });
    await dialog.getByRole('button', { name: 'Schließen' }).click();
    const menus = await menusWhere(page, (m) => tops(m)[0] === 'Datei');
    expect(tops(menus)).toContain('Bearbeiten');
  });

  test('an open effect dims every menu but Effects and Adjustments (AC-06)', async ({ page }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    const menus = await lastMenus(page);
    const effect = allItems(top(menus, 'Effects')).find((i) => i.enabled);
    if (!effect) throw new Error('no usable effect');
    await emit(page, 'menu', effect.id);
    const dimmed = await menusWhere(page, (m) => !item(m, 'Layer', 'New Layer').enabled);
    for (const name of ['File', 'Edit', 'Select', 'Image', 'Layer', 'Frame', 'View', 'Tools']) {
      expect(allItems(top(dimmed, name)).filter((i) => i.enabled), name).toEqual([]);
    }
    // Help stays usable, as the App menu does.
    expect(allItems(top(dimmed, 'Help')).every((i) => i.enabled)).toBe(true);
    // A pick of a dimmed entry does nothing (AC-17).
    await emit(page, 'menu', item(dimmed, 'Layer', 'New Layer').id);
    await expect(page.getByRole('button', { name: 'Layer 2', exact: true })).toHaveCount(0);
  });

  test('modifier chords show and belong to the menu; single keys stay with the editor (AC-10)', async ({
    page,
  }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    const menus = await lastMenus(page);
    expect(item(menus, 'Edit', 'Undo').accelerator).toBe('CmdOrCtrl+Z');
    expect(item(menus, 'File', 'Save').accelerator).toBe('CmdOrCtrl+S');
    expect(item(menus, 'Select', 'Deselect').accelerator).toBe('CmdOrCtrl+D');
    expect(item(menus, 'Edit', 'Copy').accelerator).toBe('CmdOrCtrl+C');
    // Paste is the system's item: the editors take its `paste` event.
    expect(entry(menus, 'Edit', 'Paste')).toMatchObject({ kind: 'system', role: 'paste' });
    // Single-key and unexpressible shortcuts show nothing.
    expect(item(menus, 'Edit', 'Swap Colors').accelerator).toBeUndefined();
    expect(item(menus, 'Help', 'Frame-Time Probe').accelerator).toBeUndefined();
    expect(item(menus, 'View', 'Zoom In').accelerator).toBeUndefined();

    // ⌘Z is the menu's: the editor leaves it; the native entry undoes once.
    await stroke(page);
    await expect(undoButton(page)).toBeEnabled();
    await page.keyboard.press('Meta+z');
    await expect(undoButton(page)).toBeEnabled();
    await menusWhere(page, (m) => item(m, 'Edit', 'Undo').enabled);
    await pick(page, 'Edit', 'Undo');
    await expect(undoButton(page)).toBeDisabled();
    // Redo's second chord ⌘Y is not the menu's: the editor redoes.
    await page.keyboard.press('Meta+y');
    await expect(undoButton(page)).toBeEnabled();
    // F2 (no native chord) still reaches the editor.
    await page.keyboard.press('F2');
    await expect(byText(page, /fps · compose/)).toBeVisible();
  });

  test('a rebound action shows its new chord; one rebound to a single key goes back to the editor (AC-11)', async ({
    page,
  }) => {
    await withKeymap(page, {
      undo: [{ key: 'u', mod: true }],
      deselect: [{ key: 'q' }],
      settings: [{ key: 'j', mod: true }],
    });
    const menus = await lastMenus(page);
    expect(item(menus, 'Edit', 'Undo').accelerator).toBe('CmdOrCtrl+U');
    expect(item(menus, 'Select', 'Deselect').accelerator).toBeUndefined();
    // Settings… is in the App menu, which the shell builds with the keymap's chord.
    await stroke(page);
    await page.keyboard.press('Meta+u');
    await expect(undoButton(page)).toBeEnabled();
  });

  test('a rebind onto an App menu chord or a taken chord shows none (AC-18)', async ({ page }) => {
    await withKeymap(page, {
      undo: [{ key: 'q', mod: true }],
      redo: [{ key: 'd', mod: true }],
    });
    const menus = await lastMenus(page);
    expect(item(menus, 'Edit', 'Undo').accelerator).toBeUndefined();
    // Redo (Edit) comes before Deselect (Select): Redo keeps ⌘D.
    expect(item(menus, 'Edit', 'Redo').accelerator).toBe('CmdOrCtrl+D');
    expect(item(menus, 'Select', 'Deselect').accelerator).toBeUndefined();
  });

  test('a focused text field gets the system text items and keeps only the old chords (AC-12, AC-13)', async ({
    page,
  }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await lastMenus(page);
    const field = page.getByRole('spinbutton').first();
    await field.focus();
    const typing = await menusWhere(page, (m) => entry(m, 'Edit', 'Copy').kind === 'system');
    expect(entry(typing, 'Edit', 'Cut')).toMatchObject({ kind: 'system', role: 'cut' });
    expect(entry(typing, 'Edit', 'Paste')).toMatchObject({ kind: 'system', role: 'paste' });
    expect(entry(typing, 'Select', 'Select All')).toMatchObject({ kind: 'system', role: 'selectAll' });
    expect(item(typing, 'Edit', 'Undo').accelerator).toBe('CmdOrCtrl+Z');
    expect(item(typing, 'File', 'Save').accelerator).toBe('CmdOrCtrl+S');
    expect(item(typing, 'Select', 'Deselect').accelerator).toBeUndefined();
    await field.blur();
    const back = await menusWhere(page, (m) => entry(m, 'Edit', 'Copy').kind === 'item');
    expect(item(back, 'Select', 'Deselect').accelerator).toBe('CmdOrCtrl+D');
  });

  test('a failed update keeps the editor working and its keys with the editor (AC-14)', async ({
    page,
  }) => {
    await reply(page, 'set_editor_menu', { __error: 'boom' });
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await expect.poll(async () => (await pushes(page)).length).toBeGreaterThan(0);
    await expect(page.getByRole('menubar')).toHaveCount(0);
    // No menu took ⌘Z, so the editor undoes.
    await stroke(page);
    await page.keyboard.press('Meta+z');
    await expect(undoButton(page)).toBeDisabled();
    // The next change sends again, and this time it lands.
    await reply(page, 'set_editor_menu', null);
    const sent = (await pushes(page)).length;
    await stroke(page);
    await expect.poll(async () => (await pushes(page)).length).toBeGreaterThan(sent);
    await page.keyboard.press('Meta+z');
    await expect(undoButton(page)).toBeEnabled();
  });

  test('closing the last tab brings back the start-screen menu (AC-15)', async ({ page }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await lastMenus(page);
    await pick(page, 'File', 'Close');
    await expect(page.getByRole('button', { name: 'New pixel sprite' })).toBeVisible();
    await expect.poll(async () => (await pushes(page)).at(-1)).toBeNull();
    await newDocument(page, 'image', { width: 32, height: 32 });
    await menusWhere(page, (m) => tops(m).includes('Effects'));
  });

  test('a stale entry id does nothing (AC-17)', async ({ page }) => {
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await lastMenus(page);
    await emit(page, 'menu', 'ed:layer.99|Nothing');
    await emit(page, 'menu', item(await lastMenus(page), 'Edit', 'Undo').id);
    await expect(undoButton(page)).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Layer 1', exact: true })).toBeVisible();
  });
});

test.describe('elsewhere', () => {
  test('a browser on macOS keeps the in-window bar with Help (AC-02)', async ({ browser }) => {
    const context = await browser.newContext({ userAgent: MAC_UA });
    const page = await context.newPage();
    await createDocument(page, 'pixel', { width: 32, height: 32 });
    await expect(page.getByRole('menubar')).toBeVisible();
    await expect(
      page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }),
    ).toBeVisible();
    await context.close();
  });

  test('the Windows desktop app keeps the in-window bar and sends no menus (AC-02)', async ({
    browser,
  }) => {
    const context = await browser.newContext({ userAgent: WINDOWS_UA });
    const page = await context.newPage();
    await installTauriMock(page);
    await launchDesktop(page);
    await newDocument(page, 'pixel', { width: 32, height: 32 });
    await expect(
      page.getByRole('menubar').getByRole('menuitem', { name: 'Help', exact: true }),
    ).toBeVisible();
    expect(await pushes(page)).toEqual([]);
    await context.close();
  });
});
