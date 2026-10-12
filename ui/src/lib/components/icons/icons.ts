// Pincel's icon set (spec §9.2): hand-drawn 24×24 strokes, no icon-font or
// package dependency. Each icon is a list of path strokes in `currentColor`;
// `fill` paths are painted solid, `dash` paths dashed (selection tools).

/** One stroke of an icon. */
export interface IconPath {
  d: string;
  fill?: boolean;
  dash?: boolean;
}

const RECT = 'M4 5h16v14H4z';
const ELLIPSE = 'M4 12a8 7 0 1 0 16 0a8 7 0 1 0-16 0z';
const GRID = 'M4 4h16v16H4zM4 12h16M12 4v16';

/** Every icon by name. */
export const ICONS = {
  // Tools
  pencil: [{ d: 'M4 20l1-4L16 5l3 3L8 19z' }, { d: 'M14 7l3 3' }],
  brush: [
    { d: 'M20 4l-8.5 8.5' },
    { d: 'M9.5 12.5c-2.5 0-4 1.6-4 3.8 0 1.4-.8 2.7-2 3.2 3 .9 6.4.3 7.6-1.4 1-1.4.9-3.4-1.6-5.6z' },
    { d: 'M18 3l3 3' },
  ],
  eraser: [{ d: 'M3 16l9-9 6 6-6 6H6z' }, { d: 'M8 11l6 6' }, { d: 'M14 19h7' }],
  eyedropper: [
    { d: 'M19.5 4.5a2.1 2.1 0 0 0-3 0L14 7l3 3 2.5-2.5a2.1 2.1 0 0 0 0-3z' },
    { d: 'M12.5 5.5l6 6' },
    { d: 'M15 9l-9 9H4v-2l9-9' },
  ],
  bucket: [
    { d: 'M4 12l7-7 7 7-7 7z' },
    { d: 'M11 5L8 2' },
    { d: 'M20.5 14s1.75 2.25 1.75 3.6a1.75 1.75 0 0 1-3.5 0c0-1.35 1.75-3.6 1.75-3.6z' },
  ],
  line: [{ d: 'M5 19L19 5' }],
  rect: [{ d: RECT }],
  'rect-fill': [{ d: RECT, fill: true }],
  ellipse: [{ d: ELLIPSE }],
  'ellipse-fill': [{ d: ELLIPSE, fill: true }],
  'rounded-rect': [
    { d: 'M8 5h8a4 4 0 0 1 4 4v6a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V9a4 4 0 0 1 4-4z' },
  ],
  polygon: [{ d: 'M12 3l8 4.5v9L12 21l-8-4.5v-9z' }],
  shapes: [{ d: 'M3 3h10v10H3z' }, { d: 'M11.5 16.5a5 5 0 1 0 10 0a5 5 0 1 0-10 0z' }],
  'select-rect': [{ d: RECT, dash: true }],
  'select-ellipse': [{ d: ELLIPSE, dash: true }],
  lasso: [
    { d: 'M6 16C3 14 3 9 7 6.5S17 4 19.5 7 19 14 14 15.5 8 16 6 16' },
    { d: 'M6 16c-.5 1.5 0 3.5 2 4.5' },
  ],
  'polygon-lasso': [{ d: 'M4 17l1-10 8-3 7 7-6 8z', dash: true }],
  wand: [
    { d: 'M3 21L15 9' },
    { d: 'M15 9l3-3' },
    { d: 'M19 2v3M17.5 3.5h3M8 4v3M6.5 5.5h3M20 10v3M18.5 11.5h3' },
  ],
  text: [{ d: 'M5 7V4h14v3' }, { d: 'M12 4v16' }, { d: 'M9 20h6' }],
  move: [
    { d: 'M12 3v18M3 12h18' },
    { d: 'M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3' },
  ],
  stamp: [{ d: GRID }, { d: 'M12 12h8v8h-8z', fill: true }],
  slice: [{ d: 'M4 4h16v16H4z' }, { d: 'M9 4v16M15 4v16M4 9h16M4 15h16', dash: true }],

  // Actions
  menu: [{ d: 'M4 6h16M4 12h16M4 18h16' }],
  undo: [{ d: 'M9 14L4 9l5-5' }, { d: 'M4 9h10a6 6 0 0 1 0 12h-3' }],
  redo: [{ d: 'M15 14l5-5-5-5' }, { d: 'M20 9H10a6 6 0 0 0 0 12h3' }],
  'zoom-in': [
    { d: 'M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0z' },
    { d: 'M20 20l-4-4M11 8v6M8 11h6' },
  ],
  'zoom-out': [{ d: 'M4 11a7 7 0 1 0 14 0a7 7 0 1 0-14 0z' }, { d: 'M20 20l-4-4M8 11h6' }],
  fit: [{ d: 'M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5' }],
  sidebar: [{ d: 'M3 4h18v16H3z' }, { d: 'M15 4v16' }],
  close: [{ d: 'M6 6l12 12M18 6L6 18' }],
  'chevron-down': [{ d: 'M6 9l6 6 6-6' }],
  'chevron-up': [{ d: 'M6 15l6-6 6 6' }],
  'chevron-left': [{ d: 'M15 6l-6 6 6 6' }],
  'chevron-right': [{ d: 'M9 6l6 6-6 6' }],
  swap: [{ d: 'M16 3l4 4-4 4M20 7H8' }, { d: 'M8 21l-4-4 4-4M4 17h12' }],
  plus: [{ d: 'M12 5v14M5 12h14' }],
  minus: [{ d: 'M5 12h14' }],
  play: [{ d: 'M7 4l13 8-13 8z', fill: true }],
  pause: [{ d: 'M6 4h4v16H6zM14 4h4v16h-4z', fill: true }],
  eye: [
    { d: 'M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z' },
    { d: 'M9 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0z' },
  ],
  'eye-off': [
    { d: 'M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z' },
    { d: 'M3 3l18 18' },
  ],
  lock: [{ d: 'M5 11h14v10H5z' }, { d: 'M8 11V7a4 4 0 0 1 8 0v4' }],
  unlock: [{ d: 'M5 11h14v10H5z' }, { d: 'M8 11V7a4 4 0 0 1 7.5-2' }],
  trash: [{ d: 'M4 7h16M9 7V4h6v3' }, { d: 'M6 7l1 13h10l1-13' }],
  grip: [
    {
      d: 'M9 5.5a1.2 1.2 0 1 0 0.01 0zM15 5.5a1.2 1.2 0 1 0 0.01 0zM9 11.5a1.2 1.2 0 1 0 0.01 0zM15 11.5a1.2 1.2 0 1 0 0.01 0zM9 17.5a1.2 1.2 0 1 0 0.01 0zM15 17.5a1.2 1.2 0 1 0 0.01 0z',
      fill: true,
    },
  ],
  rename: [{ d: 'M4 20h16' }, { d: 'M6 16l1-3 8-8 2 2-8 8z' }],
  shift: [{ d: 'M12 4l8 8h-4v7H8v-7H4z' }],
  option: [{ d: 'M3 6h5l7 12h6M14 6h7' }],
  duplicate: [{ d: 'M8 8h12v12H8z' }, { d: 'M4 16V4h12' }],
  'merge-down': [{ d: 'M12 4v12M7 11l5 5 5-5' }, { d: 'M5 20h14' }],
  layers: [{ d: 'M12 3l9 5-9 5-9-5z' }, { d: 'M3 13l9 5 9-5' }],
  palette: [{ d: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z' }],
  colors: [
    { d: 'M3 3h11v11H3z', fill: true },
    { d: 'M10 10h11v11H10z' },
  ],
  grid: [{ d: GRID }],
  film: [{ d: 'M3 5h18v14H3z' }, { d: 'M7 5v14M17 5v14M3 12h4M17 12h4' }],
  more: [{ d: 'M5 12h.01M12 12h.01M19 12h.01' }],
  download: [{ d: 'M12 4v11M7 10l5 5 5-5' }, { d: 'M5 20h14' }],
} satisfies Record<string, IconPath[]>;

/** A known icon name. */
export type IconName = keyof typeof ICONS;
