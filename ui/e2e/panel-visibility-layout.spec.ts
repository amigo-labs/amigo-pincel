import { expect, test } from '@playwright/test';
import {
  groupState,
  nextEnabled,
  normalizeLayout,
  resetPanels,
} from '../src/lib/components/layout/groups';

// docs/specs/panel-visibility.md — which panels are removed, as part of the
// stored layout (no page): the active tab of a group that lost its tab
// (AC-04), a layout without the list (AC-10), Reset Panels (AC-11).

const DOCUMENT = ['layers', 'history'];
const ASSETS = ['preview', 'tilesets', 'slices'];

test.describe('the tab that stands in for a removed one (AC-04)', () => {
  test('an enabled tab stands for itself', () => {
    expect(nextEnabled(ASSETS, 'tilesets', ['preview'])).toBe('tilesets');
  });

  test('the next enabled tab to the right', () => {
    expect(nextEnabled(ASSETS, 'preview', ['preview'])).toBe('tilesets');
    expect(nextEnabled(ASSETS, 'preview', ['preview', 'tilesets'])).toBe('slices');
  });

  test('else the nearest to the left', () => {
    expect(nextEnabled(ASSETS, 'slices', ['slices'])).toBe('tilesets');
    expect(nextEnabled(ASSETS, 'slices', ['slices', 'tilesets'])).toBe('preview');
    expect(nextEnabled(ASSETS, 'tilesets', ['tilesets', 'slices'])).toBe('preview');
  });

  test('none when every tab is removed', () => {
    expect(nextEnabled(ASSETS, 'preview', ASSETS)).toBe('');
  });

  test('a stored layout that names a removed panel as active', () => {
    const layout = normalizeLayout({
      groups: { document: { active: 'layers', collapsed: true }, assets: { active: 'slices', collapsed: false } },
      removed: ['layers', 'slices'],
    });
    // The group stays expanded or collapsed as it was.
    expect(groupState(layout, 'document', DOCUMENT)).toEqual({ active: 'history', collapsed: true });
    expect(groupState(layout, 'assets', ASSETS)).toEqual({ active: 'tilesets', collapsed: false });
  });

  test('the first-use tab removed: its neighbour is active', () => {
    const layout = normalizeLayout({ removed: ['preview'] });
    expect(groupState(layout, 'assets', ASSETS)).toEqual({ active: 'tilesets', collapsed: true });
  });
});

test.describe('the stored list (AC-09, AC-10)', () => {
  test('no stored layout, or one without the list: nothing is removed', () => {
    expect(normalizeLayout(undefined).removed).toBeUndefined();
    expect(normalizeLayout({ collapsed: { colors: true }, sidebar: false }).removed).toBeUndefined();
  });

  test('the list is read back', () => {
    expect(normalizeLayout({ removed: ['tilesets', 'timeline'] }).removed).toEqual(['tilesets', 'timeline']);
  });

  test('a list of the wrong shape is ignored; wrong entries are dropped', () => {
    expect(normalizeLayout({ removed: 'tilesets' }).removed).toBeUndefined();
    expect(normalizeLayout({ removed: { tilesets: true } }).removed).toBeUndefined();
    expect(normalizeLayout({ removed: [1, null, 'slices', 'slices'] }).removed).toEqual(['slices']);
  });

  test('a panel the platform does not have is kept and changes no group', () => {
    const layout = normalizeLayout({ removed: ['assistant'] });
    expect(layout.removed).toEqual(['assistant']);
    expect(groupState(layout, 'document', DOCUMENT)).toEqual({ active: 'layers', collapsed: false });
  });
});

test('Reset Panels: all enabled, first-use groups; sidebar, width and effect position stay (AC-11)', () => {
  const stored = normalizeLayout({
    collapsed: { colors: true },
    sidebar: false,
    sidebarWidth: 320,
    effectPos: { x: 40, y: 60 },
    groups: { document: { active: 'history', collapsed: true } },
    removed: ['palette', 'timeline'],
  });
  const reset = resetPanels(stored);
  expect(reset).toEqual({ collapsed: {}, sidebar: false, sidebarWidth: 320, effectPos: { x: 40, y: 60 } });
  expect(groupState(reset, 'document', DOCUMENT)).toEqual({ active: 'layers', collapsed: false });
  expect(resetPanels(normalizeLayout(undefined))).toEqual({ collapsed: {}, sidebar: true });
});
