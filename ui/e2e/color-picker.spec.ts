import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, imageCanvas, pixelCanvas } from './helpers';

// The own color picker (color-picker spec): a popover from the foreground
// and background swatches with an S/V area, hue and alpha sliders, a hex
// field, R / G / B / A fields and the recent colors, in both modes.

type Mode = 'pixel' | 'image';

const swatch = (page: Page, which: 'fg' | 'bg') =>
  page.getByRole('button', {
    name: which === 'fg' ? 'Foreground color' : 'Background color',
    exact: true,
  });

const pickerOf = (page: Page, which: 'fg' | 'bg') =>
  page.getByRole('dialog', {
    name: which === 'fg' ? 'Foreground color picker' : 'Background color picker',
  });

async function openPicker(page: Page, which: 'fg' | 'bg' = 'fg'): Promise<Locator> {
  await swatch(page, which).click();
  const picker = pickerOf(page, which);
  await expect(picker).toBeVisible();
  return picker;
}

/** The R, G, B and A fields' values. */
async function rgba(picker: Locator): Promise<number[]> {
  const out: number[] = [];
  for (const name of ['Red', 'Green', 'Blue', 'Alpha value']) {
    out.push(Number(await picker.getByRole('spinbutton', { name, exact: true }).inputValue()));
  }
  return out;
}

async function commitHex(picker: Locator, text: string): Promise<void> {
  const hex = picker.getByRole('textbox', { name: 'Hex color' });
  await hex.fill(text);
  await hex.press('Enter');
}

/** A point `f` (0–1, may lie outside) across `el`, vertically centred. */
async function across(el: Locator, f: number): Promise<[number, number]> {
  const b = await el.boundingBox();
  if (!b) throw new Error('not visible');
  return [b.x + b.width * f, b.y + b.height / 2];
}

/** A pen or touch drag from `from` to `to` through real CDP input. */
async function cdpDrag(
  page: Page,
  kind: 'pen' | 'touch',
  from: [number, number],
  to: [number, number],
): Promise<void> {
  const cdp = await page.context().newCDPSession(page);
  const steps = [0.25, 0.5, 0.75, 1].map(
    (f) => [from[0] + (to[0] - from[0]) * f, from[1] + (to[1] - from[1]) * f] as const,
  );
  if (kind === 'touch') {
    const at = (x: number, y: number) => [{ x, y, id: 1 }];
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: at(...from) });
    for (const [x, y] of steps) {
      await cdp.send('Input.dispatchTouchEvent', { type: 'touchMove', touchPoints: at(x, y) });
    }
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
  } else {
    const pen = { button: 'left', clickCount: 1, pointerType: 'pen', force: 0.5 } as const;
    await cdp.send('Input.dispatchMouseEvent', {
      ...pen,
      type: 'mousePressed',
      x: from[0],
      y: from[1],
      buttons: 1,
    });
    for (const [x, y] of steps) {
      await cdp.send('Input.dispatchMouseEvent', { ...pen, type: 'mouseMoved', x, y, buttons: 1 });
    }
    await cdp.send('Input.dispatchMouseEvent', {
      ...pen,
      type: 'mouseReleased',
      x: to[0],
      y: to[1],
      buttons: 0,
    });
  }
  await cdp.detach();
}

