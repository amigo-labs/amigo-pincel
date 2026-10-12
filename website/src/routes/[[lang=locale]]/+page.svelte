<script lang="ts">
  import { lang, m } from '$lib/i18n';
  import Hero from '$lib/components/Hero.svelte';
  import Section from '$lib/components/Section.svelte';
  import FeatureGrid from '$lib/components/FeatureGrid.svelte';
  import ComparisonTable from '$lib/components/ComparisonTable.svelte';
  import HowItWorks from '$lib/components/HowItWorks.svelte';
  import FinalCta from '$lib/components/FinalCta.svelte';
  import SeoHead from '$lib/components/SeoHead.svelte';

  const jsonLd = $derived({
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: 'Pincel',
    description: m().home.jsonLd,
    inLanguage: lang(),
    applicationCategory: 'DesignApplication',
    operatingSystem: 'Web, Windows, macOS, Linux, iPadOS',
    offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
    license: 'https://opensource.org/licenses/MIT',
  });
</script>

<SeoHead title={m().home.seoTitle} description={m().home.seoDescription} {jsonLd} />

<Hero />

<Section tone="elevated">
  <p class="pitch">{m().home.pitch}</p>
</Section>

<Section eyebrow={m().home.whatEyebrow} title={m().home.whatTitle}>
  <FeatureGrid />
</Section>

<Section tone="elevated" eyebrow={m().home.compareEyebrow} title={m().home.compareTitle}>
  <ComparisonTable />
</Section>

<Section eyebrow={m().home.howEyebrow} title={m().home.howTitle} align="center">
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
