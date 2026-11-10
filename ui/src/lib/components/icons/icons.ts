// Pincel's icon set (spec §9.2): hand-drawn 24×24 strokes in
// `currentColor`, no icon-font or package dependency. Each icon is a list
// of paths: `fill` paths are painted solid, `dash` paths dashed (selection
// tools), `tone` paths filled at 30 % without a stroke.
// The second tone (`tone`, 30 % fill, no stroke) stands for substance:
// paint, pixels, material, the part of the object that matters. Outlines
// of nothing (selections, hollow shapes) stay empty; filled variants are solid.

/** One stroke of an icon. */
export interface IconPath {
  d: string;
  fill?: boolean;
  dash?: boolean;
  tone?: boolean;
}

const RECT = 'M4 5h16v14H4z';
const ELLIPSE = 'M4 12a8 7 0 1 0 16 0a8 7 0 1 0-16 0z';
const LENS = 'M3.5 10.5a7 7 0 1 0 14 0a7 7 0 1 0-14 0z';
const LENS_HANDLE = 'M15.5 15.5l5 5';
const EYE = 'M2.5 12C4.5 8 8 5.5 12 5.5s7.5 2.5 9.5 6.5c-2 4-5.5 6.5-9.5 6.5S4.5 16 2.5 12z';
const LOCK_BODY = 'M5 11h14v9.5H5z';
const STAMP_BASE = 'M5 12h14a1 1 0 0 1 1 1v3H4v-3a1 1 0 0 1 1-1z';
const LAYER_TOP = 'M12 3.5l9 4.5-9 4.5-9-4.5z';
const SLAB = 'M4 15.5h16V20H4z';