for (const mode of ['pixel', 'image'] as const satisfies readonly Mode[]) {
  test(`${mode}: the swatches open the own picker, never a native dialog (AC-01, AC-02, AC-03)`, async ({
    page,
  }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    await expect(page.locator('input[type="color"]')).toHaveCount(0);

    // Each swatch opens it next to itself, inside the window (AC-20: the
    // swatches sit at the window's right edge).
    const win = page.viewportSize();
    if (!win) throw new Error('no viewport');
    for (const which of ['bg', 'fg'] as const) {
      const opened = await openPicker(page, which);
      const s = await swatch(page, which).boundingBox();
      const p = await opened.boundingBox();
      if (!s || !p) throw new Error('not visible');
      expect(Math.abs(p.y - s.y), which).toBeLessThan(80);
      expect(p.x >= s.x + s.width || p.x + p.width <= s.x, which).toBe(true);
      expect(p.x).toBeGreaterThanOrEqual(0);
      expect(p.y).toBeGreaterThanOrEqual(0);
      expect(p.x + p.width).toBeLessThanOrEqual(win.width);
      expect(p.y + p.height).toBeLessThanOrEqual(win.height);
      if (which === 'bg') {
        await page.keyboard.press('Escape');
        await expect(opened).toHaveCount(0);
      }
    }
    const picker = pickerOf(page, 'fg');
    // Every control with its name.
    await expect(picker.getByRole('slider', { name: 'Saturation and value' })).toBeVisible();
    await expect(picker.getByRole('slider', { name: 'Hue', exact: true })).toBeVisible();
    await expect(picker.getByRole('slider', { name: 'Alpha', exact: true })).toBeVisible();
    await expect(picker.getByRole('textbox', { name: 'Hex color' })).toBeVisible();
    for (const name of ['Red', 'Green', 'Blue', 'Alpha value']) {
      await expect(picker.getByRole('spinbutton', { name, exact: true })).toBeVisible();
    }
    await expect(picker.getByRole('group', { name: 'Recent colors' })).toBeAttached();

    // It edits the foreground; the background picker edits the background.
    await commitHex(picker, '#11223380');
    expect(await rgba(picker)).toEqual([17, 34, 51, 128]);
    await page.keyboard.press('Escape');
    await expect(picker).toHaveCount(0);
    await expect(page.getByLabel('Foreground color hex value')).toHaveValue('#112233');

    const bgPicker = await openPicker(page, 'bg');
    expect(await rgba(bgPicker)).toEqual([255, 255, 255, 255]);
    await commitHex(bgPicker, '#0000ff');
    await page.keyboard.press('Escape');
    const again = await openPicker(page, 'fg');
    expect(await rgba(again)).toEqual([17, 34, 51, 128]);
    await page.keyboard.press('Escape');
    expect(await rgba(await openPicker(page, 'bg'))).toEqual([0, 0, 255, 255]);
  });
}

test('hex forms, invalid text and the alpha rule (AC-04, AC-05, AC-06, AC-23)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const picker = await openPicker(page);
  await commitHex(picker, '#11223380');
  expect(await rgba(picker)).toEqual([17, 34, 51, 128]);
  // 3 and 6 digits keep the alpha.
  await commitHex(picker, '#abc');
  expect(await rgba(picker)).toEqual([0xaa, 0xbb, 0xcc, 128]);
  await commitHex(picker, '445566');
  expect(await rgba(picker)).toEqual([0x44, 0x55, 0x66, 128]);
  await commitHex(picker, '#AABBCCFF');
  expect(await rgba(picker)).toEqual([0xaa, 0xbb, 0xcc, 255]);
  // Invalid text changes nothing.
  for (const bad of ['#12345', 'zzz', '']) {
    await commitHex(picker, bad);
    expect(await rgba(picker), bad).toEqual([0xaa, 0xbb, 0xcc, 255]);
  }
});

test('R / G / B / A fields clamp to 0–255 (AC-07)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  const picker = await openPicker(page);
  const field = (name: string) => picker.getByRole('spinbutton', { name, exact: true });
  for (const [name, typed, shown] of [
    ['Red', '300', '255'],
    ['Green', '-1', '0'],
    ['Blue', '256', '255'],
    ['Alpha value', '-4', '0'],
  ] as const) {
    await field(name).fill(typed);
    await field(name).press('Enter');
    await expect(field(name), name).toHaveValue(shown);
  }
  expect(await rgba(picker)).toEqual([255, 0, 255, 0]);
  await commitHex(picker, '#80808080');
  for (const [name, typed, shown] of [
    ['Red', '-9', '0'],
    ['Green', '999', '255'],
    ['Blue', '-300', '0'],
    ['Alpha value', '300', '255'],
  ] as const) {
    await field(name).fill(typed);
    await field(name).press('Enter');
    await expect(field(name), name).toHaveValue(shown);
  }
  expect(await rgba(picker)).toEqual([0, 255, 0, 255]);
});

