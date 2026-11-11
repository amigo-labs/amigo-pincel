import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, type Locator, type Page } from '@playwright/test';
import { pixelCanvas } from './helpers';

// Helpers for the system-fonts specs (docs/specs/system-fonts.md): a faked
// Local Font Access API with a counted `blob()`, the font fixtures, the font
// list, and reference pixels drawn by the cores in Node.

const FIXTURES = fileURLToPath(
  new URL('../../crates/pincel-core/tests/fixtures/', import.meta.url),
);

/** A fixture file's bytes. */
export async function fixtureBytes(name: string): Promise<Uint8Array> {
  return new Uint8Array(await readFile(join(FIXTURES, name)));
}

/** One face the faked `queryLocalFonts()` lists. */
export interface FakeFont {
  family: string;
  style: string;
  postscriptName: string;
  /** Fixture file `blob()` returns. */
  file: string;
}

/** The Pincel Test Serif family, one file per face. */
export const SERIF: FakeFont[] = [
  {
    family: 'Pincel Test Serif',
    style: 'Regular',
    postscriptName: 'PincelTestSerif-Regular',
    file: 'PincelTestSerif-Regular.ttf',
  },
  {
    family: 'Pincel Test Serif',
    style: 'Bold',
    postscriptName: 'PincelTestSerif-Bold',
    file: 'PincelTestSerif-Bold.ttf',
  },
  {
    family: 'Pincel Test Serif',
    style: 'Italic',
    postscriptName: 'PincelTestSerif-Italic',
    file: 'PincelTestSerif-Italic.ttf',
  },
  {
    family: 'Pincel Test Serif',
    style: 'Bold Italic',
    postscriptName: 'PincelTestSerif-BoldItalic',
    file: 'PincelTestSerif-BoldItalic.ttf',
  },
];

/** How the faked permission prompt is answered. */
export type PromptAnswer = 'grant' | 'deny' | 'dismiss';

export interface LocalFontsOptions {
  /** The permission state at page load. */
  permission: PermissionState;
  /** The answer to a prompt. */
  answer?: PromptAnswer;
  fonts: FakeFont[];
}

/** What the page records about the faked API. */
export interface LocalFontsLog {
  queries: number;
  /** Whether each query ran with transient user activation. */
  activations: boolean[];
  /** `blob()` calls per PostScript name. */
  reads: Record<string, number>;
  permission: PermissionState;
}

/** Fakes `queryLocalFonts()` and the `local-fonts` permission before the
 *  app loads; `window.__lfa` records queries and face reads. */
export async function installLocalFonts(page: Page, opts: LocalFontsOptions): Promise<void> {
  const files: Record<string, string> = {};
  for (const f of opts.fonts) {
    files[f.file] ??= Buffer.from(await fixtureBytes(f.file)).toString('base64');
  }
  await page.addInitScript(
    ({ fonts, files, permission, answer }) => {
      const log = {
        queries: 0,
        activations: [] as boolean[],
        reads: {} as Record<string, number>,
        permission,
        /** While set, `blob()` waits until `release()`. */
        hold: false,
        held: [] as (() => void)[],
        release() {
          log.hold = false;
          for (const r of log.held.splice(0)) r();
        },
      };
      (window as unknown as { __lfa: typeof log }).__lfa = log;
      const decode = (b64: string): Uint8Array<ArrayBuffer> =>
        Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
      const data = fonts.map((f) => ({
        family: f.family,
        style: f.style,
        fullName: `${f.family} ${f.style}`,
        postscriptName: f.postscriptName,
        blob: async () => {
          log.reads[f.postscriptName] = (log.reads[f.postscriptName] ?? 0) + 1;
          if (log.hold) await new Promise<void>((resolve) => log.held.push(resolve));
          return new Blob([decode(files[f.file] ?? '')]);
        },
      }));
      (window as unknown as { queryLocalFonts: () => Promise<unknown[]> }).queryLocalFonts = () => {
        log.queries += 1;
        log.activations.push(navigator.userActivation?.isActive ?? false);
        if (log.permission === 'granted') return Promise.resolve(data);
        if (log.permission === 'denied' || !navigator.userActivation?.isActive) {
          return Promise.reject(new DOMException('Permission denied', 'NotAllowedError'));
        }
        if (answer === 'grant') {
          log.permission = 'granted';
          return Promise.resolve(data);
        }
        if (answer === 'deny') log.permission = 'denied';
        return Promise.reject(new DOMException('Permission not granted', 'NotAllowedError'));
      };
      const query = navigator.permissions.query.bind(navigator.permissions);
      navigator.permissions.query = ((desc: PermissionDescriptor) =>
        (desc.name as string) === 'local-fonts'
          ? Promise.resolve({ state: log.permission } as PermissionStatus)
          : query(desc)) as typeof navigator.permissions.query;
    },
    { fonts: opts.fonts, files, permission: opts.permission, answer: opts.answer ?? 'grant' },
  );
}

