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
      'Pencil, eraser, bucket, line, rectangles, ellipses, rounded rectangles, polygons, eyedropper, move, text, and rectangle / ellipse / lasso / polygon / magic-wand selections. Each tool is small, focused, and built around pixel-perfect input.',
    shortcuts: [
      { keys: 'B', label: 'pencil' },
      { keys: 'E', label: 'eraser' },
      { keys: 'G', label: 'bucket' },
      { keys: 'I', label: 'eyedropper' },
      { keys: 'V', label: 'move' },
      { keys: 'M', label: 'selection' },
      { keys: 'U', label: 'shapes' },
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
      'A timeline with per-frame durations, named tags with forward / reverse / ping-pong playback, onion skin, and sprite-sheet export with Aseprite-format JSON. Everything round-trips through the .aseprite file.',
    shortcuts: [
      { keys: ', / .', label: 'prev/next frame' },
      { keys: 'Enter', label: 'play / pause' },
    ],
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
      'New sprites start with DB32. Recolor, add, delete and drag-reorder entries; named entries survive the .aseprite round-trip. Palette presets and import / export are next.',
  },
  {
    id: 'file-format',
    icon: 'file',
    heading: 'File Format',
    description:
      'Aseprite read and write are the source of truth — layer flags, user data and grid settings survive the trip. PNG frame export and sprite sheets with Aseprite-compatible JSON. Image mode saves OpenRaster (.ora). No proprietary format lock-in, ever.',
  },
  {
    id: 'tablet',
    icon: 'tablet',
    heading: 'Tablet & Pen',
    description:
      'Pointer events throughout, two-finger pan and pinch-to-zoom in both modes. Pressure, tilt and on-screen modifier keys for iPad are next.',
    status: 'in-progress',
  },
  {
    id: 'image-mode',
    icon: 'brush',
    heading: 'Image Mode',
    description:
      'A paint.net-style editor in the same app for everything that is not pixel art: PNG, JPEG, WebP, BMP, GIF and TIFF open with layers, blend modes, brushes, shapes, text, selections, transforms, and the full effects and adjustments set. Large canvases render viewport-first. Save as OpenRaster to keep the layers, export flat images.',
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