test('a drag in the S/V area updates the color live, with mouse, pen and touch (AC-08, AC-09)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const picker = await openPicker(page);
  await commitHex(picker, '#ff0000');
  const area = picker.getByRole('slider', { name: 'Saturation and value' });
  const box = await area.boundingBox();
  if (!box) throw new Error('not visible');
  const panelHex = page.getByLabel('Foreground color hex value');

  // Mouse: the swatch follows before the release.
  await page.mouse.move(box.x + box.width - 2, box.y + 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 4 });
  await expect(panelHex).not.toHaveValue('#ff0000');
  const mid = await panelHex.inputValue();
  // Past the corner: the drag keeps the pointer and clamps.
  await page.mouse.move(box.x - 20, box.y + box.height + 20, { steps: 4 });
  await expect(panelHex).toHaveValue('#000000');
  await page.mouse.up();
  expect(mid).not.toBe('#000000');
  // Picking adds nothing to the recent colors (AC-08, AC-24).
  await expect(
    picker.getByRole('group', { name: 'Recent colors' }).getByRole('button'),
  ).toHaveCount(0);

  // Pen and touch (real CDP input) reach the top-right corner: pure hue.
  const cdp = await page.context().newCDPSession(page);
  const [x0, y0, x1, y1] = [
    box.x + 10,
    box.y + box.height - 10,
    box.x + box.width + 20,
    box.y - 20,
  ];
  for (const type of ['mousePressed', 'mouseMoved', 'mouseReleased'] as const) {
    const end = type !== 'mousePressed';
    await cdp.send('Input.dispatchMouseEvent', {
      type,
      x: end ? x1 : x0,
      y: end ? y1 : y0,
      button: 'left',
      buttons: type === 'mouseReleased' ? 0 : 1,
      clickCount: 1,
      pointerType: 'pen',
      force: 0.5,
    });
  }
  await expect(panelHex).toHaveValue('#ff0000');
  await cdp.detach();
});

test('a drag on the hue or alpha slider updates the color live, also with a pen (AC-08, AC-09)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const picker = await openPicker(page);
  await commitHex(picker, '#ff0000');
  const panelHex = page.getByLabel('Foreground color hex value');
  const panelAlpha = page.getByRole('slider', { name: 'Foreground alpha' });
  const hue = picker.getByRole('slider', { name: 'Hue', exact: true });
  const alpha = picker.getByRole('slider', { name: 'Alpha', exact: true });

  // Mouse: the panel follows each slider before the release.
  await page.mouse.move(...(await across(hue, 0.01)));
  await page.mouse.down();
  await page.mouse.move(...(await across(hue, 0.5)), { steps: 4 });
  await expect(panelHex).not.toHaveValue('#ff0000');
  expect(Number(await hue.inputValue())).toBeGreaterThan(90);
  await page.mouse.up();
  await page.mouse.move(...(await across(alpha, 0.99)));
  await page.mouse.down();
  await page.mouse.move(...(await across(alpha, 0.3)), { steps: 4 });
  await expect.poll(async () => Number(await panelAlpha.inputValue())).toBeLessThan(200);
  await page.mouse.up();
  await expect(
    picker.getByRole('group', { name: 'Recent colors' }).getByRole('button'),
  ).toHaveCount(0);

  // Pen: back to pure red and full alpha, past the slider ends.
  await cdpDrag(page, 'pen', await across(hue, 0.5), await across(hue, -0.2));
  await expect(panelHex).toHaveValue('#ff0000');
  await cdpDrag(page, 'pen', await across(alpha, 0.3), await across(alpha, 1.2));
  await expect(panelAlpha).toHaveValue('255');
});

