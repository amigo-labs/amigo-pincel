import { expect, test, type Locator, type Page } from '@playwright/test';
import { createDocument, gotoStart, menu } from './helpers';
import {
  PALETTES,
  keyFocus,
  lookOf,
  near,
  pickTheme,
  pixelAt,
  pseudoOf,
  rgb,
  sharedPage,
  stateOf,
  tokenColor,
  tokenValue,
  type Look,
  type State,
} from './controls-helpers';

// Controls on the shared amigo components (docs/specs/amigo-theme-controls.md):
// buttons, fields, toggles and sliders take the look of the `am-*` classes
// of the theme files, 24 px high inside the editor frame and 32 px in
// dialogs, popovers and on the start screen.

const MODES = ['pixel', 'image'] as const;

const TRANSPARENT = 'rgba(0, 0, 0, 0)';

const sidebar = (page: Page): Locator => page.getByRole('complementary', { name: 'Panels' });
const toolOptions = (page: Page): Locator => page.getByRole('toolbar', { name: 'Tool options' });
const settings = (page: Page): Locator => page.getByRole('dialog', { name: 'Settings', exact: true });

/** The four roles of the shared button, as tokens. */
const ROLES = {
  primary: { fill: 'accent-solid', text: 'on-accent', edge: null },
  default: { fill: 'surface', text: 'text', edge: 'border-strong' },
  ghost: { fill: null, text: 'text-secondary', edge: null },
  danger: { fill: 'danger-solid', text: 'on-accent', edge: null },
} as const;
type Role = keyof typeof ROLES;

/** `el` is the shared button in `role`, `height` px high. */
async function expectButton(page: Page, el: Locator, role: Role, height: 24 | 32, what: string): Promise<void> {
  const want = ROLES[role];
  // The shared button fades its fill and its outline (`.am-btn` transition):
  // one that was just enabled is still on its way.
  await expect
    .poll(async () => (await lookOf(el)).fill, `${what}: fill`)
    .toBe(want.fill ? await tokenColor(page, want.fill) : TRANSPARENT);
  await expect
    .poll(async () => (await lookOf(el)).edge, `${what}: outline`)
    .toBe(want.edge ? await tokenColor(page, want.edge) : TRANSPARENT);
  const look = await lookOf(el);
  expect(look.text, `${what}: text`).toBe(await tokenColor(page, want.text));
  expect(look.edgeWidth, `${what}: outline width`).toBe('1px');
  expect(look.radius, `${what}: radius`).toBe(await tokenValue(page, 'radius-sm'));
  expect(look.height, `${what}: height`).toBe(height);
}

/** What a state may change against the resting button (AC-01). A filter
 *  changes the fill the button paints, so it counts as the fill. */
function differs(a: State, b: State): boolean {
  return (
    a.fill !== b.fill ||
    a.edge !== b.edge ||
    a.outline !== b.outline ||
    a.opacity !== b.opacity ||
    a.filter !== b.filter
  );
}

/** Hover, pressed and keyboard focus each change `el` against its resting state. */
async function expectLiveStates(page: Page, el: Locator, what: string): Promise<void> {
  await page.mouse.move(0, 0);
  const rest = await stateOf(el);
  await el.hover();
  // The shared button fades its fill in: wait for the end of the transition.
  await expect.poll(async () => differs(await stateOf(el), rest), `${what}: hover`).toBe(true);
  await page.mouse.down();
  await expect.poll(async () => differs(await stateOf(el), rest), `${what}: pressed`).toBe(true);
  // Release outside, so the button's action does not run.
  await page.mouse.move(0, 0);
  await page.mouse.up();
  await expect.poll(async () => differs(await stateOf(el), rest), `${what}: back at rest`).toBe(false);
  await keyFocus(page, el);
  const focused = await stateOf(el);
  expect(focused.outline, `${what}: focus ring`).toBe(`2px solid ${await tokenColor(page, 'focus')}`);
  expect(differs(focused, rest), `${what}: focus`).toBe(true);
  await el.blur();
}

/** A stroke and Alt+W: the unsaved prompt with Save, Don't Save and Cancel. */
async function openUnsavedPrompt(page: Page): Promise<Locator> {
  await page.getByTitle('Pencil (B)').click();
  const box = await page.locator('canvas[data-ready="true"]').first().boundingBox();
  if (!box) throw new Error('no canvas');
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 20, box.y + box.height / 2);
  await page.mouse.up();
  await page.keyboard.press('Alt+w');
  const prompt = page.getByRole('alertdialog');
  await expect(prompt).toBeVisible();
  return prompt;
}

// Each test walks the start screen, both editors and their dialogs.
test.slow();

