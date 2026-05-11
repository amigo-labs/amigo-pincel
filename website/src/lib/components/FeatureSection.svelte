<script lang="ts">
  import type { FeatureSection } from '$lib/data/features';
  import PixelIcon from './PixelIcon.svelte';

  interface Props {
    section: FeatureSection;
    index: number;
  }

  const { section, index }: Props = $props();
  const flip = $derived(index % 2 === 1);
</script>

<section id={section.id} class="feature" class:flip>
  <div class="text">
    <div class="icon"><PixelIcon name={section.icon} size={32} /></div>
    <h2>
      {section.heading}
      {#if section.status === 'in-progress'}
        <span class="badge in-progress">in progress</span>
      {:else if section.status === 'planned'}
        <span class="badge planned">planned</span>
      {/if}
    </h2>
    <p>{section.description}</p>
    {#if section.shortcuts}
      <ul class="shortcuts" aria-label="Keyboard shortcuts">
        {#each section.shortcuts as s (s.keys)}
          <li><kbd>{s.keys}</kbd> <span>{s.label}</span></li>
        {/each}
      </ul>
    {/if}
  </div>
  <div class="art" aria-hidden="true">
    <div class="placeholder">
      <PixelIcon name={section.icon} size={96} />
      <p>screenshot</p>
    </div>
  </div>
</section>

<style>
  .feature {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 3rem;
    align-items: center;
    padding: 4rem 0;
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .feature:last-of-type {
    border-bottom: none;
  }
  .feature.flip .text {
    order: 2;
  }
  .feature.flip .art {
    order: 1;
  }
  .icon {
    color: var(--color-brand-primary);
    margin-bottom: 0.75rem;
  }
  h2 {
    font-family: var(--font-pixel);
    font-size: 2.25rem;
    line-height: 1.05;
    margin: 0 0 1rem;
    color: var(--color-fg-primary);
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  p {
    color: var(--color-fg-secondary);
    font-size: 1.05rem;
    line-height: 1.65;
    margin: 0 0 1.25rem;
    max-width: 38em;
  }
  .shortcuts {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem 1.25rem;
  }
  .shortcuts li {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.85rem;
    color: var(--color-fg-secondary);
  }
  kbd {
    font-family: var(--font-mono);
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border-strong);
    border-bottom-width: 2px;
    padding: 0.15rem 0.5rem;
    font-size: 0.8rem;
    color: var(--color-fg-primary);
  }
  .badge {
    font-size: 0.7rem;
    font-family: var(--font-body);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    padding: 0.2rem 0.5rem;
    border: 1px solid currentColor;
  }
  .badge.in-progress {
    color: var(--color-accent-warn);
  }
  .badge.planned {
    color: var(--color-fg-muted);
  }
  .art {
    display: flex;
    justify-content: center;
  }
  .placeholder {
    width: 100%;
    max-width: 480px;
    aspect-ratio: 4 / 3;
    background-color: var(--color-bg-card);
    border: 2px dashed var(--color-border-strong);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    color: var(--color-fg-muted);
  }
  .placeholder p {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  @media (max-width: 800px) {
    .feature {
      grid-template-columns: 1fr;
      gap: 1.5rem;
      padding: 2.5rem 0;
    }
    .feature.flip .text,
    .feature.flip .art {
      order: unset;
    }
    h2 {
      font-size: 1.85rem;
    }
  }
</style>
