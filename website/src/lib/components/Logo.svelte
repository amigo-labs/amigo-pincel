<script lang="ts">
  interface Props {
    size?: number;
    showWordmark?: boolean;
  }

  const { size = 32, showWordmark = true }: Props = $props();
</script>

<a href="/" class="logo" aria-label="Pincel — home">
  <svg
    width={size}
    height={size}
    viewBox="0 0 16 16"
    aria-hidden="true"
    class="pixelated mark"
    shape-rendering="crispEdges"
  >
    <!-- Brush stroke motif in brand-primary (PICO-8 blue, see spec §5.2) on a dark square -->
    <rect x="0" y="0" width="16" height="16" fill="#0d0d12" />
    <rect x="1" y="1" width="14" height="14" fill="#1a1a23" />
    <!-- Diagonal pink brush stroke -->
    <g fill="#29adff">
      <rect x="11" y="2" width="2" height="2" />
      <rect x="10" y="3" width="3" height="2" />
      <rect x="9" y="4" width="3" height="2" />
      <rect x="8" y="5" width="3" height="2" />
      <rect x="7" y="6" width="3" height="2" />
      <rect x="6" y="7" width="3" height="2" />
      <rect x="5" y="8" width="3" height="2" />
      <rect x="4" y="9" width="3" height="2" />
      <rect x="3" y="10" width="3" height="2" />
      <rect x="2" y="11" width="3" height="2" />
    </g>
    <!-- Drip -->
    <rect x="2" y="13" width="1" height="1" fill="#29adff" />
    <!-- Highlight -->
    <rect x="12" y="3" width="1" height="1" fill="#ffec27" />
  </svg>
  {#if showWordmark}
    <span class="wordmark">PINCEL</span>
  {/if}
</a>

<style>
  .logo {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    text-decoration: none;
    color: var(--color-fg-primary);
  }
  .wordmark {
    font-family: var(--font-pixel);
    font-size: 1.6rem;
    letter-spacing: 0.05em;
    line-height: 1;
  }
  .mark {
    border: 2px solid var(--color-bg-base);
    box-shadow: 2px 2px 0 var(--color-bg-base);
  }
</style>
