<script lang="ts">
  import { m } from '$lib/i18n';
  import type { ComponentProps } from 'svelte';
  import PixelIcon from './PixelIcon.svelte';

  type IconName = ComponentProps<typeof PixelIcon>['name'];

  type TileId = keyof ReturnType<typeof m>['grid'];

  const tiles: { id: TileId; icon: IconName }[] = [
    { id: 'aseprite', icon: 'file' },
    { id: 'tilemaps', icon: 'tile' },
    { id: 'slices', icon: 'slice' },
    { id: 'animation', icon: 'frame' },
    { id: 'palette', icon: 'palette' },
    { id: 'tablet', icon: 'tablet' },
    { id: 'image', icon: 'brush' },
    { id: 'offline', icon: 'offline' },
    { id: 'embed', icon: 'plug' },
    { id: 'open', icon: 'opensource' },
  ];
</script>

<div class="grid">
  {#each tiles as tile (tile.id)}
    <article class="card-pixel tile">
      <PixelIcon name={tile.icon} size={48} class="icon" />
      <h3>{m().grid[tile.id].title}</h3>
      <p>{m().grid[tile.id].description}</p>
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
