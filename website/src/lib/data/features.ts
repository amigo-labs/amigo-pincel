import type { ComponentProps } from 'svelte';
import type PixelIcon from '$lib/components/PixelIcon.svelte';

type IconName = ComponentProps<typeof PixelIcon>['name'];

export interface FeatureSection {
  id: string;
  icon: IconName;
  heading: string;
  description: string;
  shortcuts?: { keys: string; label: string }[];
  status?: 'shipping' | 'in-progress' | 'planned';
}

export const featureSections: FeatureSection[] = [
  {
    id: 'canvas',
    icon: 'brush',
    heading: 'The Canvas',
    description:
      'Smooth zoom and pan up to 64×, with pixel-perfect rendering. Cursor-anchored wheel zoom, auto-fit on open, and WebGPU rendering with a Canvas2D fallback.',
    shortcuts: [
      { keys: '+ / -', label: 'zoom' },
      { keys: 'Space + drag', label: 'pan' },
      { keys: '0', label: 'fit to view' },
    ],
  },
  {
    id: 'tools',
    icon: 'brush',
    heading: 'Tools',
    description:
      'Pencil, eraser, bucket, line, rectangle, ellipse, eyedropper, move, and a rectangular selection. Each tool is small, focused, and built around pixel-perfect input.',
    shortcuts: [
      { keys: 'B', label: 'pencil' },
      { keys: 'E', label: 'eraser' },
      { keys: 'G', label: 'bucket' },
      { keys: 'I', label: 'eyedropper' },
      { keys: 'V', label: 'move' },
      { keys: 'M', label: 'selection' },
    ],
  },
  {
    id: 'layers',
    icon: 'tile',
    heading: 'Layers',
    description:
      'Image layers, group layers, blend modes, opacity, and visibility. The Aseprite layer model — round-trips through Pincel and back into Aseprite without loss.',
  },
  {
    id: 'animation',
    icon: 'frame',
    heading: 'Frames & Animation',
    description:
      'Frame stepping today; the full timeline with per-frame duration, named tags, onion skin, ping-pong playback, and sprite-sheet export is in the works. Tags already survive the round-trip.',
    shortcuts: [{ keys: '◀ / ▶', label: 'prev/next frame' }],
    status: 'in-progress',
  },
  {
    id: 'tilemaps',
    icon: 'tile',
    heading: 'Tilemaps',
    description:
      'First-class tilemap layers. Build a tileset, stamp tiles on the canvas, flip and rotate per-tile, and edit a tile in place to update everywhere it appears.',
    status: 'in-progress',
  },
  {
    id: 'slices',
    icon: 'slice',
    heading: 'Slices',
    description:
      'Define named rectangular regions, optionally with 9-patch borders and pivot points. Per-frame keys for animated regions. Your engine reads them straight out of the .aseprite file.',
    status: 'in-progress',
  },
  {
    id: 'palette',
    icon: 'palette',
    heading: 'Palette',
    description:
      'Indexed color mode with named entries, palette swap, and import/export. PICO-8, NES, GameBoy, and DB32 presets planned. Palettes already survive the .aseprite round-trip.',
    status: 'in-progress',
  },
  {
    id: 'file-format',
    icon: 'file',
    heading: 'File Format',
    description:
      'Aseprite read and write are the source of truth. PNG and sprite-sheet export with sidecar JSON are on the roadmap. No proprietary format lock-in, ever.',
  },
  {
    id: 'tablet',
    icon: 'tablet',
    heading: 'Tablet & Pen',
    description:
      'Pointer events with pressure, tilt, and twist where the device supports them. Two-finger pan, pinch-to-zoom, and on-screen modifier keys to make it usable on iPad.',
    status: 'planned',
  },
  {
    id: 'pwa',
    icon: 'offline',
    heading: 'PWA',
    description:
      'Install Pincel from your browser. It works offline, autosaves to IndexedDB, and keeps your files on your device. Use the File System Access API where available; fall back to download otherwise.',
  },
  {
    id: 'roadmap',
    icon: 'sparkle',
    heading: 'What\'s Coming',
    description:
      'Native desktop builds via Tauri. Lua scripting. Custom brushes. Headless Node API for asset pipelines. A read-only viewer for embeds. We ship in small, honest increments.',
    status: 'planned',
  },
];
