<script lang="ts">
  import { m } from '#lib/i18n/index.ts';
  import Rich from '#lib/components/Rich.svelte';
  import SeoHead from '#lib/components/SeoHead.svelte';
  import PixelIcon from '#lib/components/PixelIcon.svelte';
</script>

<SeoHead title={m().about.seoTitle} description={m().about.seoDescription} />

<div class="page">
  <header class="page-head">
    <p class="caption">{m().about.caption}</p>
    <h1 class="h-display">{m().about.title}</h1>
  </header>

  <article class="prose">
    <p><Rich text={m().about.p1} /></p>
    <p><Rich text={m().about.p2} /></p>
    <p><Rich text={m().about.p3} /></p>
  </article>

  <section id="built-in-pincel" class="block">
    <h2>{m().about.builtTitle}</h2>
    <p>{m().about.built}</p>
  </section>

  <section class="block">
    <h2>{m().about.teamTitle}</h2>
    <p><Rich text={m().about.team} /></p>
    <a class="btn-pixel btn-pixel--secondary" href="https://github.com/amigo-labs/amigo-pincel">
      <PixelIcon name="github" size={16} />
      {m().about.contribute}
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
    margin-bottom: 2.5rem;
  }
  h1 {
    font-size: 3rem;
    line-height: 1.05;
    margin: 0.5rem 0 0;
    color: var(--color-fg-primary);
  }
  .prose p {
    color: var(--color-fg-primary);
    font-size: 1.125rem;
    line-height: 1.75;
    margin: 0 0 1.5rem;
  }
  .prose :global(a) {
    color: var(--color-brand-primary);
  }
  .block {
    margin-top: 4rem;
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
    margin: 0 0 1.5rem;
  }
  .block :global(a) {
    color: var(--color-brand-primary);
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
