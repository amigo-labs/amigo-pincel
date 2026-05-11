<script lang="ts">
  import { page } from '$app/state';
  import { absoluteUrl, siteUrl } from '$lib/config';

  interface Props {
    title: string;
    description: string;
    ogImage?: string;
    ogImageType?: string;
    noindex?: boolean;
    jsonLd?: Record<string, unknown> | null;
  }

  const {
    title,
    description,
    ogImage = '/og/default.svg',
    ogImageType = 'image/svg+xml',
    noindex = false,
    jsonLd = null,
  }: Props = $props();

  const fullTitle = $derived(title.includes('Pincel') ? title : `${title} — Pincel`);
  // page.url uses SvelteKit's prerender placeholder origin at build time, so we
  // derive canonical URLs from the configured production origin instead.
  const url = $derived(absoluteUrl(page.url.pathname));
  const ogImageUrl = $derived(
    ogImage.startsWith('http') ? ogImage : `${siteUrl}${ogImage}`,
  );
  const jsonLdString = $derived(jsonLd ? JSON.stringify(jsonLd) : null);
</script>

<svelte:head>
  <title>{fullTitle}</title>
  <meta name="description" content={description} />
  {#if noindex}
    <meta name="robots" content="noindex" />
  {/if}
  <link rel="canonical" href={url} />

  <meta property="og:title" content={fullTitle} />
  <meta property="og:description" content={description} />
  <meta property="og:image" content={ogImageUrl} />
  <meta property="og:image:type" content={ogImageType} />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:url" content={url} />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Pincel" />

  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={fullTitle} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={ogImageUrl} />

  {#if jsonLdString}
    <!-- eslint-disable-next-line svelte/no-at-html-tags -- JSON.stringify of a typed literal, no user input -->
    {@html `<script type="application/ld+json">${jsonLdString}</` + `script>`}
  {/if}
</svelte:head>
