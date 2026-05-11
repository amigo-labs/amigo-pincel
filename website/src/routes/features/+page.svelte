<script lang="ts">
  import SeoHead from '$lib/components/SeoHead.svelte';
  import FeatureSection from '$lib/components/FeatureSection.svelte';
  import { featureSections } from '$lib/data/features';
</script>

<SeoHead
  title="Features — Pincel"
  description="Every feature in Pincel, deep-dive style: canvas, tools, layers, animation, tilemaps, slices, palette, file format, tablet support, PWA, and what's coming next."
/>

<div class="page">
  <header class="page-head">
    <p class="caption">Feature deep-dive</p>
    <h1 class="h-display">Everything Pincel does today.</h1>
    <p class="lede">
      Dense, honest, screenshot-led. Where a feature is still in progress, we say so. Where it's
      planned but not built, we say that too.
    </p>
  </header>

  <div class="sections">
    {#each featureSections as section, i}
      <FeatureSection {section} index={i} />
    {/each}
  </div>
</div>

<style>
  .page {
    max-width: 1100px;
    margin: 0 auto;
    padding: 4rem 1.5rem 2rem;
  }
  .page-head {
    max-width: 56ch;
    margin-bottom: 2rem;
  }
  h1 {
    font-size: 3.5rem;
    line-height: 1;
    margin: 0.5rem 0 1.25rem;
    color: var(--color-fg-primary);
  }
  .lede {
    color: var(--color-fg-secondary);
    font-size: 1.15rem;
    line-height: 1.6;
    margin: 0;
  }
  @media (max-width: 720px) {
    h1 {
      font-size: 2.5rem;
    }
    .page {
      padding: 2rem 1rem;
    }
  }
</style>