test.describe('touch', () => {
  test.use({ hasTouch: true });

  test('a touch drag on the S/V area moves the color (AC-09)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    const picker = await openPicker(page);
    await commitHex(picker, '#ff0000');
    const box = await picker.getByRole('slider', { name: 'Saturation and value' }).boundingBox();
    if (!box) throw new Error('not visible');
    const cdp = await page.context().newCDPSession(page);
    const at = (x: number, y: number) => [{ x, y, id: 1 }];
    await cdp.send('Input.dispatchTouchEvent', {
      type: 'touchStart',
      touchPoints: at(box.x + box.width - 2, box.y + 2),
    });
    for (const f of [0.25, 0.5, 0.75, 1]) {
      await cdp.send('Input.dispatchTouchEvent', {
        type: 'touchMove',
        touchPoints: at(box.x + box.width - 2 - f * (box.width + 20), box.y - 10),
      });
    }
    await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
    await cdp.detach();
    // The left edge has no saturation: white.
    expect(await rgba(picker)).toEqual([255, 255, 255, 255]);
  });

  test('a touch drag on the hue and alpha sliders moves them (AC-09)', async ({ page }) => {
    await createDocument(page, 'image', { width: 16, height: 16 });
    const picker = await openPicker(page);
    await commitHex(picker, '#ff0000');
    const hue = picker.getByRole('slider', { name: 'Hue', exact: true });
    const alpha = picker.getByRole('slider', { name: 'Alpha', exact: true });
    await cdpDrag(page, 'touch', await across(hue, 0.02), await across(hue, 0.5));
    expect(Number(await hue.inputValue())).toBeGreaterThan(90);
    expect(await rgba(picker)).not.toEqual([255, 0, 0, 255]);
    await cdpDrag(page, 'touch', await across(alpha, 0.98), await across(alpha, -0.2));
    expect((await rgba(picker))[3]).toBe(0);
  });
});

test('arrow keys step the sliders, Shift by 10; black and grey keep the hue (AC-10, AC-11)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const picker = await openPicker(page);
  await commitHex(picker, '#3399cc');
  const hue = picker.getByRole('slider', { name: 'Hue', exact: true });
  await expect(hue).toHaveValue('200');
  await hue.focus();
  await page.keyboard.press('ArrowRight');
  await expect(hue).toHaveValue('201');
  await page.keyboard.press('Shift+ArrowLeft');
  await expect(hue).toHaveValue('191');

  const area = picker.getByRole('slider', { name: 'Saturation and value' });
  await area.focus();
  const valueOf = async () =>
    Number(/value (\d+)/.exec((await area.getAttribute('aria-valuetext')) ?? '')?.[1]);
  const satOf = async () =>
    Number(/Saturation (\d+)/.exec((await area.getAttribute('aria-valuetext')) ?? '')?.[1]);
  const v0 = await valueOf();
  const s0 = await satOf();
  await page.keyboard.press('ArrowUp');
  expect(await valueOf()).toBe(Math.min(100, v0 + 1));
  await page.keyboard.press('Shift+ArrowDown');
  expect(await valueOf()).toBe(Math.min(100, v0 + 1) - 10);
  await page.keyboard.press('ArrowLeft');
  expect(await satOf()).toBe(s0 - 1);
  await page.keyboard.press('Shift+ArrowRight');
  expect(await satOf()).toBe(s0 + 9);
  // Down to black and back: the hue stays.
  for (let i = 0; i < 12; i += 1) await page.keyboard.press('Shift+ArrowDown');
  expect(await rgba(picker)).toEqual([0, 0, 0, 255]);
  await expect(hue).toHaveValue('191');
  await page.keyboard.press('Shift+ArrowUp');
  await expect(hue).toHaveValue('191');
  // Grey (no saturation) and back: the hue stays too.
  for (let i = 0; i < 12; i += 1) await page.keyboard.press('Shift+ArrowLeft');
  await expect(hue).toHaveValue('191');

  // The alpha slider: one step, ten with Shift.
  const alpha = picker.getByRole('slider', { name: 'Alpha', exact: true });
  await alpha.focus();
  await expect(alpha).toHaveValue('255');
  await page.keyboard.press('ArrowLeft');
  await expect(alpha).toHaveValue('254');
  await page.keyboard.press('Shift+ArrowLeft');
  await expect(alpha).toHaveValue('244');
  await page.keyboard.press('Shift+ArrowRight');
  await expect(alpha).toHaveValue('254');
  expect((await rgba(picker))[3]).toBe(254);
});

