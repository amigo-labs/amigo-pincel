import { readFileSync } from 'node:fs';
import { expect, type Locator, type Page } from '@playwright/test';
import { storedPref } from './helpers';

// Helpers of the specs for docs/specs/amigo-theme-controls.md: what a
// control looks like (computed fill, text colour, outline, radius, height),
// and the same for a shared component rendered alone from the theme files.

export const PALETTES = ['Dark', 'Light'] as const;
export type Palette = (typeof PALETTES)[number];

/** What a control looks like, as computed style. */
export interface Look {
  fill: string;
  text: string;
  /** Colour of the 1px edge (the top border). */
  edge: string;
  edgeWidth: string;
  radius: string;
  height: number;
}

/** A state on top of the look: what hover, pressed, focus or disabled may change. */
export interface State extends Look {
  /** Colour and width of the bottom edge (a tab's underline). */
  underline: string;
  shadow: string;
  outline: string;
  opacity: string;
  filter: string;
}

function read(node: Element): State {
  const cs = getComputedStyle(node);
  return {
    fill: cs.backgroundColor,
    text: cs.color,
    edge: cs.borderTopColor,
    edgeWidth: cs.borderTopWidth,
    radius: cs.borderTopLeftRadius,
    height: Math.round(node.getBoundingClientRect().height * 100) / 100,
    underline: `${cs.borderBottomWidth} ${cs.borderBottomColor}`,
    shadow: cs.boxShadow,
    outline: cs.outlineStyle === 'none' ? 'none' : `${cs.outlineWidth} ${cs.outlineStyle} ${cs.outlineColor}`,
    opacity: cs.opacity,
    filter: cs.filter,
  };
}

/** The control's look with its state (outline, opacity, filter). */
export async function stateOf(el: Locator): Promise<State> {
  return el.evaluate(read);
}

/** The control's look: fill, text colour, edge, radius, height. */
export async function lookOf(el: Locator): Promise<Look> {
  const { fill, text, edge, edgeWidth, radius, height } = await stateOf(el);
  return { fill, text, edge, edgeWidth, radius, height };
}

/** A pseudo-element's computed properties (`::after` of a checked toggle). */
export async function pseudoOf(
  el: Locator,
  pseudo: string,
): Promise<{ fill: string; edge: string; content: string }> {
  return el.evaluate((node, p) => {
    const cs = getComputedStyle(node, p);
    return { fill: cs.backgroundColor, edge: cs.borderLeftColor, content: cs.content };
  }, pseudo);
}

/** The theme token `--am-<name>` as the browser computes it for a colour. */
export async function tokenColor(page: Page, name: string): Promise<string> {
  return page.evaluate((n) => {
    const probe = document.createElement('span');
    probe.style.color = `var(--am-${n})`;
    document.body.append(probe);
    const color = getComputedStyle(probe).color;
    probe.remove();
    return color;
  }, name);
}

/** The theme token `--am-<name>` as written (a size, a shadow). */
export async function tokenValue(page: Page, name: string): Promise<string> {
  return page.evaluate(
    (n) => getComputedStyle(document.documentElement).getPropertyValue(`--am-${n}`).trim(),
    name,
  );
}

/** Picks a theme in Settings and closes the dialog; waits for the pref. */
export async function pickTheme(page: Page, name: Palette | 'System'): Promise<void> {
  const settings = page.getByRole('dialog', { name: 'Settings', exact: true });
  await page.keyboard.press('Control+,');
  await settings
    .getByRole('radiogroup', { name: 'Theme' })
    .getByRole('radio', { name, exact: true })
    .check();
  await expect.poll(() => storedPref(page, 'theme')).toBe(name.toLowerCase());
  await settings.getByRole('button', { name: 'Close' }).click();
  await expect(settings).toBeHidden();
}

/** Moves keyboard focus to `el` the way a key does, so `:focus-visible` applies. */
export async function keyFocus(page: Page, el: Locator): Promise<void> {
  await page.keyboard.press('Shift');
  await el.focus();
  await expect(el).toBeFocused();
}

const THEME_CSS = ['src/theme/tokens.css', 'src/theme/components.css']
  .map((file) => readFileSync(file, 'utf8'))
  .join('\n');

/**
 * A page that holds nothing but the theme files (Playwright runs in `ui/`)
 * and `html`: the shared components rendered alone, in `palette`.
 */
export async function sharedPage(page: Page, palette: Palette, html: string): Promise<Page> {
  const alone = await page.context().newPage();
  await alone.setContent(
    `<!doctype html><html data-theme="${palette.toLowerCase()}"><head><style>${THEME_CSS}</style></head>` +
      `<body><div style="padding:24px;width:640px">${html}</div></body></html>`,
  );
  return alone;
}

/** The pixel of the page at a CSS position, as `[r, g, b]`. */
export async function pixelAt(page: Page, x: number, y: number): Promise<[number, number, number]> {
  const png = await page.screenshot({ clip: { x: Math.floor(x), y: Math.floor(y), width: 1, height: 1 } });
  return page.evaluate(async (b64) => {
    const blob = await (await fetch(`data:image/png;base64,${b64}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 1;
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(bitmap, 0, 0);
    const d = ctx.getImageData(0, 0, 1, 1).data;
    return [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0] as [number, number, number];
  }, png.toString('base64'));
}

/** `rgb(r, g, b)` as numbers. */
export function rgb(css: string): [number, number, number] {
  const m = /rgba?\((\d+), (\d+), (\d+)/.exec(css);
  if (!m) throw new Error(`not an rgb() colour: ${css}`);
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

/** Two colours are the same within one step of rounding per channel. */
export function near(a: [number, number, number], b: [number, number, number], by = 2): boolean {
  return a.every((v, i) => Math.abs(v - (b[i] ?? 0)) <= by);
}