for (const palette of PALETTES) {
  test(`${palette}: text buttons are the shared button in its four roles, 24 px in the frame and 32 px elsewhere (AC-01)`, async ({
    page,
  }) => {
    // The start screen: 32 px.
    await gotoStart(page);
    if (palette === 'Light') await pickTheme(page, 'Light');
    await page.getByRole('button', { name: 'Open a sample' }).click();
    const sample = page.getByRole('button', { name: 'Bouncing slime (Pixel)' });
    await expectButton(page, sample, 'default', 32, 'start screen, sample');
    await expectLiveStates(page, sample, 'default');
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    const create = page.getByRole('form', { name: 'New pixel sprite' }).getByRole('button', { name: 'Create' });
    await expectButton(page, create, 'primary', 32, 'start screen, Create');
    await expectLiveStates(page, create, 'primary');

    // Busy, as `Button.svelte` renders it: disabled with `aria-busy`.
    const restCreate = await stateOf(create);
    await create.evaluate((b: HTMLButtonElement) => {
      b.setAttribute('aria-busy', 'true');
      b.disabled = true;
    });
    await expect.poll(async () => differs(await stateOf(create), restCreate), 'busy').toBe(true);
    await create.evaluate((b: HTMLButtonElement) => {
      b.removeAttribute('aria-busy');
      b.disabled = false;
    });

    for (const mode of MODES) {
      await createDocument(page, mode, { width: 16, height: 16 });
      // Inside the editor frame: 24 px. Flatten is disabled with one layer.
      const merge = sidebar(page).getByRole('button', { name: 'Flatten' });
      await expect(merge).toBeDisabled();
      const off = await lookOf(merge);
      expect(off.height, `${mode}: frame button height`).toBe(24);
      expect(off.radius).toBe(await tokenValue(page, 'radius-sm'));
      expect(off.fill, `${mode}: disabled fill`).toBe(await tokenColor(page, 'surface-raised'));
      expect(off.text, `${mode}: disabled text`).toBe(await tokenColor(page, 'text-muted'));
      expect(off.edge, `${mode}: disabled outline`).toBe(await tokenColor(page, 'border'));
      const fit = page.getByRole('button', { name: 'Fit to window' });
      await expectButton(page, fit, 'ghost', 24, `${mode}: zoom readout`);
      await expectLiveStates(page, fit, `${mode}: ghost`);
    }

    // Pixel mode's timeline: an enabled default button in the frame.
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const tag = page.getByRole('region', { name: 'Timeline' }).getByRole('button', { name: 'Add tag' });
    await expectButton(page, tag, 'default', 24, 'timeline, + Tag');

    // Image mode's dialogs (the shared Modal).
    await createDocument(page, 'image', { width: 16, height: 16 });
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Image', exact: true }).click();
    await page.getByRole('menu', { name: 'Image', exact: true }).getByRole('menuitem', { name: 'Scale Image…' }).click();
    const scale = page.getByRole('dialog', { name: 'Scale Image' });
    await expectButton(page, scale.getByRole('button', { name: 'Scale' }), 'primary', 32, 'Scale');
    await expectButton(page, scale.getByRole('button', { name: 'Cancel' }), 'default', 32, 'Scale, Cancel');

    // Pixel mode's dialogs: 32 px. The unsaved prompt comes last: it leaves unsaved work behind.
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await page.keyboard.press('Control+,');
    await expectButton(page, settings(page).getByRole('button', { name: 'Close' }), 'default', 32, 'Settings, Close');
    await page.keyboard.press('Escape');
    const prompt = await openUnsavedPrompt(page);
    await expectButton(page, prompt.getByRole('button', { name: 'Save', exact: true }), 'primary', 32, 'Save');
    const dontSave = prompt.getByRole('button', { name: "Don't Save" });
    await expectButton(page, dontSave, 'danger', 32, "Don't Save");
    await expectLiveStates(page, dontSave, 'danger');
    await expectButton(page, prompt.getByRole('button', { name: 'Cancel', exact: true }), 'default', 32, 'Cancel');

  });

  test(`${palette}: fields and selects are the shared field, 24 px in the frame and 32 px elsewhere (AC-02)`, async ({
    page,
  }) => {
    /** `el` is the shared field at rest, `height` px high. */
    const expectField = async (el: Locator, height: 24 | 32, what: string): Promise<void> => {
      // At rest: a field that has the focus shows the ring instead of its outline.
      await el.blur();
      const look = await lookOf(el);
      expect(look.fill, `${what}: fill`).toBe(await tokenColor(page, 'surface-raised'));
      expect(look.edge, `${what}: outline`).toBe(await tokenColor(page, 'border-strong'));
      expect(look.edgeWidth, `${what}: outline width`).toBe('1px');
      expect(look.radius, `${what}: radius`).toBe(await tokenValue(page, 'radius-sm'));
      expect(look.height, `${what}: height`).toBe(height);
    };
    /** With keyboard focus the ring replaces the outline. */
    const expectFocus = async (el: Locator, what: string): Promise<void> => {
      await keyFocus(page, el);
      const s = await stateOf(el);
      expect(s.outline, `${what}: focus ring`).toBe(`2px solid ${await tokenColor(page, 'focus')}`);
      expect(s.edge, `${what}: outline under the ring`).toBe(TRANSPARENT);
      await el.blur();
    };

    // The start screen: 32 px.
    await gotoStart(page);
    if (palette === 'Light') await pickTheme(page, 'Light');
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    const width = page.getByRole('form', { name: 'New pixel sprite' }).getByRole('spinbutton', { name: 'Width' });
    await expectField(width, 32, 'start screen, Width');
    await expectFocus(width, 'start screen, Width');

    for (const mode of MODES) {
      await createDocument(page, mode, { width: 16, height: 16 });
      // Inside the editor frame: 24 px.
      const hex = sidebar(page).getByRole('textbox', { name: 'Foreground color hex value' });
      await expectField(hex, 24, `${mode}: hex field`);
      await expectFocus(hex, `${mode}: hex field`);
      await expectField(toolOptions(page).getByRole('combobox').first(), 24, `${mode}: tool option select`);
      // A dialog: 32 px.
      await page.keyboard.press('Control+,');
      const language = settings(page).getByRole('combobox', { name: 'Language' });
      await expectField(language, 32, `${mode}: Language`);
      await expectFocus(language, `${mode}: Language`);
      await settings(page).getByRole('button', { name: 'Close' }).click();
      await expect(settings(page)).toBeHidden();
    }

    // A wrong value: outlined in danger-ink; focused it shows the ring and keeps its error text.
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Image', exact: true }).click();
    await page.getByRole('menu', { name: 'Image', exact: true }).getByRole('menuitem', { name: 'Scale Image…' }).click();
    const dialog = page.getByRole('dialog', { name: 'Scale Image' });
    const percent = dialog.getByLabel('Percentage', { exact: true });
    await expectField(percent, 32, 'Scale, Percentage');
    await percent.fill('0');
    await expect(percent).toHaveAttribute('aria-invalid', 'true');
    await percent.blur();
    expect((await lookOf(percent)).edge, 'invalid outline').toBe(await tokenColor(page, 'danger-ink'));
    await keyFocus(page, percent);
    expect((await stateOf(percent)).outline, 'invalid, focused').toBe(`2px solid ${await tokenColor(page, 'focus')}`);
    // The size fields say what is wrong in their hint; it stays while the field has the focus.
    await expect(percent).toHaveAttribute('aria-invalid', 'true');
    await expect(percent).toHaveAttribute('title', 'Enter a positive number');
  });

  test(`${palette}: checkboxes and radios are the shared toggle (AC-03)`, async ({ page }) => {
    const strong = (): Promise<string> => tokenColor(page, 'border-strong');
    const solid = (): Promise<string> => tokenColor(page, 'accent-solid');
    const mark = (): Promise<string> => tokenColor(page, 'on-accent');
    const expectOff = async (el: Locator, what: string): Promise<void> => {
      await expect(el, what).not.toBeChecked();
      const look = await lookOf(el);
      expect(look.edge, `${what}: outline`).toBe(await strong());
      expect(look.fill, `${what}: fill`).toBe(await tokenColor(page, 'surface-raised'));
      expect(look.height, `${what}: size`).toBe(16);
    };
    const expectOn = async (el: Locator, kind: 'checkbox' | 'radio', what: string): Promise<void> => {
      await expect(el, what).toBeChecked();
      // Reduce motion turns every change into a (very short) transition.
      await expect.poll(async () => (await lookOf(el)).fill, `${what}: fill`).toBe(await solid());
      const look = await lookOf(el);
      expect(look.edge, `${what}: outline`).toBe(TRANSPARENT);
      const after = await pseudoOf(el, '::after');
      expect(after.content, `${what}: mark`).not.toBe('none');
      expect(kind === 'checkbox' ? after.edge : after.fill, `${what}: mark colour`).toBe(await mark());
    };

    await createDocument(page, 'pixel', { width: 16, height: 16 });
    if (palette === 'Light') await pickTheme(page, 'Light');
    // Inside the editor frame.
    const pressure = toolOptions(page).getByRole('checkbox', { name: 'Size' });
    await expectOn(pressure, 'checkbox', 'tool option');
    await pressure.uncheck();
    await expectOff(pressure, 'tool option');
    await pressure.check();
    // A dialog.
    await page.keyboard.press('Control+,');
    const motion = settings(page).getByRole('checkbox', { name: 'Reduce motion' });
    await expectOff(motion, 'Reduce motion');
    await motion.check();
    await expectOn(motion, 'checkbox', 'Reduce motion');
    await motion.uncheck();
    await page.keyboard.press('Escape');

    // Image mode's tool options.
    await createDocument(page, 'image', { width: 16, height: 16 });
    // Radios: the size mode of Scale Image.
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Image', exact: true }).click();
    await page.getByRole('menu', { name: 'Image', exact: true }).getByRole('menuitem', { name: 'Scale Image…' }).click();
    const scale = page.getByRole('dialog', { name: 'Scale Image' });
    await expectOn(scale.getByRole('radio', { checked: true }), 'radio', 'size mode');
    const other = scale.getByRole('radio', { checked: false }).first();
    await expectOff(other, 'size mode');
    expect((await lookOf(other)).radius).toBe(await tokenValue(page, 'radius-full'));
    await scale.getByRole('button', { name: 'Cancel' }).click();
    await expect(scale).toBeHidden();
    const box = toolOptions(page).getByRole('checkbox').first();
    const kind = (await box.isChecked()) ? expectOn(box, 'checkbox', 'image tool option') : expectOff(box, 'image tool option');
    await kind;

    // The start screen.
    await gotoStart(page);
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    const startBox = page.getByRole('form', { name: 'New pixel sprite' }).getByRole('checkbox').first();
    await (await startBox.isChecked()
      ? expectOn(startBox, 'checkbox', 'start screen')
      : expectOff(startBox, 'start screen'));
  });

  test(`${palette}: sliders are the shared slider: the travelled part in accent on a border-strong rail (AC-04)`, async ({
    page,
  }) => {
    for (const mode of MODES) {
      await createDocument(page, mode, { width: 16, height: 16 });
      if (palette === 'Light' && mode === 'pixel') await pickTheme(page, 'Light');
      const accent = rgb(await tokenColor(page, 'accent'));
      const rail = rgb(await tokenColor(page, 'border-strong'));
      const sliders: [string, Locator][] = [
        ['alpha', sidebar(page).getByRole('slider', { name: 'Foreground alpha' })],
        ['tool option', toolOptions(page).getByRole('slider').first()],
      ];
      for (const [name, slider] of sliders) {
        const what = `${mode}: ${name}`;
        await slider.scrollIntoViewIfNeeded();
        const min = Number((await slider.getAttribute('min')) ?? 0);
        const max = Number(await slider.getAttribute('max'));
        const start = await slider.inputValue();
        const share = async (): Promise<number> =>
          slider.evaluate((el) => parseFloat(getComputedStyle(el).getPropertyValue('--am-range-fill')));
        // The share follows the value from the start, not only after an input.
        expect(await share(), `${what}: travelled share at the start`).toBeCloseTo(
          ((Number(start) - min) / (max - min)) * 100,
          3,
        );
        expect((await lookOf(slider)).height, `${what}: height`).toBe(16);
        await slider.fill(String(max));
        expect(await share(), `${what}: travelled share at the maximum`).toBe(100);

        // In the middle: accent left of the thumb, the rail right of it, the thumb in accent.
        const mid = min + Math.round((max - min) / 2);
        await slider.fill(String(mid));
        expect(await share(), `${what}: travelled share`).toBeCloseTo(((mid - min) / (max - min)) * 100, 3);
        const box = await slider.boundingBox();
        if (!box) throw new Error('no slider');
        const y = box.y + box.height / 2;
        await page.mouse.move(0, 0);
        expect(near(await pixelAt(page, box.x + 4, y), accent), `${what}: travelled part`).toBe(true);
        expect(near(await pixelAt(page, box.x + box.width - 4, y), rail), `${what}: rail`).toBe(true);
        // The thumb (14 px wide) travels between the rail's ends and is taller than the rail.
        const thumb = box.x + 7 + ((mid - min) / (max - min)) * (box.width - 14);
        expect(near(await pixelAt(page, thumb, y), accent), `${what}: thumb`).toBe(true);
        expect(near(await pixelAt(page, thumb, y - 3), accent), `${what}: thumb height`).toBe(true);
        await slider.fill(String(min));
        expect(await share(), `${what}: travelled share at the minimum`).toBe(0);
        expect(near(await pixelAt(page, box.x + box.width - 4, y), rail), `${what}: rail at the minimum`).toBe(true);
        expect(near(await pixelAt(page, box.x + 6, y), accent), `${what}: thumb at the minimum`).toBe(true);
        await slider.fill(start);
      }
    }
  });
}

