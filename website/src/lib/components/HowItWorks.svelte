<script lang="ts">
  import PixelIcon from './PixelIcon.svelte';
</script>

<ol class="steps">
  <li>
    <div class="step-icon"><PixelIcon name="file" size={40} /></div>
    <h3>1. Open</h3>
    <p>Drag in a <code>.aseprite</code> file or start fresh.</p>
  </li>
  <li class="arrow" aria-hidden="true"><PixelIcon name="arrow-right" size={32} /></li>
  <li>
    <div class="step-icon"><PixelIcon name="brush" size={40} /></div>
    <h3>2. Edit</h3>
    <p>Paint, animate, build tilemaps, define slices.</p>
  </li>
  <li class="arrow" aria-hidden="true"><PixelIcon name="arrow-right" size={32} /></li>
  <li>
    <div class="step-icon"><PixelIcon name="frame" size={40} /></div>
    <h3>3. Save</h3>
    <p>Save back to <code>.aseprite</code>. Your engine hot-reloads.</p>
  </li>
</ol>

<style>
  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: 1fr auto 1fr auto 1fr;
    gap: 1rem;
    align-items: center;
  }
  .steps > li {
    text-align: center;
  }
  .steps > li:not(.arrow) {
    background-color: var(--color-bg-card);
    border: 1px solid var(--color-border-subtle);
    border-top: 4px solid var(--color-brand-primary);
    padding: 2rem 1.5rem;
  }
  .step-icon {
    color: var(--color-brand-primary);
    margin-bottom: 0.75rem;
  }
  h3 {
    margin: 0 0 0.5rem;
    font-size: 1.15rem;
    color: var(--color-fg-primary);
  }
  p {
    margin: 0;
    color: var(--color-fg-secondary);
    font-size: 0.95rem;
  }
  .arrow {
    color: var(--color-fg-muted);
  }
  @media (max-width: 760px) {
    .steps {
      grid-template-columns: 1fr;
    }
    .arrow {
      transform: rotate(90deg);
    }
  }
</style>