test('Escape discards typed text; an outside click commits it (AC-18)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  let picker = await openPicker(page);
  await commitHex(picker, '#112233');
  await picker.getByRole('textbox', { name: 'Hex color' }).fill('#00ff00');
  await page.keyboard.press('Escape');
  await expect(picker).toHaveCount(0);
  await expect(page.getByLabel('Foreground color hex value')).toHaveValue('#112233');

  picker = await openPicker(page);
  await picker.getByRole('textbox', { name: 'Hex color' }).fill('#00ff00');
  await page.mouse.click(5, 300);
  await expect(picker).toHaveCount(0);
  await expect(page.getByLabel('Foreground color hex value')).toHaveValue('#00ff00');

  // Invalid text is dropped on the outside click.
  picker = await openPicker(page);
  await picker.getByRole('textbox', { name: 'Hex color' }).fill('#0f');
  await page.mouse.click(5, 300);
  await expect(picker).toHaveCount(0);
  await expect(page.getByLabel('Foreground color hex value')).toHaveValue('#00ff00');

  // A number field: Escape discards, an outside click commits.
  picker = await openPicker(page);
  await picker.getByRole('spinbutton', { name: 'Red', exact: true }).fill('255');
  await page.keyboard.press('Escape');
  await expect(picker).toHaveCount(0);
  await expect(page.getByLabel('Foreground color hex value')).toHaveValue('#00ff00');
  picker = await openPicker(page);
  await picker.getByRole('spinbutton', { name: 'Blue', exact: true }).fill('255');
  await page.mouse.click(5, 300);
  await expect(picker).toHaveCount(0);
  await expect(page.getByLabel('Foreground color hex value')).toHaveValue('#00ffff');
});

for (const mode of ['pixel', 'image'] as const satisfies readonly Mode[]) {
  test(`${mode}: swap and reset keep working, with alpha (AC-19)`, async ({ page }) => {
    await createDocument(page, mode, { width: 16, height: 16 });
    let picker = await openPicker(page);
    await commitHex(picker, '#11223380');
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: 'Swap foreground and background colors' }).click();
    picker = await openPicker(page, 'bg');
    expect(await rgba(picker)).toEqual([17, 34, 51, 128]);
    await page.keyboard.press('Escape');
    picker = await openPicker(page, 'fg');
    expect(await rgba(picker)).toEqual([255, 255, 255, 255]);
    await page.keyboard.press('Escape');

    await page.getByRole('button', { name: 'Reset to default colors' }).click();
    picker = await openPicker(page, 'fg');
    expect(await rgba(picker)).toEqual([0, 0, 0, 255]);
    await page.keyboard.press('Escape');
    picker = await openPicker(page, 'bg');
    expect(await rgba(picker)).toEqual([255, 255, 255, 255]);
  });
}

test.describe('phone', () => {
  test.use({ viewport: { width: 360, height: 640 } });

  for (const mode of ['pixel', 'image'] as const satisfies readonly Mode[]) {
    test(`${mode}: the popover stays inside a 360 px window (AC-20)`, async ({ page }) => {
      await createDocument(page, mode, { width: 16, height: 16 });
      await page.getByRole('button', { name: 'Panels' }).click();
      await page.getByRole('tab', { name: 'Colors' }).click();
      for (const which of ['fg', 'bg'] as const) {
        const picker = await openPicker(page, which);
        const box = await picker.boundingBox();
        if (!box) throw new Error('not visible');
        expect(box.x).toBeGreaterThanOrEqual(0);
        expect(box.y).toBeGreaterThanOrEqual(0);
        expect(box.x + box.width).toBeLessThanOrEqual(360);
        expect(box.y + box.height).toBeLessThanOrEqual(640);
        await page.keyboard.press('Escape');
      }
    });
  }
});