/** The phone layout's panel sheet, open. */
async function openPanelSheet(page: Page): Promise<Locator> {
  await page.setViewportSize({ width: 390, height: 800 });
  await page.getByRole('button', { name: 'Panels', exact: true }).click();
  const sheet = page.getByRole('dialog', { name: 'Panels' });
  await expect(sheet).toBeVisible();
  return sheet;
}

for (const palette of PALETTES) {
  test(`${palette}: the phone's panel sheet has the shared tab row (AC-05)`, async ({ page }) => {
    for (const mode of MODES) {
      await page.setViewportSize({ width: 1280, height: 720 });
      await createDocument(page, mode, { width: 16, height: 16 });
      if (palette === 'Light' && mode === 'pixel') await pickTheme(page, 'Light');
      const sheet = await openPanelSheet(page);
      const tabs = sheet.getByRole('tablist').getByRole('tab');
      expect(await tabs.count()).toBeGreaterThan(1);
      const on = sheet.getByRole('tab', { selected: true });
      await expect(on).toHaveCount(1);
      const selected = await stateOf(on);
      expect(selected.text, `${mode}: selected tab`).toBe(await tokenColor(page, 'accent-ink'));
      expect(selected.underline, `${mode}: underline`).toBe(`2px ${await tokenColor(page, 'accent')}`);
      expect(selected.height).toBe(36);
      const name = await sheet.getByRole('tab', { selected: false }).first().innerText();
      const off = sheet.getByRole('tab', { name: name.trim(), exact: true });
      const rest = await stateOf(off);
      expect(rest.text, `${mode}: other tab`).toBe(await tokenColor(page, 'text-secondary'));
      expect(rest.underline, `${mode}: no underline`).toBe(`2px ${TRANSPARENT}`);
      // The row's hairline, and a pick moves the underline.
      expect((await stateOf(sheet.getByRole('tablist'))).underline).toBe(`1px ${await tokenColor(page, 'border')}`);
      await off.click();
      await expect(off).toHaveAttribute('aria-selected', 'true');
      expect((await stateOf(off)).text).toBe(await tokenColor(page, 'accent-ink'));
      await expect(sheet.getByRole('button', { name: 'Close panels' })).toBeVisible();
    }
  });

  test(`${palette}: the preview's zoom choice is the shared segmented control (AC-05)`, async ({ page }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    if (palette === 'Light') await pickTheme(page, 'Light');
    const panel = sidebar(page).locator('section[aria-label="Preview"]');
    await panel.locator('header button[aria-expanded="false"]').click();
    const group = panel.getByRole('group', { name: 'Preview zoom' });
    const box = await lookOf(group);
    expect(box.fill).toBe(await tokenColor(page, 'surface-raised'));
    expect(box.edge).toBe(await tokenColor(page, 'border'));
    expect(box.radius).toBe(await tokenValue(page, 'radius-sm'));
    expect(box.height, 'dense: 24 px').toBe(24);
    const on = group.locator('button[aria-pressed="true"]');
    await expect(on).toHaveCount(1);
    const selected = await stateOf(on);
    expect(selected.fill, 'selected segment').toBe(await tokenColor(page, 'surface'));
    expect(selected.text).toBe(await tokenColor(page, 'text'));
    expect(selected.shadow, 'raised').not.toBe('none');
    const off = group.locator('button[aria-pressed="false"]').first();
    const rest = await stateOf(off);
    expect(rest.fill).toBe(TRANSPARENT);
    expect(rest.text).toBe(await tokenColor(page, 'text-secondary'));
    expect(rest.shadow).toBe('none');
  });
}

