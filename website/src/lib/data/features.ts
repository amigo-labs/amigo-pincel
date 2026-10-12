// Structure of the /features page: one section per id, in page order. The
// copy (heading, description, shortcut labels) lives in `$lib/i18n`, keyed
// by the same id; `shortcuts` here are the key combos, in label order.

import type { ComponentProps } from 'svelte';
import type PixelIcon from '$lib/components/PixelIcon.svelte';
import type { Messages } from '$lib/i18n';

type IconName = ComponentProps<typeof PixelIcon>['name'];

export interface FeatureSection {
  id: keyof Messages['features']['sections'];
  icon: IconName;
  shortcuts?: string[];
  status?: 'shipping' | 'in-progress' | 'planned';
}

export const featureSections: FeatureSection[] = [
  { id: 'canvas', icon: 'brush', shortcuts: ['+ / -', 'Space + drag', '0'] },
  { id: 'tools', icon: 'brush', shortcuts: ['B', 'E', 'G', 'I', 'V', 'M', 'U'] },
  { id: 'layers', icon: 'tile' },
  { id: 'animation', icon: 'frame', shortcuts: [', / .', 'Enter'] },
  { id: 'tilemaps', icon: 'tile', status: 'in-progress' },
  { id: 'slices', icon: 'slice', status: 'in-progress' },
  { id: 'palette', icon: 'palette' },
  { id: 'file-format', icon: 'file' },
  { id: 'tablet', icon: 'tablet', status: 'in-progress' },
  { id: 'image-mode', icon: 'brush' },
  { id: 'pwa', icon: 'offline' },
  { id: 'roadmap', icon: 'sparkle', status: 'planned' },
];
