<script lang="ts">
  import { localized, m } from '$lib/i18n';
  import Rich from '$lib/components/Rich.svelte';
  import SeoHead from '$lib/components/SeoHead.svelte';
  import PixelIcon from '$lib/components/PixelIcon.svelte';

  const install = `npm install @amigo-labs/pincel`;

  const usage = `import { Pincel, ColorMode } from '@amigo-labs/pincel';

const pincel = await Pincel.create({
  width: 64,
  height: 64,
  colorMode: ColorMode.Rgba,
});

pincel.mount(document.getElementById('editor'));

pincel.on('change', () => {
  console.log('document modified');
});

const bytes = await pincel.saveAseprite();`;
</script>

<SeoHead title={m().embed.seoTitle} description={m().embed.seoDescription} />

<div class="page">
  <header class="page-head">
    <p class="caption">{m().embed.caption}</p>
    <h1 class="h-display">{m().embed.title}</h1>
    <p class="lede"><Rich text={m().embed.lede} /></p>
  </header>

  <section class="block">
    <h2>{m().embed.quickStart}</h2>
    <pre><code>{install}</code></pre>
    <pre><code>{usage}</code></pre>
  </section>

  <section class="block">
    <h2>{m().embed.demoTitle}</h2>
    <div class="demo">
      <div class="demo-frame" aria-label={m().embed.demoLabel}>
        <div class="demo-placeholder">
          <PixelIcon name="plug" size={64} />
          <p>{m().embed.demoSoon}</p>
          <p class="muted"><Rich text={m().embed.demoFollow} /></p>
        </div>
      </div>
      <details class="source">
        <summary>{m().embed.viewSource}</summary>
        <pre><code
            >{`<div id="editor" style="height: 480px"></div>
<script type="module">
  import { Pincel } from '@amigo-labs/pincel';
  const pincel = await Pincel.create({ width: 32, height: 32 });
  pincel.mount(document.getElementById('editor'));
</` + `script>`}</code
          ></pre>
      </details>
    </div>
  </section>

  <section class="block">
    <h2>{m().embed.getTitle}</h2>
    <ul class="bullets">
      {#each m().embed.get as item (item)}
        <li><Rich text={item} /></li>
      {/each}
    </ul>
  </section>

  <section class="block">
    <h2>{m().embed.costTitle}</h2>
    <p>{m().embed.cost}</p>
  </section>

  <section class="block">
    <h2>{m().embed.limitsTitle}</h2>
    <ul class="bullets">
      {#each m().embed.limits as item (item)}
        <li>{item}</li>
      {/each}
    </ul>
  </section>

  <section class="cta-block">
    <a class="btn-pixel" href="https://github.com/amigo-labs/amigo-pincel">
      <PixelIcon name="github" size={16} />
      {m().embed.source}
    </a>
    <a class="btn-pixel btn-pixel--secondary" href={localized('/features')}>
      {m().embed.coming}
      <PixelIcon name="arrow-right" size={16} />
    </a>
  </section>
</div>

<style>
  .page {
    max-width: 900px;
    margin: 0 auto;
    padding: 4rem 1.5rem 2rem;
  }
  .page-head {
    margin-bottom: 3rem;
  }
  h1 {
    font-size: 3rem;
    line-height: 1.05;
    margin: 0.5rem 0 1rem;
    color: var(--color-fg-primary);
  }
  .lede {
    color: var(--color-fg-secondary);
    font-size: 1.1rem;
    line-height: 1.65;
    margin: 0;
    max-width: 64ch;
  }
  .lede :global(code),
  .block :global(code) {
    color: var(--color-brand-primary);
  }
  .block {
    margin: 0 0 3rem;
  }
  .block h2 {
    font-family: var(--font-pixel);
    font-size: 1.85rem;
    line-height: 1;
    margin: 0 0 1rem;
    color: var(--color-fg-primary);
  }
  .block p,
  .bullets li {
    color: var(--color-fg-secondary);
    line-height: 1.65;
  }
  .bullets {
    margin: 0;
    padding: 0 0 0 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  pre {
    margin: 0 0 1rem;
  }
  .demo {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .demo-frame {
    width: 100%;
    aspect-ratio: 16 / 9;
    background-color: var(--color-bg-card);
    border: 2px solid var(--color-bg-base);
    box-shadow:
      4px 4px 0 var(--color-bg-base),
      0 0 0 1px var(--color-border-strong);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .demo-placeholder {
    text-align: center;
    color: var(--color-fg-muted);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
  }
  .demo-placeholder p {
    margin: 0;
    color: var(--color-fg-secondary);
  }
  .demo-placeholder .muted {
    color: var(--color-fg-muted);
    font-size: 0.9rem;
  }
  .demo-placeholder :global(a) {
    color: var(--color-brand-primary);
  }
  summary {
    cursor: pointer;
    color: var(--color-fg-secondary);
    font-family: var(--font-mono);
    font-size: 0.85rem;
    padding: 0.5rem 0;
  }
  summary:hover {
    color: var(--color-fg-primary);
  }
  .cta-block {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    margin-top: 4rem;
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