const MENU_ALONE = `
  <div id="menu" class="am-menu">
    <button id="item"><span class="am-menu-label">Copy</span><kbd id="kbd" class="am-kbd">Ctrl+C</kbd></button>
    <button id="item-off" disabled><span class="am-menu-label">Paste</span></button>
    <button id="item-danger" class="am-danger"><span class="am-menu-label">Delete</span></button>
  </div>
  <div id="seg" class="am-seg"><label id="seg-on"><input type="radio" name="s" checked />Dark</label><label id="seg-off"><input type="radio" name="s" />Light</label></div>
  <div id="seg-sm" class="am-seg am-seg--sm"><button id="seg-sm-on" role="radio" aria-checked="true">A</button><button id="seg-sm-off" role="radio" aria-checked="false">B</button></div>
`;

for (const palette of PALETTES) {
  test(`${palette}: the tool variants and the theme choice are the shared segmented control (AC-05)`, async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    if (palette === 'Light') await pickTheme(page, 'Light');
    const alone = await sharedPage(page, palette, MENU_ALONE);
    const same = async (el: Locator, id: string): Promise<void> => {
      const want = await stateOf(alone.locator(`#${id}`));
      const pick = (s: State): unknown => ({ fill: s.fill, text: s.text, radius: s.radius, height: s.height, shadow: s.shadow });
      await expect.poll(async () => pick(await stateOf(el)), `against the shared #${id}`).toEqual(pick(want));
    };

    // The variants of the active tool group, in the options bar (24 px).
    await page.getByRole('toolbar', { name: 'Tools' }).getByRole('button', { name: /^Marquee/ }).click();
    const variants = toolOptions(page).getByRole('radiogroup');
    await expect(variants).toBeVisible();
    const box = await lookOf(variants);
    expect(box.height, 'dense').toBe(24);
    expect(box.fill).toBe(await tokenColor(page, 'surface-raised'));
    expect(box.edge).toBe(await tokenColor(page, 'border'));
    const on = variants.getByRole('radio', { checked: true });
    await expect(on).toHaveCount(1);
    await same(on, 'seg-sm-on');
    expect((await stateOf(on)).fill, 'selected segment raised on surface').toBe(await tokenColor(page, 'surface'));
    const label = await variants.getByRole('radio', { checked: false }).first().getAttribute('aria-label');
    const off = variants.getByRole('radio', { name: label ?? '', exact: true });
    await same(off, 'seg-sm-off');
    // The keyboard still moves the choice; a click picks.
    await off.click();
    await expect(off).toBeChecked();
    await same(off, 'seg-sm-on');

    // The theme choice in Settings (32 px).
    await page.keyboard.press('Control+,');
    const theme = settings(page).getByRole('radiogroup', { name: 'Theme' });
    const segments = theme.locator('.am-seg');
    expect((await lookOf(segments)).height).toBe(32);
    expect((await lookOf(segments)).fill).toBe(await tokenColor(page, 'surface-raised'));
    await same(theme.locator('label').filter({ hasText: palette }), 'seg-on');
    await same(theme.locator('label').filter({ hasText: 'System' }), 'seg-off');
    await expect(theme.getByRole('radio', { name: palette, exact: true })).toBeChecked();
    await alone.close();
  });

  test(`${palette}: menus from the menu bar, context menus and tool flyouts are the shared menu (AC-06)`, async ({
    page,
  }) => {
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    if (palette === 'Light') await pickTheme(page, 'Light');
    const alone = await sharedPage(page, palette, MENU_ALONE);
    const menuLook = async (el: Locator): Promise<unknown> => {
      const s = await stateOf(el);
      return { fill: s.fill, edge: s.edge, radius: s.radius, shadow: s.shadow };
    };
    const itemLook = async (el: Locator): Promise<unknown> => {
      const s = await stateOf(el);
      return { fill: s.fill, text: s.text, radius: s.radius, height: s.height };
    };
    const wantMenu = await menuLook(alone.locator('#menu'));
    const wantItem = await itemLook(alone.locator('#item'));
    expect((wantMenu as { radius: string }).radius).toBe(await tokenValue(page, 'radius-md'));

    // A menu of the menu bar.
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Edit', exact: true }).click();
    const edit = page.getByRole('menu', { name: 'Edit', exact: true });
    await page.mouse.move(0, 0);
    expect(await menuLook(edit), 'Edit menu').toEqual(wantMenu);
    expect((await stateOf(edit)).fill).toBe(await tokenColor(page, 'surface'));
    const copy = edit.getByRole('menuitem', { name: 'Copy', exact: true });
    expect(await itemLook(copy), 'item').toEqual(wantItem);
    // The shortcut is a key cap at the right end.
    const cap = copy.locator('kbd');
    await expect(cap).toHaveText(/C$/);
    const capLook = await lookOf(cap);
    expect({ ...capLook, height: 0 }).toEqual({ ...(await lookOf(alone.locator('#kbd'))), height: 0 });
    const capBox = await cap.boundingBox();
    const copyBox = await copy.boundingBox();
    if (!capBox || !copyBox) throw new Error('no item');
    expect(copyBox.x + copyBox.width - (capBox.x + capBox.width), 'key cap at the right').toBeLessThanOrEqual(8);
    // A disabled item: muted, and no fill under the pointer.
    const disabled = edit.locator('button:disabled').first();
    await expect(disabled).toBeVisible();
    expect((await stateOf(disabled)).text, 'disabled item').toBe(await tokenColor(page, 'text-muted'));
    await disabled.hover({ force: true });
    expect((await stateOf(disabled)).fill, 'disabled item under the pointer').toBe(TRANSPARENT);
    await copy.hover();
    await expect.poll(async () => (await stateOf(copy)).fill, 'hovered item').toBe(await tokenColor(page, 'hover'));
    await page.keyboard.press('Escape');

    // A destructive item.
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Layer', exact: true }).click();
    const del = page.getByRole('menu', { name: 'Layer', exact: true }).getByRole('menuitem', { name: 'Delete Layer' });
    await page.mouse.move(0, 0);
    const danger = await stateOf(del);
    expect(await del.isDisabled() ? await tokenColor(page, 'text-muted') : await tokenColor(page, 'danger-ink')).toBe(
      danger.text,
    );
    await expect(del).toHaveClass(/am-danger/);
    await page.keyboard.press('Escape');

    // A context menu (the palette's) and a tool flyout.
    await sidebar(page).getByRole('button', { name: 'Palette options' }).click();
    const more = page.getByRole('menu', { name: 'Palette options' });
    await page.mouse.move(0, 0);
    expect(await menuLook(more), 'palette menu').toEqual(wantMenu);
    expect(await itemLook(more.getByRole('menuitem').first()), 'palette menu item').toEqual(wantItem);
    // The menu takes Escape while the focus is in it.
    await more.getByRole('menuitem').first().focus();
    await page.keyboard.press('Escape');
    await expect(more).toBeHidden();
    await page.getByRole('toolbar', { name: 'Tools' }).getByRole('button', { name: /^Marquee/ }).focus();
    await page.keyboard.press('Shift+F10');
    const flyout = page.getByRole('menu', { name: 'Marquee' });
    await expect(flyout).toBeVisible();
    await page.mouse.move(0, 0);
    expect(await menuLook(flyout), 'tool flyout').toEqual(wantMenu);
    // The entry the keyboard is on is filled like a hovered one; read another.
    const entry = flyout.locator('button[role="menuitemradio"]:not(:focus)').first();
    await expect(flyout.locator('button:focus')).toHaveAttribute('data-active', '');
    expect(await itemLook(entry), 'flyout entry').toEqual(wantItem);
    await alone.close();
  });
}