/** Removes the Local Font Access API before the app loads (Firefox, Safari). */
export async function withoutLocalFonts(page: Page): Promise<void> {
  await page.addInitScript(() => {
    delete (window as unknown as { queryLocalFonts?: unknown }).queryLocalFonts;
  });
}

/** The page's record of the faked API. */
export async function lfaLog(page: Page): Promise<LocalFontsLog> {
  return page.evaluate(() => {
    const l = (window as unknown as { __lfa: LocalFontsLog }).__lfa;
    return {
      queries: l.queries,
      activations: l.activations,
      reads: l.reads,
      permission: l.permission,
    };
  });
}

/** Makes face reads wait until `releaseReads`. */
export async function holdReads(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as unknown as { __lfa: { hold: boolean } }).__lfa.hold = true;
  });
}

/** Lets held face reads finish. */
export async function releaseReads(page: Page): Promise<void> {
  await page.evaluate(() => {
    (window as unknown as { __lfa: { release(): void } }).__lfa.release();
  });
}

/** The family control of the Text tool on screen. */
export function fontControl(page: Page): Locator {
  return page.locator('[data-font-control]:visible');
}

/** The open font list. */
export function fontList(page: Page): Locator {
  return page.getByRole('dialog', { name: 'Fonts', exact: true });
}

/** The family entries of the open font list. */
export function fontEntries(page: Page): Locator {
  return fontList(page).getByRole('option');
}

/** Opens the font list. */
export async function openFontList(page: Page): Promise<void> {
  await fontControl(page).click();
  await expect(fontList(page)).toBeVisible();
}

/** Picks `family` from the font list. */
export async function pickFamily(page: Page, family: string): Promise<void> {
  if (!(await fontList(page).isVisible())) await openFontList(page);
  await fontEntries(page)
    .filter({ hasText: new RegExp(`^${family}$`) })
    .click();
  await expect(fontList(page)).toHaveCount(0);
  await expect(fontControl(page)).toHaveAttribute('aria-label', `Font: ${family}`);
}

