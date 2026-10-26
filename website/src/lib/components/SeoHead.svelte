<script lang="ts">
  import { page } from '$app/state';
  import { absoluteUrl, siteUrl } from '#lib/config.ts';
  import { LANGS, lang, localized, m, unlocalized } from '#lib/i18n/index.ts';

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
    // PNG, not SVG — Twitter/Facebook/LinkedIn crawlers don't render
    // SVG og:images, so link previews came up blank. The PNG is
    // rasterized from static/og/default.svg (1200×630).
    ogImage = '/og/default.png',
    ogImageType = 'image/png',
    noindex = false,
    jsonLd = null,
  }: Props = $props();

  const fullTitle = $derived(title.includes('Pincel') ? title : `${title} — Pincel`);
  // page.url uses SvelteKit's prerender placeholder origin at build time, so we
  // derive canonical URLs from the configured production origin instead.
  const url = $derived(absoluteUrl(page.url.pathname));
  // The same page in every language, for hreflang alternates.
  const base = $derived(unlocalized(page.url.pathname));
  const otherLocales = $derived(
    LANGS.filter((l) => l !== lang()).map((l) => (l === 'de' ? 'de_DE' : 'en_US')),
  );
  const ogImageUrl = $derived(ogImage.startsWith('http') ? ogImage : `${siteUrl}${ogImage}`);
  const jsonLdString = $derived(jsonLd ? JSON.stringify(jsonLd) : null);
</script>

<svelte:head>
  <title>{fullTitle}</title>
  <meta name="description" content={description} />
  {#if noindex}
    <meta name="robots" content="noindex" />
  {/if}
  <link rel="canonical" href={url} />
  {#each LANGS as l (l)}
    <link rel="alternate" hreflang={l} href={absoluteUrl(localized(base, l))} />
  {/each}
  <link rel="alternate" hreflang="x-default" href={absoluteUrl(localized(base, 'en'))} />

  <meta property="og:title" content={fullTitle} />
  <meta property="og:description" content={description} />
  <meta property="og:image" content={ogImageUrl} />
  <meta property="og:image:type" content={ogImageType} />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="630" />
  <meta property="og:url" content={url} />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Pincel" />
  <meta property="og:locale" content={m().meta.locale} />
  {#each otherLocales as l (l)}
    <meta property="og:locale:alternate" content={l} />
  {/each}

  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={fullTitle} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={ogImageUrl} />

  {#if jsonLdString}
    <!-- eslint-disable-next-line svelte/no-at-html-tags -- JSON.stringify of a typed literal, no user input -->
    {@html `<script type="application/ld+json">${jsonLdString}</` + `script>`}
  {/if}
</svelte:head>
