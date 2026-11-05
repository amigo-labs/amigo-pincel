import { expect, test } from '@playwright/test';
import {
  flyoutFocus,
  flyoutStep,
  hasFlyout,
  placeFlyout,
  pressTarget,
  type ToolGroup,
} from '../src/lib/components/layout/tools';

// Pure tool-rail model behind the variant flyout (tool-variant-flyout):
// no browser needed. No shipping group mixes enabled and disabled
// variants, so the disabled rules are proven here on a made-up group.

type Id = 'a' | 'b' | 'c' | 'd';

const group = (disabled: Id[] = [], ids: Id[] = ['a', 'b', 'c', 'd']): ToolGroup<Id> => ({
  id: 'g',
  label: 'Group',
  shortcut: 'G',
  tools: ids.map((id) => ({ id, label: id, icon: 'rect', disabled: disabled.includes(id) })),
});

test('hasFlyout: a single button and an all-disabled group open no flyout (AC-02)', () => {
  expect(hasFlyout(group())).toBe(true);
  expect(hasFlyout(group(['a', 'b', 'c']))).toBe(true);
  expect(hasFlyout(group([], ['a']))).toBe(false);
  expect(hasFlyout(group(['a', 'b', 'c', 'd']))).toBe(false);
});

test('pressTarget: a short press selects the shown variant, or the first enabled one (AC-04)', () => {
  const g = group(['a', 'c']);
  const shown = (id: Id) => ({ id, label: id, icon: 'rect' as const, disabled: id === 'a' || id === 'c' });
  expect(pressTarget(g, shown('d'))).toBe('d');
  expect(pressTarget(g, shown('c'))).toBe('b');
  expect(pressTarget(g, shown('a'))).toBe('b');
});

test('flyoutStep: arrows skip disabled entries and wrap, Home / End go to the ends (AC-05, AC-08)', () => {
  const tools = group(['a', 'c']).tools;
  expect(flyoutStep(tools, 'b', 'next')).toBe('d');
  expect(flyoutStep(tools, 'd', 'next')).toBe('b');
  expect(flyoutStep(tools, 'b', 'prev')).toBe('d');
  expect(flyoutStep(tools, 'd', 'prev')).toBe('b');
  expect(flyoutStep(tools, 'd', 'first')).toBe('b');
  expect(flyoutStep(tools, 'b', 'last')).toBe('d');
  expect(flyoutStep(tools, null, 'next')).toBe('b');
  expect(flyoutStep(tools, null, 'prev')).toBe('d');
  expect(flyoutStep(group(['a', 'b', 'c', 'd']).tools, null, 'first')).toBeNull();
});

test('flyoutFocus: the checked entry, else the first enabled one (AC-08)', () => {
  expect(flyoutFocus(group(), 'c')).toBe('c');
  // The active tool is in another group.
  expect(flyoutFocus(group(['a'], ['a', 'b', 'c']), 'd')).toBe('b');
  // A disabled active tool is never focused: it cannot be chosen.
  expect(flyoutFocus(group(['c']), 'c')).toBe('a');
});

test('placeFlyout: right of a vertical rail, top edges aligned (AC-11)', () => {
  const button = { left: 6, top: 200, right: 42, bottom: 236 };
  const rail = { left: 0, top: 90, right: 48, bottom: 800 };
  const at = placeFlyout(button, rail, { width: 180, height: 220 }, { width: 1280, height: 900 }, 'vertical');
  expect(at.top).toBe(200);
  expect(at.left).toBeGreaterThanOrEqual(48);
  expect(at.maxHeight).toBeGreaterThanOrEqual(220);
});

test('placeFlyout: above a horizontal strip, left edges aligned (AC-11)', () => {
  const button = { left: 120, top: 806, right: 164, bottom: 850 };
  const rail = { left: 0, top: 800, right: 400, bottom: 856 };
  const at = placeFlyout(button, rail, { width: 180, height: 220 }, { width: 400, height: 856 }, 'horizontal');
  expect(at.left).toBe(120);
  expect(at.top + 220).toBeLessThanOrEqual(800);
});

test('placeFlyout: shifted back inside the window and height-limited (AC-11)', () => {
  // Strip button near the right edge: the flyout moves left.
  const strip = { left: 0, top: 800, right: 400, bottom: 856 };
  const right = placeFlyout(
    { left: 350, top: 806, right: 394, bottom: 850 },
    strip,
    { width: 180, height: 220 },
    { width: 400, height: 856 },
    'horizontal',
  );
  expect(right.left + 180).toBeLessThanOrEqual(400);
  expect(right.left).toBeGreaterThanOrEqual(0);

  // Rail button near the bottom: the flyout moves up.
  const rail = { left: 0, top: 0, right: 48, bottom: 600 };
  const low = placeFlyout(
    { left: 6, top: 560, right: 42, bottom: 596 },
    rail,
    { width: 180, height: 220 },
    { width: 1000, height: 600 },
    'vertical',
  );
  expect(low.top + 220).toBeLessThanOrEqual(600);

  // A window shorter than the flyout: limited to the window.
  const short = placeFlyout(
    { left: 6, top: 40, right: 42, bottom: 76 },
    { left: 0, top: 0, right: 48, bottom: 150 },
    { width: 180, height: 220 },
    { width: 1000, height: 150 },
    'vertical',
  );
  expect(short.maxHeight).toBeLessThanOrEqual(150);
  expect(short.top).toBeGreaterThanOrEqual(0);
  expect(short.top + short.maxHeight).toBeLessThanOrEqual(150);
});
