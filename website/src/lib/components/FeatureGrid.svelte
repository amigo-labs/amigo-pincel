<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import PixelIcon from './PixelIcon.svelte';

  type IconName = ComponentProps<typeof PixelIcon>['name'];

  interface Tile {
    icon: IconName;
    title: string;
    description: string;
  }

  const tiles: Tile[] = [
    {
      icon: 'file',
      title: 'Aseprite-Compatible',
      description: 'Read and write .aseprite files. Drop into your game pipeline.',
    },
    {
      icon: 'tile',
      title: 'Tilemaps',
      description: 'First-class tilemap layers with tilesets. Stamp, swap, edit in place.',
    },
    {
      icon: 'slice',
      title: 'Slices',
      description: 'Named regions, 9-patch, pivot points — for hitboxes, UI, and game logic.',
    },
    {
      icon: 'frame',
      title: 'Animation',
      description: 'Frames, tags, onion skin, ping-pong. Tag your states; export sheets.',
    },
    {
      icon: 'palette',
      title: 'Palette Workflow',
      description: 'Indexed color, palette swaps, named colors. Pixel-art done right.',
    },
    {
      icon: 'tablet',
      title: 'Tablet & Pen',
      description: 'Pointer events with pressure, tilt, pinch-to-zoom. Pencil-friendly.',
    },
    {
      icon: 'offline',
      title: 'Offline PWA',
      description:
        'Install it. Works without a connection. Your files stay on your device.',
    },
    {
      icon: 'plug',
      title: 'Embeddable',
      description: 'npm package. Drop into your tool. Same editor, in your app.',
    },
    {
      icon: 'opensource',
      title: 'Open Source',
      description: 'MIT / Apache 2.0. Forkable, auditable, yours.',
    },
  ];
</script>

<div class="grid">
  {#each tiles as t}
    <article class="card-pixel tile">
      <PixelIcon name={t.icon} size={48} class="icon" />
      <h3>{t.title}</h3>
      <p>{t.description}</p>
    </article>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1.5rem;
  }
  .tile {
    padding: 1.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .tile :global(.icon) {
    color: var(--color-brand-primary);
  }
  .tile h3 {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
    color: var(--color-fg-primary);
  }
  .tile p {
    margin: 0;
    color: var(--color-fg-secondary);
    font-size: 0.95rem;
    line-height: 1.55;
  }
  @media (max-width: 900px) {
    .grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }
  @media (max-width: 600px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
