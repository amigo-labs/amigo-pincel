<script lang="ts">
  interface Props {
    id?: string;
    eyebrow?: string;
    title?: string;
    align?: 'left' | 'center';
    tone?: 'default' | 'elevated';
    children?: import('svelte').Snippet;
  }

  const {
    id,
    eyebrow,
    title,
    align = 'left',
    tone = 'default',
    children,
  }: Props = $props();
</script>

<section {id} class:elevated={tone === 'elevated'} class:center={align === 'center'}>
  <div class="container">
    {#if eyebrow || title}
      <header class="head">
        {#if eyebrow}<p class="caption">{eyebrow}</p>{/if}
        {#if title}<h2 class="h-display">{title}</h2>{/if}
      </header>
    {/if}
    {@render children?.()}
  </div>
</section>

<style>
  section {
    padding: 6rem 1.5rem;
  }
  section.elevated {
    background-color: var(--color-bg-card);
    border-top: 1px solid var(--color-border-subtle);
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .container {
    max-width: 1200px;
    margin: 0 auto;
  }
  .center {
    text-align: center;
  }
  .center .head {
    margin-inline: auto;
  }
  .head {
    margin-bottom: 2.5rem;
    max-width: 48rem;
  }
  .head h2 {
    font-size: 2.5rem;
    margin: 0.5rem 0 0;
    color: var(--color-fg-primary);
  }
  @media (max-width: 720px) {
    section {
      padding: 3rem 1rem;
    }
    .head h2 {
      font-size: 2rem;
    }
  }
</style>
