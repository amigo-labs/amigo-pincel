<script lang="ts">
  import Hero from '$lib/components/Hero.svelte';
  import Section from '$lib/components/Section.svelte';
  import FeatureGrid from '$lib/components/FeatureGrid.svelte';
  import ComparisonTable from '$lib/components/ComparisonTable.svelte';
  import HowItWorks from '$lib/components/HowItWorks.svelte';
  import FinalCta from '$lib/components/FinalCta.svelte';
  import SeoHead from '$lib/components/SeoHead.svelte';

  const jsonLd = {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'Pincel',
    description:
      'A pixel-art editor for the web. Aseprite-compatible, open source, free. Make sprites, tilemaps, and animations in your browser.',
    applicationCategory: 'DesignApplication',
    operatingSystem: 'Web, Windows, macOS, Linux, iPadOS',
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    license: 'https://opensource.org/licenses/MIT',
  };
</script>

<SeoHead
  title="Pincel — a pixel-art editor for the web"
  description="Aseprite-compatible pixel-art editor that runs in your browser. Open source, free, with tilemaps, slices, animation, and tablet support. Embeddable via npm."
  {jsonLd}
/>

<Hero />

<Section tone="elevated">
  <p class="pitch">
    Pincel is built for game developers and pixel artists who want a serious tool that runs in a
    browser tab. It reads and writes the Aseprite file format, so your work fits into existing
    pipelines. It supports tilemaps, slices, animation tags, palettes, and indexed color — the
    things you actually need. It runs offline as a PWA, and you can install it on iPad. It's open
    source under MIT or Apache 2.0, and it will stay that way.
  </p>
</Section>

<Section eyebrow="What it does" title="Built for game-dev workflows.">
  <FeatureGrid />
</Section>

<Section
  tone="elevated"
  eyebrow="Honest comparison"
  title="Where it stands today."
>
  <ComparisonTable />
</Section>

<Section eyebrow="In three steps" title="How it works." align="center">
  <HowItWorks />
</Section>

<Section align="center">
  <FinalCta />
</Section>

<style>
  .pitch {
    max-width: 56ch;
    margin: 0 auto;
    font-size: 1.2rem;
    line-height: 1.7;
    color: var(--color-fg-primary);
    text-align: center;
  }
</style>
