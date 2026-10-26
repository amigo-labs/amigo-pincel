<script lang="ts">
  import { m } from '#lib/i18n/index.ts';
  import Rich from './Rich.svelte';
  import { appPath } from '#lib/config.ts';
  import PixelIcon from './PixelIcon.svelte';
  import HeroScene from './HeroScene.svelte';
</script>

<section class="hero crt-target">
  <div class="container">
    <div class="copy">
      <p class="caption">{m().hero.caption}</p>
      <h1 class="h-display"><Rich text={m().hero.title} /></h1>
      <p class="tagline-sub"><Rich text={m().hero.tagline} /></p>
      <div class="ctas">
        <a class="btn-pixel" href={appPath} rel="external">
          {m().nav.openEditor}
          <PixelIcon name="arrow-right" size={16} />
        </a>
        <a class="btn-pixel btn-pixel--secondary" href="https://github.com/amigo-labs/amigo-pincel">
          <PixelIcon name="github" size={16} />
          {m().hero.github}
        </a>
      </div>
    </div>
    <div class="art" aria-hidden="true">
      <HeroScene />
    </div>
  </div>
</section>

<style>
  .hero {
    position: relative;
    padding: 5rem 1.5rem 6rem;
    overflow: hidden;
    background:
      radial-gradient(
        circle at 80% 20%,
        color-mix(in srgb, var(--color-bg-elevated) 60%, transparent),
        transparent 55%
      ),
      var(--color-bg-base);
  }
  .container {
    max-width: 1200px;
    margin: 0 auto;
    display: grid;
    grid-template-columns: 1.1fr 1fr;
    gap: 3rem;
    align-items: center;
  }
  h1 {
    font-size: 4rem;
    line-height: 1;
    margin: 0.75rem 0 1.25rem;
    color: var(--color-fg-primary);
  }
  .tagline-sub :global(code) {
    color: var(--color-brand-primary);
  }
  .ctas {
    display: flex;
    gap: 1rem;
    margin-top: 2rem;
    flex-wrap: wrap;
  }
  .art {
    display: flex;
    justify-content: center;
  }
  @media (max-width: 900px) {
    .container {
      grid-template-columns: 1fr;
    }
    h1 {
      font-size: 3rem;
    }
    .hero {
      padding: 3rem 1rem 4rem;
    }
  }
</style>
