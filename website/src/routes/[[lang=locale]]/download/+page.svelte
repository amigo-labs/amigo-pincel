<script lang="ts">
  import { m } from '#lib/i18n/index.ts';
  import Rich from '#lib/components/Rich.svelte';
  import SeoHead from '#lib/components/SeoHead.svelte';
  import PixelIcon from '#lib/components/PixelIcon.svelte';
  import DownloadButton from '#lib/components/DownloadButton.svelte';
  import { appPath, downloadUrl, downloads, releasesUrl, version } from '#lib/config.ts';

  // Every installer the release publishes (docs/RELEASING.md, "Release
  // files"), per system; the first one is the recommended download.
  const systems = $derived([
    {
      id: 'windows',
      copy: m().download.windows,
      files: [
        { label: m().download.windows.setup, file: downloads.windowsSetup },
        { label: m().download.windows.msi, file: downloads.windowsMsi },
      ],
      after: m().download.windows.firstStart,
    },
    {
      id: 'macos',
      copy: m().download.macos,
      files: [{ label: m().download.macos.dmg, file: downloads.macos }],
      after: m().download.macos.firstStart,
    },
    {
      id: 'linux',
      copy: m().download.linux,
      files: [
        { label: m().download.linux.appimage, file: downloads.linuxAppImage },
        { label: m().download.linux.deb, file: downloads.linuxDeb },
        { label: m().download.linux.rpm, file: downloads.linuxRpm },
      ],
      code: `chmod +x ${downloads.linuxAppImage}\n./${downloads.linuxAppImage} --install`,
      after: m().download.linux.after,
    },
  ]);
</script>

<SeoHead title={m().download.seoTitle} description={m().download.seoDescription} />

<div class="page">
  <header class="page-head">
    <p class="caption">{m().download.caption}</p>
    <h1 class="h-display">{m().download.title}</h1>
    <p class="lede">{m().download.lede.replace('{version}', version)}</p>
    <div class="ctas">
      <DownloadButton class="btn-pixel" detectedOnly />
      <a class="btn-pixel btn-pixel--tertiary" href={releasesUrl}>{m().download.releases}</a>
    </div>
    <nav class="jump" aria-label={m().download.platforms}>
      {#each systems as system (system.id)}
        <a href={`#${system.id}`}>{system.copy.heading}</a>
      {/each}
    </nav>
  </header>

  {#each systems as system (system.id)}
    <section id={system.id} class="block">
      <h2>{system.copy.heading}</h2>
      <p class="requires"><span>{m().download.requires}:</span> {system.copy.requires}</p>
      <div class="files">
        {#each system.files as entry, i (entry.file)}
          <a
            class={i === 0 ? 'btn-pixel' : 'btn-pixel btn-pixel--secondary'}
            href={downloadUrl(entry.file)}
          >
            <PixelIcon name="download" size={16} />
            {entry.label}
          </a>
        {/each}
      </div>
      <p><Rich text={system.copy.install} /></p>
      {#if system.code}
        <pre><code>{system.code}</code></pre>
      {/if}
      <p><Rich text={system.after} /></p>
    </section>
  {/each}

  <section class="block">
    <h2>{m().download.verifyTitle}</h2>
    <p><Rich text={m().download.verify.replace('{sums}', downloadUrl(downloads.sums))} /></p>
  </section>

  <section class="block">
    <h2>{m().download.updatesTitle}</h2>
    <p><Rich text={m().download.updates} /></p>
  </section>

  <section class="block">
    <h2>{m().download.webTitle}</h2>
    <p><Rich text={m().download.web} /></p>
    <a class="btn-pixel btn-pixel--secondary" href={appPath} rel="external">
      {m().nav.openEditor}
      <PixelIcon name="arrow-right" size={16} />
    </a>
  </section>
</div>

<style>
  .page {
    max-width: 760px;
    margin: 0 auto;
    padding: 4rem 1.5rem 2rem;
  }
  .page-head {
    margin-bottom: 1rem;
  }
  h1 {
    font-size: 3rem;
    line-height: 1.05;
    margin: 0.5rem 0 1.25rem;
    color: var(--color-fg-primary);
  }
  .lede {
    color: var(--color-fg-secondary);
    font-size: 1.15rem;
    line-height: 1.6;
    margin: 0;
  }
  .ctas,
  .files {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1rem;
    margin: 1.75rem 0;
  }
  .jump {
    display: flex;
    gap: 1.25rem;
    font-family: var(--font-mono);
    font-size: 0.9rem;
  }
  .jump a {
    color: var(--color-brand-primary);
  }
  .block {
    margin-top: 3.5rem;
    scroll-margin-top: 5rem;
  }
  .block h2 {
    font-family: var(--font-pixel);
    font-size: 1.85rem;
    line-height: 1;
    margin: 0 0 1rem;
    color: var(--color-fg-primary);
  }
  .block p {
    color: var(--color-fg-secondary);
    line-height: 1.65;
    margin: 0 0 1.25rem;
  }
  .block :global(a) {
    color: var(--color-brand-primary);
  }
  .block :global(.btn-pixel) {
    color: var(--color-bg-base);
  }
  .block :global(.btn-pixel--secondary) {
    color: var(--color-fg-primary);
  }
  .requires span {
    color: var(--color-fg-primary);
    font-weight: 600;
  }
  pre {
    overflow-x: auto;
    padding: 1rem 1.25rem;
    margin: 0 0 1.25rem;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border-subtle);
    font-family: var(--font-mono);
    font-size: 0.9rem;
    color: var(--color-fg-primary);
  }
  @media (max-width: 720px) {
    h1 {
      font-size: 2.25rem;
    }
    .page {
      padding: 2rem 1rem;
    }
  }
</style>