// AC-07: a dialog as the shared dialog rendered alone: the surface, the
// title, and where the buttons sit (bottom right, the action last).
const DIALOG_ALONE = `
  <div id="dialog" class="am-dialog">
    <header><h2 id="dialog-title">Title</h2></header>
    <div class="am-body">Body</div>
    <footer><button class="am-btn">Cancel</button><button id="dialog-action" class="am-btn am-btn--primary">OK</button></footer>
  </div>
`;

/** What a dialog's surface and title look like, and where its action sits. */
interface DialogLook {
  surface: { fill: string; edge: string; radius: string; shadow: string; text: string };
  title: { font: string; color: string };
  /** The action's distance from the surface's right and bottom edge. */
  action: { right: number; bottom: number };
}

/**
 * Reads `dialog` (a role=dialog element, the surface itself or the overlay
 * around it): the surface is the first opaque box around its title.
 */
async function dialogLook(title: Locator, action: Locator): Promise<DialogLook> {
  const a = await action.boundingBox();
  if (!a) throw new Error('no action button');
  return title.evaluate(
    (node, box) => {
      let surface = node.parentElement;
      while (surface && getComputedStyle(surface).backgroundColor === 'rgba(0, 0, 0, 0)') {
        surface = surface.parentElement;
      }
      if (!surface) throw new Error('no surface');
      const cs = getComputedStyle(surface);
      const ts = getComputedStyle(node);
      const r = surface.getBoundingClientRect();
      return {
        surface: {
          fill: cs.backgroundColor,
          edge: `${cs.borderTopWidth} ${cs.borderTopStyle} ${cs.borderTopColor}`,
          radius: cs.borderTopLeftRadius,
          shadow: cs.boxShadow,
          text: cs.color,
        },
        title: {
          font: `${ts.fontWeight} ${ts.fontSize}/${ts.lineHeight} ${ts.letterSpacing}`,
          color: ts.color,
        },
        action: {
          right: Math.round(r.right - (box.x + box.width)),
          bottom: Math.round(r.bottom - (box.y + box.height)),
        },
      };
    },
    a,
  );
}

/** `safe` sits left of `action` in its row, and nothing in that row is right of `action`. */
async function expectActionLast(dialog: Locator, action: Locator, safe: Locator | null, what: string): Promise<void> {
  const a = await action.boundingBox();
  if (!a) throw new Error(`${what}: no action`);
  if (safe) {
    const s = await safe.boundingBox();
    if (!s) throw new Error(`${what}: no safe choice`);
    expect(s.x + s.width, `${what}: the safe choice before the action`).toBeLessThanOrEqual(a.x);
  }
  const rights = await dialog.getByRole('button').evaluateAll(
    (els, row) =>
      els
        .map((e) => e.getBoundingClientRect())
        .filter((r) => r.width > 0 && Math.abs(r.top + r.height / 2 - row) < 4)
        .map((r) => r.right),
    a.y + a.height / 2,
  );
  expect(Math.max(...rights), `${what}: the action is last in its row`).toBeLessThanOrEqual(a.x + a.width + 0.5);
}