/** Every icon by name. */
export const ICONS = {
  // Tools
  pencil: [
    { d: 'M5.3 15.3l8.5-8.5 3.4 3.4-8.5 8.5z', tone: true },
    { d: 'M4 20l1.3-4.7L15.8 4.8a1.7 1.7 0 0 1 2.4 0l1 1a1.7 1.7 0 0 1 0 2.4L8.7 18.7z' },
    { d: 'M13.8 6.8l3.4 3.4' },
    { d: 'M4 20l.7-2.5 1.8 1.8z', fill: true },
  ],
  brush: [
    { d: 'M20.5 3.5l-8.3 8.3' },
    {
      d: 'M10.6 10.2c-2.9.2-4.6 2.1-4.8 4.8-.2 2-1 3.8-2.8 5.2 4 .7 7.9-.3 9.6-3 .8-1.3 1.1-2.6 1.2-3.8z',
      tone: true,
    },
    {
      d: 'M10.6 10.2c-2.9.2-4.6 2.1-4.8 4.8-.2 2-1 3.8-2.8 5.2 4 .7 7.9-.3 9.6-3 .8-1.3 1.1-2.6 1.2-3.8z',
    },
  ],
  eraser: [
    { d: 'M4 15.5l4.5-4.5 5.5 5.5-2.5 2.5h-4z', tone: true },
    { d: 'M4 15.5L12.5 7l5.5 5.5-6.5 6.5h-4z' },
    { d: 'M8.5 11l5.5 5.5' },
    { d: 'M11.5 19H20' },
  ],
  eyedropper: [
    { d: 'M14.2 7.2l2.2-2.2a1.9 1.9 0 0 1 2.7 2.7l-2.2 2.2z', fill: true },
    { d: 'M12.5 5.5l6 6' },
    { d: 'M10.4 11.4l-4 4L5 19l3.6-1.4 4-4z', tone: true },
    { d: 'M14.4 7.4l-8 8L5 19l3.6-1.4 8-8' },
  ],
  // A tipped bucket, half full, and the drop it pours.
  bucket: [
    { d: 'M4 12h14l-7 7z', tone: true },
    { d: 'M4 12l7-7 7 7-7 7z' },
    { d: 'M11 5L8 2' },
    {
      d: 'M20.5 14s1.75 2.25 1.75 3.6a1.75 1.75 0 0 1-3.5 0c0-1.35 1.75-3.6 1.75-3.6z',
      fill: true,
    },
  ],
  line: [
    { d: 'M5.5 18.5l13-13' },
    {
      d: 'M4 18.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0zM17 5.5a1.5 1.5 0 1 0 3 0a1.5 1.5 0 1 0-3 0z',
      fill: true,
    },
  ],
  rect: [{ d: RECT }],
  'rect-fill': [{ d: RECT, fill: true }],
  ellipse: [{ d: ELLIPSE }],
  'ellipse-fill': [{ d: ELLIPSE, fill: true }],
  'rounded-rect': [
    {
      d: 'M8.5 5h7A4.5 4.5 0 0 1 20 9.5v5a4.5 4.5 0 0 1-4.5 4.5h-7A4.5 4.5 0 0 1 4 14.5v-5A4.5 4.5 0 0 1 8.5 5z',
    },
  ],
  polygon: [{ d: 'M12 3.5l8.8 6.4-3.3 10.4h-11L3.2 9.9z' }],
  triangle: [{ d: 'M12 4.5l8.5 15h-17z' }],
  diamond: [{ d: 'M12 3l7.5 9-7.5 9-7.5-9z' }],
  star: [
    {
      d: 'M12 3.3L14.4 9.3 20.8 9.7 15.9 13.9 17.5 20.1 12 16.7 6.5 20.1 8.1 13.9 3.2 9.7 9.6 9.3z',
    },
  ],
  'block-arrow': [{ d: 'M3 9.5h9.5V5L21 12l-8.5 7v-4.5H3z' }],
  heart: [
    {
      d: 'M12 20.5C7 16.5 3.5 13.3 3.5 9.3A4.6 4.6 0 0 1 12 7a4.6 4.6 0 0 1 8.5 2.3c0 4-3.5 7.2-8.5 11.2z',
    },
  ],
  callout: [
    {
      d: 'M4 4.5h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-8.5L7 20.5v-4H4a1 1 0 0 1-1-1v-10a1 1 0 0 1 1-1z',
    },
  ],
  shapes: [
    { d: 'M3.5 3.5h10v10h-10z' },
    { d: 'M10.5 15.5a5 5 0 1 0 10 0a5 5 0 1 0-10 0z', tone: true },
    { d: 'M10.5 15.5a5 5 0 1 0 10 0a5 5 0 1 0-10 0z' },
  ],
  'select-rect': [{ d: RECT, dash: true }],
  'select-ellipse': [{ d: ELLIPSE, dash: true }],
  lasso: [
    { d: 'M6.3 15C4.3 13.9 3 12.1 3 10c0-3.3 4-6 9-6s9 2.7 9 6-4 6-9 6c-.9 0-1.8-.1-2.7-.3' },
    { d: 'M5.7 16.5a1.8 1.8 0 1 0 3.6 0a1.8 1.8 0 1 0-3.6 0z' },
    { d: 'M7 18.3c-.3 1.2-1.2 2.2-2.5 2.7' },
  ],
  'polygon-lasso': [
    { d: 'M5 17.5L6 7l8-3 6 7.5-6 8z', dash: true },
    { d: 'M3.5 16h3v3h-3z', fill: true },
  ],
  wand: [
    { d: 'M4 20l9-9' },
    { d: 'M16.5 3l1.2 3.3 3.3 1.2-3.3 1.2-1.2 3.3-1.2-3.3L12 7.5l3.3-1.2z', tone: true },
    { d: 'M16.5 3l1.2 3.3 3.3 1.2-3.3 1.2-1.2 3.3-1.2-3.3L12 7.5l3.3-1.2z' },
    { d: 'M7 4v3M5.5 5.5h3M19.5 14.5v3M18 16h3' },
  ],
  text: [{ d: 'M5 7.5V5h14v2.5' }, { d: 'M12 5v14' }, { d: 'M9.5 19h5' }],
  hand: [
    { d: 'M8 12h11.2v3a6 6 0 0 1-6 6h-1.6a6 6 0 0 1-5-2.6L5 15.5l3-.3z', tone: true },
    { d: 'M8 12V6.2a1.4 1.4 0 0 1 2.8 0V11' },
    { d: 'M10.8 10.5V4.4a1.4 1.4 0 0 1 2.8 0V11' },
    { d: 'M13.6 11V5.6a1.4 1.4 0 0 1 2.8 0V12' },
    {
      d: 'M16.4 8.4a1.4 1.4 0 0 1 2.8 0V15a6 6 0 0 1-6 6h-1.6a6 6 0 0 1-5-2.6L4 14.6a1.4 1.4 0 0 1 2.3-1.6L8 15.2V12',
    },
  ],
  move: [
    { d: 'M12 3v18M3 12h18' },
    {
      d: 'M9.2 5.8L12 3l2.8 2.8M9.2 18.2L12 21l2.8-2.8M5.8 9.2L3 12l2.8 2.8M18.2 9.2L21 12l-2.8 2.8',
    },
  ],
  recolor: [
    { d: 'M3 3h8v8H3z', fill: true },
    { d: 'M13 13h8v8h-8z', tone: true },
    { d: 'M13 13h8v8h-8z' },
    { d: 'M14 5h1a2 2 0 0 1 2 2v3' },
    { d: 'M14.8 8.3L17 10.5l2.2-2.2' },
  ],
  // A fingertip dragged through wet paint.
  smudge: [
    {
      d: 'M2.5 18.5c0-2 2.5-3.5 6-3.5 4.5 0 6.5 2 13 1.5-1 2.7-4.7 4.5-9.7 4.5-5.3 0-9.3-.5-9.3-2.5z',
      tone: true,
    },
    { d: 'M2.5 18.5c0 2 4 2.5 9.3 2.5 5 0 8.7-1.8 9.7-4.5' },
    { d: 'M18.2 2.2l-8.5 8.5a2.6 2.6 0 0 0 3.6 3.6l8.5-8.5z' },
    { d: 'M9.3 13.3a2.6 2.6 0 0 0 4 1l-2.6-3.6z', fill: true },
  ],
  'clone-stamp': [
    { d: STAMP_BASE, tone: true },
    { d: STAMP_BASE },
    { d: 'M9.5 12c0-2-1-2.8-1-4.8a3.5 3.5 0 0 1 7 0c0 2-1 2.8-1 4.8' },
    { d: 'M4 20h16' },
  ],
  gradient: [
    { d: 'M3 5h6v14H3z', fill: true },
    { d: 'M9 5h6v14H9z', tone: true },
    { d: 'M3 5h18v14H3z' },
  ],
  'move-selection': [
    { d: 'M2.5 2.5h9v9h-9z', tone: true },
    { d: 'M2.5 2.5h9v9h-9z', dash: true },
    { d: 'M16.5 11.5v10M11.5 16.5h10' },
    {
      d: 'M14.7 13.3l1.8-1.8 1.8 1.8M14.7 19.7l1.8 1.8 1.8-1.8M13.3 14.7l-1.8 1.8 1.8 1.8M19.7 14.7l1.8 1.8-1.8 1.8',
    },
  ],
  'move-mask': [{ d: 'M9 9h12v12H9z', dash: true }, { d: 'M7 7L3 3M3 7V3h4' }],
  // A square block stamp over the row of tiles it prints.
  stamp: [
    { d: 'M9.5 2.5h5V8h-5z' },
    { d: 'M5 8h14v4.5H5z', fill: true },
    { d: 'M4 16.5h16v4H4z', tone: true },
    { d: 'M4 16.5h16v4H4zM9.3 16.5v4M14.7 16.5v4' },
  ],
  slice: [
    { d: 'M4 4h4.5v4.5H4zM15.5 4H20v4.5h-4.5zM4 15.5h4.5V20H4zM15.5 15.5H20V20h-4.5z', tone: true },
    { d: 'M4 4h16v16H4z' },
    { d: 'M8.5 4v16M15.5 4v16M4 8.5h16M4 15.5h16' },
  ],

  // Actions
  menu: [{ d: 'M4 6h16M4 12h16M4 18h16' }],
  undo: [{ d: 'M8 5L4 9l4 4' }, { d: 'M4 9h10.5a5.5 5.5 0 0 1 0 11H10' }],
  redo: [{ d: 'M16 5l4 4-4 4' }, { d: 'M20 9H9.5a5.5 5.5 0 0 0 0 11H14' }],
  'zoom-in': [
    { d: LENS, tone: true },
    { d: LENS },
    { d: LENS_HANDLE },
    { d: 'M10.5 7.5v6M7.5 10.5h6' },
  ],
  'zoom-out': [{ d: LENS, tone: true }, { d: LENS }, { d: LENS_HANDLE }, { d: 'M7.5 10.5h6' }],
  // Plain corner brackets: a filled centre read as "record".
  fit: [{ d: 'M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5' }],
  sidebar: [{ d: 'M15 4h6v16h-6z', tone: true }, { d: 'M3 4h18v16H3z' }, { d: 'M15 4v16' }],
  close: [{ d: 'M6 6l12 12M18 6L6 18' }],
  'chevron-down': [{ d: 'M6 9l6 6 6-6' }],
  'chevron-up': [{ d: 'M6 15l6-6 6 6' }],
  'chevron-left': [{ d: 'M15 6l-6 6 6 6' }],
  'chevron-right': [{ d: 'M9 6l6 6-6 6' }],
  swap: [{ d: 'M16.5 3.5L20 7l-3.5 3.5M20 7H7' }, { d: 'M7.5 13.5L4 17l3.5 3.5M4 17h13' }],
  plus: [{ d: 'M12 5v14M5 12h14' }],
  minus: [{ d: 'M5 12h14' }],
  play: [{ d: 'M7.5 4.5l12 7.5-12 7.5z', fill: true }],
  pause: [{ d: 'M6.5 5h3.5v14H6.5zM14 5h3.5v14H14z', fill: true }],
  eye: [{ d: EYE }, { d: 'M9.25 12a2.75 2.75 0 1 0 5.5 0a2.75 2.75 0 1 0-5.5 0z', fill: true }],
  'eye-off': [{ d: EYE, tone: true }, { d: EYE }, { d: 'M4 4l16 16' }],
  lock: [{ d: LOCK_BODY, tone: true }, { d: LOCK_BODY }, { d: 'M8 11V7.5a4 4 0 0 1 8 0V11' }],
  unlock: [{ d: LOCK_BODY }, { d: 'M8 11V7.5a4 4 0 0 1 7.7-1.5' }],
  trash: [
    { d: 'M6 7h12l-.9 13H6.9z', tone: true },
    { d: 'M4 7h16M9.5 7V4.5h5V7' },
    { d: 'M6 7l.9 13h10.2L18 7' },
  ],
  grip: [
    {
      d: 'M8 6a1 1 0 1 0 2 0a1 1 0 1 0-2 0zM14 6a1 1 0 1 0 2 0a1 1 0 1 0-2 0zM8 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0zM14 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0zM8 18a1 1 0 1 0 2 0a1 1 0 1 0-2 0zM14 18a1 1 0 1 0 2 0a1 1 0 1 0-2 0z',
      fill: true,
    },
  ],
  rename: [
    { d: 'M3 8h18v8H3z', tone: true },
    { d: 'M11.5 8H21v8h-9.5M5.5 8H3v8h2.5' },
    { d: 'M8.5 5v14M6.5 4.5c1 0 2 .2 2 1 0-.8 1-1 2-1M6.5 19.5c1 0 2-.2 2-1 0 .8 1 1 2 1' },
  ],
  shift: [{ d: 'M12 3.5l8.5 8.5H16v7.5H8V12H3.5z' }],
  option: [{ d: 'M3 6.5h5l7.5 11H21M14 6.5h7' }],
  'folder-plus': [
    { d: 'M3 8h18v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z', tone: true },
    { d: 'M3 6.5a1 1 0 0 1 1-1h5l2 2.5h9a1 1 0 0 1 1 1V18a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z' },
    { d: 'M12 11v5M9.5 13.5h5' },
  ],
  duplicate: [
    { d: 'M8.5 8.5H20V20H8.5z', tone: true },
    { d: 'M8.5 8.5H20V20H8.5z' },
    { d: 'M4 15.5V4h11.5' },
  ],
  'merge-down': [{ d: SLAB, tone: true }, { d: SLAB }, { d: 'M12 3v8.5M8 8l4 4 4-4' }],
  layers: [{ d: LAYER_TOP, tone: true }, { d: LAYER_TOP }, { d: 'M3 12.5l9 4.5 9-4.5' }],
  // The painter's palette: four swatches collided with grid and slice.
  palette: [
    {
      d: 'M12 3a9 9 0 1 0 0 18c1.3 0 2.2-.9 2.2-2.1 0-.6-.2-1.1-.6-1.5-.3-.4-.5-.8-.5-1.3 0-1.1.9-2 2-2H17a4 4 0 0 0 4-4c0-3.9-4-7.1-9-7.1z',
    },
    {
      d: 'M7.5 11a.5.5 0 1 0 0 1a.5.5 0 1 0 0-1zM10 6.8a.5.5 0 1 0 0 1a.5.5 0 1 0 0-1zM15 6.8a.5.5 0 1 0 0 1a.5.5 0 1 0 0-1z',
      fill: true,
    },
  ],
  colors: [
    { d: 'M3.5 3.5h10v10h-10z', fill: true },
    { d: 'M16.5 9.5h4v11h-11v-4h7z', tone: true },
    { d: 'M16.5 9.5h4v11h-11v-4' },
  ],
  grid: [{ d: 'M9 3.5v17M15 3.5v17M3.5 9h17M3.5 15h17' }],
  film: [
    { d: 'M3 5h4v14H3zM17 5h4v14h-4z', tone: true },
    { d: 'M3 5h18v14H3z' },
    { d: 'M7 5v14M17 5v14M3 12h4M17 12h4' },
  ],
  more: [
    {
      d: 'M4.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM11.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0zM18.4 12a.6.6 0 1 0 1.2 0a.6.6 0 1 0-1.2 0z',
      fill: true,
    },
  ],
  download: [{ d: 'M12 3.5v11M7.5 10L12 14.5 16.5 10' }, { d: 'M4 16v4h16v-4' }],
  history: [{ d: 'M4 12a8 8 0 1 0 1.07-4' }, { d: 'M3.5 4v4h4' }, { d: 'M12 8v4.5l3 1.5' }],
  rewind: [
    { d: 'M11 5.5L4.5 12l6.5 6.5z', fill: true },
    { d: 'M19.5 5.5L13 12l6.5 6.5z', fill: true },
  ],
  'fast-forward': [
    { d: 'M4.5 5.5L11 12l-6.5 6.5z', fill: true },
    { d: 'M13 5.5l6.5 6.5-6.5 6.5z', fill: true },
  ],
  // Document modes (tab bar marker)
  // A pixel staircase (a checker sat too close to colors).
  'mode-pixel': [
    { d: 'M4 14h6v6H4z', fill: true },
    { d: 'M10 8h6v6h-6z', fill: true },
    { d: 'M16 4h4v4h-4z', fill: true },
  ],
  'mode-image': [
    { d: 'M3.5 17l5-5.5 4.5 4.5 2.5-2.5 5 5V19h-17z', tone: true },
    { d: 'M3.5 5h17v14h-17z' },
    { d: 'M3.5 17l5-5.5 4.5 4.5 2.5-2.5 5 5' },
    { d: 'M16 9h.01' },
  ],
} satisfies Record<string, IconPath[]>;

/** A known icon name. */
export type IconName = keyof typeof ICONS;
