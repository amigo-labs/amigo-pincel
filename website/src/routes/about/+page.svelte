<script lang="ts">
  import SeoHead from '$lib/components/SeoHead.svelte';
  import PixelIcon from '$lib/components/PixelIcon.svelte';
</script>

<SeoHead
  title="About — Pincel"
  description="Pincel is part of amigo-labs, an open-source ecosystem of game-development tools built in Rust. Free, MIT or Apache 2.0, no funding round, no subscription tier."
/>

<div class="page">
  <header class="page-head">
    <p class="caption">About</p>
    <h1 class="h-display">A pixel-art editor with a long view.</h1>
  </header>

  <article class="prose">
    <p>
      Pincel is part of <a href="https://amigo-labs.dev">amigo-labs</a>, an open-source ecosystem
      of game-development tools built in Rust. The flagship is amigo-engine — a deterministic
      ECS-based 2D engine with a built-in level editor, AI asset pipelines, and chiptune music
      tooling. Pincel fills the pixel-art-editor gap in that ecosystem, and stands alone for
      anyone outside it.
    </p>

    <p>
      The project is open source under MIT or Apache 2.0 — your choice. The format we use is
      Aseprite's, which is documented and reimplementable. We are not affiliated with the
      Aseprite project, and we don't compete with it: Aseprite has 14 years of refinement and a
      deep feature set we won't catch up to overnight. Pincel exists for people who want a free,
      web-based, embeddable alternative.
    </p>

    <p>
      No funding round. No subscription tier coming. No "open core" gotcha. The plan is to keep
      the tool good and the codebase clean, and to maintain it for as long as people use it.
    </p>
  </article>

  <section id="built-in-pincel" class="block">
    <h2>Built with Pincel itself</h2>
    <p>
      Every pixel-art element on this site — the logo, the hero scene, the icons in the feature
      grid — was made in Pincel. The site is dogfood proof. When we change something in the
      editor, this site is one of the first places we notice if it broke.
    </p>
  </section>

  <section class="block">
    <h2>The team</h2>
    <p>
      Pincel is currently maintained by <strong>Daniel Rück</strong> at amigo-labs. Contributors
      are welcome — see the
      <a href="https://github.com/amigo-labs/amigo-pincel">GitHub repo</a> for open issues and
      the contributing guide.
    </p>
    <a class="btn-pixel btn-pixel--secondary" href="https://github.com/amigo-labs/amigo-pincel">
      <PixelIcon name="github" size={16} />
      Contribute on GitHub
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
  .prose a {
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
  .block a {
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
