import { expect, test } from '@playwright/test';
import {
  PANEL_GROUPS,
  groupState,
  normalizeLayout,
} from '../src/lib/components/layout/groups';

// docs/specs/sidebar-tab-groups.md — the stored layout and the state of a
// group, as plain functions (no page): first use (AC-07), a layout stored
// before the groups (AC-09), unreadable or stale values (AC-10).

const DOCUMENT = ['layers', 'history'];
const ASSETS = ['preview', 'tilesets', 'slices'];
const AI = ['generate', 'assistant'];

test.describe('first use (AC-07)', () => {
  const layout = normalizeLayout(undefined);

  test('no stored layout: sidebar shown, nothing collapsed, no group state', () => {
    expect(layout).toEqual({ collapsed: {}, sidebar: true });
  });

  test('Layers | History starts expanded with Layers active', () => {
    expect(groupState(layout, 'document', DOCUMENT)).toEqual({ active: 'layers', collapsed: false });
  });

  test('Preview | Tilesets | Slices starts collapsed with Preview active', () => {
    expect(groupState(layout, 'assets', ASSETS)).toEqual({ active: 'preview', collapsed: true });
  });

  test('Generate | Assistant starts expanded with Assistant active', () => {
    expect(groupState(layout, 'ai', AI)).toEqual({ active: 'assistant', collapsed: false });
  });

  test('the three groups are the ones the spec names', () => {
    expect(Object.keys(PANEL_GROUPS).sort()).toEqual(['ai', 'assets', 'document']);
  });
});

test.describe('a layout stored before the groups (AC-09)', () => {
  const stored = {
    collapsed: { colors: true, palette: false, layers: true, history: false, preview: false, assistant: true },
    sidebar: false,
    sidebarWidth: 320,
    effectPos: { x: 20, y: 30 },
  };
  const layout = normalizeLayout(stored);

  test('single panels keep their collapse state; width, visibility and effect place stay', () => {
    expect(layout.collapsed.colors).toBe(true);
    expect(layout.collapsed.palette).toBe(false);
    expect(layout.sidebar).toBe(false);
    expect(layout.sidebarWidth).toBe(320);
    expect(layout.effectPos).toEqual({ x: 20, y: 30 });
    expect(layout.groups).toBeUndefined();
  });

  test('every group starts as on first use, whatever its panels had stored', () => {
    expect(groupState(layout, 'document', DOCUMENT)).toEqual({ active: 'layers', collapsed: false });
    expect(groupState(layout, 'assets', ASSETS)).toEqual({ active: 'preview', collapsed: true });
    expect(groupState(layout, 'ai', AI)).toEqual({ active: 'assistant', collapsed: false });
  });
});

test.describe('stored group state (AC-08)', () => {
  test('a stored active tab and collapse state come back', () => {
    const layout = normalizeLayout({
      collapsed: {},
      sidebar: true,
      groups: { document: { active: 'history', collapsed: true }, assets: { active: 'slices', collapsed: false } },
    });
    expect(groupState(layout, 'document', DOCUMENT)).toEqual({ active: 'history', collapsed: true });
    expect(groupState(layout, 'assets', ASSETS)).toEqual({ active: 'slices', collapsed: false });
    expect(groupState(layout, 'ai', AI)).toEqual({ active: 'assistant', collapsed: false });
  });
});

test.describe('unreadable or stale values (AC-10)', () => {
  for (const raw of [null, 7, 'layout', [], true]) {
    test(`${JSON.stringify(raw)} is no layout: first-use state`, () => {
      expect(normalizeLayout(raw)).toEqual({ collapsed: {}, sidebar: true });
    });
  }

  test('broken fields are dropped one by one', () => {
    const layout = normalizeLayout({
      collapsed: { colors: 'yes', palette: true },
      sidebar: 'no',
      sidebarWidth: Number.NaN,
      effectPos: { x: 1 },
      groups: { document: { active: 5, collapsed: true }, assets: 'open', ai: { active: 'generate', collapsed: 'x' } },
    });
    expect(layout.collapsed).toEqual({ palette: true });
    expect(layout.sidebar).toBe(true);
    expect(layout.sidebarWidth).toBeUndefined();
    expect(layout.effectPos).toBeUndefined();
    expect(layout.groups).toBeUndefined();
  });

  test('a stored tab the group does not have on this platform falls back to its first-use tab', () => {
    const layout = normalizeLayout({
      collapsed: {},
      sidebar: true,
      groups: { document: { active: 'gone', collapsed: true } },
    });
    // The collapse state is still the stored one.
    expect(groupState(layout, 'document', DOCUMENT)).toEqual({ active: 'layers', collapsed: true });
  });

  test('where the first-use tab is missing too, the first panel is active', () => {
    const layout = normalizeLayout({ collapsed: {}, sidebar: true, groups: { ai: { active: 'gone', collapsed: false } } });
    expect(groupState(layout, 'ai', ['generate'])).toEqual({ active: 'generate', collapsed: false });
    expect(groupState(normalizeLayout(undefined), 'ai', ['generate'])).toEqual({
      active: 'generate',
      collapsed: false,
    });
  });

  test('an unknown group id is expanded on its first panel', () => {
    expect(groupState(normalizeLayout(undefined), 'other', ['a', 'b'])).toEqual({ active: 'a', collapsed: false });
  });
});
