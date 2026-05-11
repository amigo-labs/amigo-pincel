<script lang="ts">
  import PixelIcon from './PixelIcon.svelte';
</script>

<div class="wrap">
  <a class="btn-pixel big" href="/app">
    Open Editor
    <PixelIcon name="arrow-right" size={20} />
  </a>
  <p class="sub">
    Or: <code>npm install @amigo-labs/pincel</code> to <a href="/embed">embed it</a> →
  </p>
</div>

<style>
  .wrap {
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1.25rem;
  }
  .big {
    font-size: 1.1rem;
    padding: 1rem 2rem;
  }
  .sub {
    color: var(--color-fg-secondary);
    font-size: 0.95rem;
    margin: 0;
  }
  .sub a {
    color: var(--color-brand-primary);
  }
</style>