test('the picker is German in the German UI (AC-21)', async ({ page }) => {
  await createDocument(page, 'image', { width: 16, height: 16 });
  await page.getByRole('menubar').getByRole('menuitem', { name: 'View', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Language' }).click();
  await page.getByRole('menuitemcheckbox', { name: 'Deutsch' }).click();
  await page.getByRole('button', { name: 'Vordergrundfarbe', exact: true }).click();
  const picker = page.getByRole('dialog', { name: 'Farbwähler Vordergrundfarbe' });
  await expect(picker.getByRole('slider', { name: 'Farbton' })).toBeVisible();
  await expect(picker.getByRole('slider', { name: 'Sättigung und Hellwert' })).toBeVisible();
  await expect(picker.getByRole('slider', { name: 'Alpha', exact: true })).toBeVisible();
  await expect(picker.getByRole('textbox', { name: 'Hex-Farbwert' })).toBeVisible();
  await expect(picker.getByRole('group', { name: 'Zuletzt verwendete Farben' })).toBeAttached();
});

/** The fg picker's R / G / B / A after the Eyedropper sampled `at`. */
async function sampled(page: Page, canvas: Locator, at: [number, number]): Promise<number[]> {
  // An opaque white first, so the sample has to set every channel.
  await commitHex(await openPicker(page), '#ffffffff');
  await page.keyboard.press('Escape');
  await page.keyboard.press('i');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.click(box.x + box.width * at[0], box.y + box.height * at[1]);
  const picker = await openPicker(page);
  const out = await rgba(picker);
  await page.keyboard.press('Escape');
  return out;
}

test('pixel: the Eyedropper reads back a translucent foreground (AC-16, AC-22)', async ({
  page,
}) => {
  await createDocument(page, 'pixel', { width: 16, height: 16 });
  const picker = await openPicker(page);
  await commitHex(picker, '#11223380');
  await page.keyboard.press('Escape');
  const canvas = pixelCanvas(page);
  await page.keyboard.press('b');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.5);
  expect(await sampled(page, canvas, [0.5, 0.5])).toEqual([17, 34, 51, 128]);
});

/** One brush dab (a click): every pointer batch of a drag is its own
 *  source-over application, so overlapping dabs of a drag build up alpha
 *  (see the spec's Decisions). */
async function dab(page: Page, canvas: Locator, at: [number, number]): Promise<void> {
  const box = await canvas.boundingBox();
  if (!box) throw new Error('canvas is not visible');
  await page.mouse.click(box.x + box.width * at[0], box.y + box.height * at[1]);
}

test('image: painted alpha is color alpha × opacity, and the Eyedropper keeps it (AC-17, AC-22)', async ({
  page,
}) => {
  await createDocument(page, 'image', { width: 32, height: 32 });
  // A transparent layer on top; the Eyedropper samples that layer.
  await page.getByRole('button', { name: 'Add layer' }).click();
  await page.keyboard.press('i');
  await page
    .getByRole('toolbar', { name: 'Tool options' })
    .getByLabel('Sample')
    .selectOption('current_layer');
  const canvas = imageCanvas(page);
  const options = page.getByRole('toolbar', { name: 'Tool options' });

  await commitHex(await openPicker(page), '#ff000080');
  await page.keyboard.press('Escape');
  await page.keyboard.press('b');
  await dab(page, canvas, [0.5, 0.3]);
  expect(await sampled(page, canvas, [0.5, 0.3])).toEqual([255, 0, 0, 128]);

  // 50 % tool opacity halves it.
  await commitHex(await openPicker(page), '#0000ff80');
  await page.keyboard.press('Escape');
  await page.keyboard.press('b');
  await options.getByRole('slider', { name: /^Opacity/ }).fill('50');
  await dab(page, canvas, [0.5, 0.7]);
  const [r, g, b, a] = await sampled(page, canvas, [0.5, 0.7]);
  expect([r, g, b]).toEqual([0, 0, 255]);
  expect(Math.abs((a ?? 0) - 64)).toBeLessThanOrEqual(1);
});