for (const palette of PALETTES) {
  test(`${palette}: dialogs are the shared dialog, buttons bottom right with the action last (AC-07)`, async ({
    page,
  }) => {
    await gotoStart(page);
    if (palette === 'Light') await pickTheme(page, 'Light');
    const alone = await sharedPage(page, palette, DIALOG_ALONE);
    const want = await dialogLook(alone.locator('#dialog-title'), alone.locator('#dialog-action'));
    expect(want.surface.radius).toBe(await tokenValue(page, 'radius-lg'));
    expect(want.surface.fill).toBe(await tokenColor(page, 'surface'));
    /** `dialog` has the shared look; `action` is last, `safe` before it. */
    const check = async (dialog: Locator, actionName: string, safeName: string | null, what: string): Promise<void> => {
      await expect(dialog).toBeVisible();
      const action = dialog.getByRole('button', { name: actionName, exact: true });
      const safe = safeName ? dialog.getByRole('button', { name: safeName, exact: true }) : null;
      const look = await dialogLook(dialog.getByRole('heading').first(), action);
      expect(look, what).toEqual(want);
      await expectActionLast(dialog, action, safe, what);
    };

    // Settings, from the start screen.
    await page.keyboard.press('Control+,');
    await check(settings(page), 'Close', null, 'Settings');
    await settings(page).getByRole('button', { name: 'Close' }).click();

    // Pixel mode: New, Export PNG, Keyboard Shortcuts, Rename layer.
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await menu(page, 'File', 'New…');
    const created = page.getByRole('dialog', { name: 'New document' });
    await check(created, 'Create', 'Cancel', 'New');
    await created.getByRole('button', { name: 'Cancel' }).click();
    await menu(page, 'File', 'Export', 'PNG (current frame)');
    const png = page.getByRole('dialog', { name: 'Export PNG' });
    await check(png, 'Export', 'Cancel', 'Export PNG');
    await png.getByRole('button', { name: 'Cancel' }).click();
    await menu(page, 'Help', 'Keyboard Shortcuts');
    const keys = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
    await check(keys, 'Close', null, 'Keyboard Shortcuts');
    await keys.getByRole('button', { name: 'Close' }).click();
    await menu(page, 'Layer', 'Rename…');
    const rename = page.getByRole('dialog', { name: 'Rename layer' });
    await check(rename, 'Rename', 'Cancel', 'Rename layer');
    await rename.getByRole('button', { name: 'Cancel' }).click();

    // Image mode's dialog shell (Scale Image) and an effect's floating dialog.
    await createDocument(page, 'image', { width: 16, height: 16 });
    await menu(page, 'Image', 'Scale Image…');
    const scale = page.getByRole('dialog', { name: 'Scale Image' });
    await check(scale, 'Scale', 'Cancel', 'Scale Image');
    await scale.getByRole('button', { name: 'Cancel' }).click();
    await menu(page, 'Effects', 'Blurs', 'Gaussian Blur…');
    const effect = page.getByRole('dialog', { name: /Gaussian Blur/ });
    await check(effect, 'OK', 'Cancel', 'Gaussian Blur');
    await effect.getByRole('button', { name: 'Cancel' }).click();
    await expect(effect).toBeHidden();

    // The unsaved prompt: Cancel first, Save last, Don't Save between.
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const prompt = await openUnsavedPrompt(page);
    await check(prompt, 'Save', 'Cancel', 'unsaved prompt');
    await prompt.getByRole('button', { name: 'Cancel' }).click();
    await alone.close();
  });
}

// AC-08: notices on the shared banner and chip.
const NOTICE_ALONE = `
  <div id="banner" class="am-banner"><div>Text</div></div>
  <div id="banner-warning" class="am-banner am-banner--warning"><div><strong>Text</strong></div></div>
  <span id="chip-warning" class="am-chip am-chip--warning">Text</span>
`;

for (const palette of PALETTES) {
  test(`${palette}: notices are the shared banner and chip, in the colour of their state (AC-08)`, async ({
    page,
  }) => {
    await gotoStart(page);
    const alone = await sharedPage(page, palette, NOTICE_ALONE);
    /** A banner's look without its height (that follows its text). */
    const bannerLook = async (el: Locator): Promise<Omit<Look, 'height'>> => {
      const { fill, text, edge, edgeWidth, radius } = await lookOf(el);
      return { fill, text, edge, edgeWidth, radius };
    };

    // The tour's offer on the start screen: a neutral banner.
    const tour = page.getByRole('region', { name: 'Tour', exact: true });
    if (palette === 'Light') await pickTheme(page, 'Light');
    await expect(tour).toBeVisible();
    expect(await bannerLook(tour), 'tour banner').toEqual(await bannerLook(alone.locator('#banner')));
    expect((await lookOf(tour)).fill).toBe(await tokenColor(page, 'surface-raised'));

    // A taken chord in Keyboard Shortcuts: a warning banner, Cancel before Replace.
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await menu(page, 'Help', 'Keyboard Shortcuts');
    const keys = page.getByRole('dialog', { name: 'Keyboard Shortcuts' });
    await keys
      .getByRole('row')
      .filter({ has: page.getByRole('rowheader', { name: 'Undo', exact: true }) })
      .getByRole('button', { name: 'Change', exact: true })
      .click();
    await page.keyboard.press('Control+Shift+z');
    const warning = keys.getByRole('alert');
    await expect(warning).toContainText('Redo');
    expect(await bannerLook(warning), 'conflict banner').toEqual(await bannerLook(alone.locator('#banner-warning')));
    expect((await lookOf(warning)).fill).toBe(await tokenColor(page, 'warning-subtle'));
    await expectActionLast(
      warning,
      warning.getByRole('button', { name: 'Replace' }),
      warning.getByRole('button', { name: 'Cancel' }),
      'conflict',
    );
    await warning.getByRole('button', { name: 'Cancel' }).click();
    await keys.getByRole('button', { name: 'Close' }).click();

    // A refused action in Image mode: the status bar's notice is a warning chip with its text.
    await createDocument(page, 'image', { width: 16, height: 16 });
    await page.getByRole('button', { name: 'Lock Layer 1', exact: true }).click();
    await menu(page, 'Edit', 'Fill Selection');
    const notice = page.getByText('Layer “Layer 1” is locked', { exact: true });
    await expect(notice).toBeVisible();
    expect(await lookOf(notice), 'status notice').toEqual(await lookOf(alone.locator('#chip-warning')));
    expect((await lookOf(notice)).text).toBe(await tokenColor(page, 'warning-ink'));
    await alone.close();
  });
}

