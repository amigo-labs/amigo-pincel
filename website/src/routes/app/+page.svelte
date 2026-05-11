<script lang="ts">
  import SeoHead from '$lib/components/SeoHead.svelte';
  import Logo from '$lib/components/Logo.svelte';
  import PixelIcon from '$lib/components/PixelIcon.svelte';
</script>

<SeoHead
  title="Pincel — Editor"
  description="The Pincel editor."
  noindex
/>

<div class="shell">
  <header class="chrome">
    <Logo size={24} showWordmark={false} />
    <a href="/" class="back">← Back to site</a>
  </header>
  <div class="stage">
    <div class="placeholder">
      <PixelIcon name="brush" size={96} />
      <h1>Editor loading soon.</h1>
      <p>
        The full editor mounts here once <code>@amigo-labs/pincel</code> is published. Until then,
        you can follow progress in
        <a href="https://github.com/amigo-labs/amigo-pincel">the GitHub repo</a>.
      </p>
      <p class="muted">
        This route ships its own bundle, isolated from the marketing site, just like an external
        embed would import the package.
      </p>
    </div>
  </div>
</div>

<style>
  .shell {
    min-height: 100dvh;
    display: flex;
    flex-direction: column;
    background-color: var(--color-bg-base);
  }
  .chrome {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .back {
    color: var(--color-fg-secondary);
    text-decoration: none;
    font-size: 0.85rem;
  }
  .back:hover {
    color: var(--color-fg-primary);
  }
  .stage {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem 1.5rem;
  }
  .placeholder {
    text-align: center;
    max-width: 32em;
    color: var(--color-fg-secondary);
  }
  .placeholder :global(svg) {
    color: var(--color-brand-primary);
  }
  h1 {
    font-family: var(--font-pixel);
    font-size: 2.5rem;
    color: var(--color-fg-primary);
    margin: 1rem 0 0.75rem;
  }
  code {
    color: var(--color-brand-primary);
  }
  a {
    color: var(--color-brand-primary);
  }
  .muted {
    color: var(--color-fg-muted);
    font-size: 0.9rem;
  }
</style>