/** Opens the Pixel Text dialog at sprite pixel `(x, y)` of a `size`² sprite. */
export async function openPixelText(page: Page, x = 4, y = 4, size = 32): Promise<Locator> {
  await page.keyboard.press('t');
  const box = await pixelCanvas(page).boundingBox();
  if (!box) throw new Error('canvas not visible');
  await page.mouse.click(
    box.x + ((x + 0.5) * box.width) / size,
    box.y + ((y + 0.5) * box.height) / size,
  );
  const dialog = page.getByRole('dialog', { name: 'Text' });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** The editor's status bar. */
export function statusBar(page: Page): Locator {
  return page.locator('footer:visible', { hasText: /\d+\s*×\s*\d+/ });
}

// --- Reference pixels from the cores, in Node ---------------------------

const require = createRequire(import.meta.url);

type PixelPkg = typeof import('@amigo-labs/pincel-wasm');
let pixelPkg: Promise<PixelPkg> | null = null;

/** `@amigo-labs/pincel-wasm`, initialized in Node. */
export function pixelCore(): Promise<PixelPkg> {
  pixelPkg ??= (async () => {
    const json = require.resolve('@amigo-labs/pincel-wasm/package.json');
    const pkg = (await import('@amigo-labs/pincel-wasm')) as PixelPkg;
    await pkg.default({
      module_or_path: await readFile(join(dirname(json), 'pincel_wasm_bg.wasm')),
    });
    return pkg;
  })();
  return pixelPkg;
}

/** Options of a reference text draw. */
export interface RefText {
  width: number;
  height: number;
  /** Fixture file and face index. */
  file: string;
  index?: number;
  text: string;
  x: number;
  y: number;
  size: number;
  /** 0xRRGGBBAA. */
  color: number;
  bold?: boolean;
  italic?: boolean;
}

type ImagePkg = typeof import('pincel-image-wasm');
let imagePkg: Promise<ImagePkg> | null = null;

/** `pincel-image-wasm`, initialized in Node. */
export function imageCore(): Promise<ImagePkg> {
  imagePkg ??= (async () => {
    const json = require.resolve('pincel-image-wasm/package.json');
    const pkg = (await import('pincel-image-wasm')) as ImagePkg;
    await pkg.default({
      module_or_path: await readFile(join(dirname(json), 'pincel_image_wasm_bg.wasm')),
    });
    return pkg;
  })();
  return imagePkg;
}

/** The RGBA pixels `pincel-image-core` draws for `r` (anti-aliased, as the
 *  Image Text tool does) on a new, transparent image. */
export async function imageReference(r: RefText): Promise<number[]> {
  const pkg = await imageCore();
  const font = pkg.register_font(await fixtureBytes(r.file), r.index ?? 0);
  const doc = pkg.create_document(r.width, r.height);
  try {
    pkg.apply_command(
      doc,
      JSON.stringify({
        type: 'draw_text',
        layer: 0,
        font_id: font,
        text: r.text,
        x: r.x,
        y: r.y,
        size: r.size,
        color: [
          (r.color >>> 24) & 255,
          (r.color >>> 16) & 255,
          (r.color >>> 8) & 255,
          r.color & 255,
        ],
        bold: r.bold ?? false,
        italic: r.italic ?? false,
        anti_alias: true,
        align: 'left',
      }),
    );
    return Array.from(pkg.composite(doc));
  } finally {
    pkg.close_document(doc);
  }
}

/**
 * Whether two RGBA images hold the same text: equal alpha everywhere and
 * equal color where opaque (a clipboard PNG decoded through a canvas may
 * round the color of partly transparent pixels).
 */
export function sameInk(a: readonly number[], b: readonly number[]): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 4) {
    if (a[i + 3] !== b[i + 3]) return false;
    if (a[i + 3] === 255 && (a[i] !== b[i] || a[i + 1] !== b[i + 1] || a[i + 2] !== b[i + 2]))
      return false;
  }
  return a.some((v, i) => i % 4 === 3 && v > 0);
}

/** The RGBA pixels `pincel-core` draws for `r` on an empty sprite layer. */
export async function pixelReference(r: RefText): Promise<number[]> {
  const pkg = await pixelCore();
  const font = pkg.registerFont(await fixtureBytes(r.file), r.index ?? 0);
  const doc = new pkg.Document(r.width, r.height);
  try {
    doc.drawText(
      font,
      r.text,
      r.x,
      r.y,
      r.size,
      r.color,
      r.bold ?? false,
      r.italic ?? false,
      false,
      'left',
    );
    const frame = doc.compose(0, 1);
    try {
      return Array.from(frame.pixels);
    } finally {
      frame.free();
    }
  } finally {
    doc.free();
  }
}