// AC-10: the app's control against the shared component rendered alone
// from the theme files (`ui/src/theme/`), property by property.
const ALONE = `
  <button id="btn" class="am-btn">Label</button>
  <button id="btn-sm" class="am-btn am-btn--sm">Label</button>
  <button id="btn-off" class="am-btn am-btn--sm" disabled>Label</button>
  <button id="primary" class="am-btn am-btn--primary">Label</button>
  <button id="ghost-sm" class="am-btn am-btn--ghost am-btn--sm">Label</button>
  <button id="danger" class="am-btn am-btn--danger">Label</button>
  <input id="field" class="am-input" value="64" />
  <input id="field-sm" class="am-input am-input--sm" value="#000000" />
  <input id="field-bad" class="am-input" aria-invalid="true" value="0" />
  <select id="select-sm" class="am-input am-input--sm"><option>One</option></select>
  <label class="am-check"><input id="check" type="checkbox" /> Off</label>
  <label class="am-check"><input id="check-on" type="checkbox" checked /> On</label>
  <label class="am-check"><input id="radio" type="radio" name="r" /> Off</label>
  <label class="am-check"><input id="radio-on" type="radio" name="r" checked /> On</label>
  <input id="range" type="range" class="am-range" min="0" max="100" value="50" style="--am-range-fill: 50%; width: 80px" />
  <div id="seg" class="am-seg am-seg--sm"><button id="seg-on" aria-pressed="true">1×</button><button id="seg-off" aria-pressed="false">2×</button></div>
  <div id="tabs" class="am-tabs"><button id="tab-on" aria-selected="true">Colors</button><button id="tab-off" aria-selected="false">Layers</button></div>
`;

for (const palette of PALETTES) {
  test(`${palette}: each control family looks like the shared component rendered alone (AC-10)`, async ({
    page,
  }) => {
    await gotoStart(page);
    if (palette === 'Light') await pickTheme(page, 'Light');
    const alone = await sharedPage(page, palette, ALONE);
    /** The app's control has the look of the shared component `#id`. */
    const same = async (el: Locator, id: string): Promise<void> => {
      await el.evaluate((node) => (node as HTMLElement).blur());
      // A toggle and a slider have no text of their own: their colour is their label's.
      const textless = await el.evaluate(
        (node) => node instanceof HTMLInputElement && ['checkbox', 'radio', 'range'].includes(node.type),
      );
      const look = async (of: Locator): Promise<Partial<Look>> => {
        const { text, ...rest } = await lookOf(of);
        return textless ? rest : { text, ...rest };
      };
      const want = await look(alone.locator(`#${id}`));
      // A control that was just enabled or released still fades its fill.
      await expect.poll(() => look(el), `the app's control against the shared #${id}`).toEqual(want);
    };
    /** The same for what a state adds: a toggle's mark, a tab's underline, a segment's shadow. */
    const sameState = async (el: Locator, id: string): Promise<void> => {
      const mine = await stateOf(el);
      const theirs = await stateOf(alone.locator(`#${id}`));
      expect({ underline: mine.underline, shadow: mine.shadow }, `#${id}`).toEqual({
        underline: theirs.underline,
        shadow: theirs.shadow,
      });
      expect(await pseudoOf(el, '::after'), `#${id}: mark`).toEqual(await pseudoOf(alone.locator(`#${id}`), '::after'));
    };

    // Buttons and fields at 32 px: the start screen and a dialog.
    await page.getByRole('button', { name: 'Open a sample' }).click();
    await same(page.getByRole('button', { name: 'Bouncing slime (Pixel)' }), 'btn');
    await page.getByRole('button', { name: 'New pixel sprite' }).click();
    const form = page.getByRole('form', { name: 'New pixel sprite' });
    await same(form.getByRole('button', { name: 'Create' }), 'primary');
    await same(form.getByRole('spinbutton', { name: 'Width' }), 'field');
    await same(form.getByRole('checkbox').first(), 'check');

    // The editor frame: 24 px.
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    await same(page.getByRole('region', { name: 'Timeline' }).getByRole('button', { name: 'Add tag' }), 'btn-sm');
    await same(sidebar(page).getByRole('button', { name: 'Flatten' }), 'btn-off');
    await same(page.getByRole('button', { name: 'Fit to window' }), 'ghost-sm');
    await same(sidebar(page).getByRole('textbox', { name: 'Foreground color hex value' }), 'field-sm');
    await same(toolOptions(page).getByRole('combobox').first(), 'select-sm');
    const size = toolOptions(page).getByRole('checkbox', { name: 'Size' });
    await same(size, 'check-on');
    await sameState(size, 'check-on');
    await same(toolOptions(page).getByRole('checkbox', { name: 'Pixel-perfect' }), 'check');

    // The slider: the element, and the same pixels along the rail and on the thumb.
    const slider = sidebar(page).getByRole('slider', { name: 'Foreground alpha' });
    await slider.fill('128');
    await same(slider, 'range');
    await page.mouse.move(0, 0);
    const mine = await slider.boundingBox();
    const theirs = await alone.locator('#range').boundingBox();
    if (!mine || !theirs) throw new Error('no slider');
    expect(mine.width, 'both sliders 80 px wide').toBe(theirs.width);
    for (const [dx, dy] of [[4, 8], [40, 8], [40, 5], [76, 8]] as const) {
      const a = await pixelAt(page, mine.x + dx, mine.y + dy);
      const b = await pixelAt(alone, theirs.x + dx, theirs.y + dy);
      expect(near(a, b), `slider pixel ${dx},${dy}: ${a.join(' ')} against ${b.join(' ')}`).toBe(true);
    }

    // The segmented control (the preview's zoom).
    const preview = sidebar(page).locator('section[aria-label="Preview"]');
    await preview.locator('header button[aria-expanded="false"]').click();
    const zoom = preview.getByRole('group', { name: 'Preview zoom' });
    await same(zoom, 'seg');
    await same(zoom.locator('button[aria-pressed="true"]'), 'seg-on');
    await sameState(zoom.locator('button[aria-pressed="true"]'), 'seg-on');
    await same(zoom.locator('button[aria-pressed="false"]').first(), 'seg-off');

    // A dialog's button: Settings.
    await page.keyboard.press('Control+,');
    await same(settings(page).getByRole('button', { name: 'Close' }), 'btn');
    await settings(page).getByRole('button', { name: 'Close' }).click();

    // A wrong value in a dialog's field (Image mode).
    await createDocument(page, 'image', { width: 16, height: 16 });
    await page.getByRole('menubar').getByRole('menuitem', { name: 'Image', exact: true }).click();
    await page.getByRole('menu', { name: 'Image', exact: true }).getByRole('menuitem', { name: 'Scale Image…' }).click();
    const scale = page.getByRole('dialog', { name: 'Scale Image' });
    const percent = scale.getByLabel('Percentage', { exact: true });
    await percent.fill('0');
    await same(percent, 'field-bad');
    await same(scale.getByRole('radio', { checked: true }), 'radio-on');
    await sameState(scale.getByRole('radio', { checked: true }), 'radio-on');
    await same(scale.getByRole('radio', { checked: false }).first(), 'radio');
    await scale.getByRole('button', { name: 'Cancel' }).click();
    await expect(scale).toBeHidden();

    // The tab row of the phone's panel sheet.
    const sheet = await openPanelSheet(page);
    await same(sheet.getByRole('tab', { selected: true }), 'tab-on');
    await sameState(sheet.getByRole('tab', { selected: true }), 'tab-on');
    await same(sheet.getByRole('tab', { selected: false }).first(), 'tab-off');
    await page.setViewportSize({ width: 1280, height: 720 });

    // The danger button: the unsaved prompt (last, it leaves unsaved work behind).
    await createDocument(page, 'pixel', { width: 16, height: 16 });
    const prompt = await openUnsavedPrompt(page);
    await same(prompt.getByRole('button', { name: "Don't Save" }), 'danger');
    await alone.close();
  });
}


