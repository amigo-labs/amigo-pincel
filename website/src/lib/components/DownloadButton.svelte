<script lang="ts">
  // Download CTA. Without JavaScript (and on systems without a desktop
  // build) it links to the download page; in the browser it offers the
  // installer for the visitor's system directly. `detectedOnly` shows it
  // only once a system is detected (on the download page itself).
  import { onMount } from 'svelte';
  import { localized, m } from '#lib/i18n/index.ts';
  import { downloadUrl, downloads } from '#lib/config.ts';
  import { detectPlatform, type Platform } from '#lib/platform.ts';
  import PixelIcon from './PixelIcon.svelte';

  interface Props {
    class?: string;
    size?: number;
    detectedOnly?: boolean;
  }
  const {
    class: className = 'btn-pixel btn-pixel--secondary',
    size = 16,
    detectedOnly = false,
  }: Props = $props();

  let platform = $state<Platform | null>(null);
  onMount(() => {
    platform = detectPlatform(navigator);
  });

  const files = {
    windows: downloads.windowsSetup,
    macos: downloads.macos,
    linux: downloads.linuxAppImage,
  } as const;

  const href = $derived(platform ? downloadUrl(files[platform]) : localized('/download'));
  const label = $derived(platform ? m().download.for[platform] : m().nav.download);
</script>

{#if platform || !detectedOnly}
  <a class={className} {href}>
    <PixelIcon name="download" {size} />
    {label}
  </a>
{/if}