// AC-07 (panels), AC-08 (floating notices), AC-10: docked panels, the
// banners that float over the app, and the New dialog's size warnings.
const DOCKED_ALONE = `
  <section id="docked" class="am-panel am-panel--docked">
    <header><button type="button">Layers</button></header>
    <div class="am-body">Body</div>
  </section>
  <div id="float" class="am-banner am-banner--danger am-banner--float"><div><strong>Text</strong></div></div>
  <p id="hint-warning" class="am-hint am-hint--warning">Text</p>
`;

/** What the shared docked panel fixes: the panel's edges and its header. */
async function dockedLook(panel: Locator): Promise<Record<string, string>> {
  return panel.evaluate((el) => {
    const p = getComputedStyle(el);
    const header = el.querySelector(':scope > header');
    const h = header ? getComputedStyle(header) : null;
    return {
      fill: p.backgroundColor,
      radius: p.borderTopLeftRadius,
      top: p.borderTopWidth,
      side: p.borderLeftWidth,
      hairline: `${p.borderBottomWidth} ${p.borderBottomStyle} ${p.borderBottomColor}`,
      headerHeight: h?.height ?? 'none',
      headerPadding: h ? `${h.paddingLeft} ${h.paddingRight}` : 'none',
      headerFont: h ? `${h.fontSize} ${h.fontWeight} ${h.textTransform} ${h.letterSpacing}` : 'none',
      headerText: h?.color ?? 'none',
    };
  });
}

for (const palette of PALETTES) {
  test(`${palette}: docked panels, floating notices and size warnings are the shared ones (AC-07, AC-08, AC-10)`, async ({
    page,
  }) => {
    await gotoStart(page);
    if (palette === 'Light') await pickTheme(page, 'Light');
    const alone = await sharedPage(page, palette, DOCKED_ALONE);
    const docked = await dockedLook(alone.locator('#docked'));
    expect(docked.hairline).toBe(`1px solid ${await tokenColor(page, 'border')}`);

    // Panels in the sidebar of both editors and in Pixel mode's bottom dock.
    for (const mode of MODES) {
      await createDocument(page, mode, { width: 16, height: 16 });
      const layers = sidebar(page).getByRole('region', { name: 'Layers', exact: true });
      expect(await dockedLook(layers), `${mode}: Layers panel`).toEqual(docked);
      if (mode === 'pixel') {
        const timeline = page.getByRole('region', { name: 'Timeline' });
        expect(await dockedLook(timeline), 'Timeline panel').toEqual(docked);
      }
    }

    // The New dialog: a width over the limit is noted as a warning hint.
    await menu(page, 'File', 'New…');
    const created = page.getByRole('dialog', { name: 'New document' });
    const width = created.getByRole('spinbutton', { name: 'Width' });
    await width.fill('999999');
    // The note the field names as its description.
    await expect(width).toHaveAttribute('aria-describedby', /.+/);
    const hint = created.locator(`[id="${await width.getAttribute('aria-describedby')}"]`);
    await expect(hint).toContainText('At most');
    // A hint's height follows its text (the note wraps), so it is left out.
    const textLook = async (el: Locator): Promise<Omit<Look, 'height'>> => {
      const { fill, text, edge, edgeWidth, radius } = await lookOf(el);
      return { fill, text, edge, edgeWidth, radius };
    };
    expect(await textLook(hint), 'size warning').toEqual(await textLook(alone.locator('#hint-warning')));
    const font = (el: Locator): Promise<string> =>
      el.evaluate((n) => `${getComputedStyle(n).fontSize}/${getComputedStyle(n).lineHeight}`);
    expect(await font(hint), 'size warning font').toBe(await font(alone.locator('#hint-warning')));
    expect((await lookOf(hint)).text).toBe(await tokenColor(page, 'warning-ink'));
    await created.getByRole('button', { name: 'Cancel' }).click();

    // A file that cannot be opened: the floating error notice.
    await gotoStart(page);
    await page.getByRole('main').evaluate((el) => {
      const data = new DataTransfer();
      data.items.add(new File(['notes'], 'notes.txt', { type: 'text/plain' }));
      el.dispatchEvent(new DragEvent('drop', { dataTransfer: data, bubbles: true, cancelable: true }));
    });
    const toast = page.getByRole('alert').filter({ hasText: 'notes.txt' });
    await expect(toast).toBeVisible();
    const shadow = (el: Locator): Promise<string> => el.evaluate((n) => getComputedStyle(n).boxShadow);
    expect(await shadow(toast), 'floating notice').toBe(await shadow(alone.locator('#float')));
    expect(await shadow(toast)).not.toBe('none');
    await toast.getByRole('button', { name: 'Dismiss' }).click();
    await alone.close();
  });
}
