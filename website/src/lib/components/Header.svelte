<script lang="ts">
  import { LANGS, lang, localized, m, unlocalized } from '#lib/i18n/index.ts';
  import { appPath } from '#lib/config.ts';
  import { page } from '$app/state';
  import Logo from './Logo.svelte';
  import PixelIcon from './PixelIcon.svelte';

  const navItems = $derived([
    { path: '/', label: m().nav.home },
    { path: '/features', label: m().nav.features },
    { path: '/embed', label: m().nav.devs },
    { path: '/about', label: m().nav.about },
  ]);

  // The current page without its language prefix, for the active state
  // and the language switch.
  const here = $derived(unlocalized(page.url.pathname));

  function isActive(path: string): boolean {
    if (path === '/') return here === '/';
    return here.startsWith(path);
  }
</script>

<header class="site-header">
  <div class="bar">
    <Logo />
    <nav aria-label={m().nav.primary}>
      <ul>
        {#each navItems as item (item.path)}
          <li>
            <a href={localized(item.path)} class:active={isActive(item.path)}>{item.label}</a>
          </li>
        {/each}
      </ul>
    </nav>
    <div class="langs" role="group" aria-label={m().nav.language}>
      {#each LANGS as l (l)}
        <a
          href={localized(here, l)}
          hreflang={l}
          lang={l}
          aria-current={l === lang() ? 'true' : undefined}
          class:active={l === lang()}>{l.toUpperCase()}</a
        >
      {/each}
    </div>
    <a href={appPath} rel="external" class="btn-pixel cta">
      {m().nav.openEditor}
      <PixelIcon name="arrow-right" size={16} />
    </a>
  </div>
</header>

<style>
  .site-header {
    position: sticky;
    top: 0;
    z-index: 50;
    backdrop-filter: blur(8px);
    background-color: color-mix(in srgb, var(--color-bg-base) 80%, transparent);
    border-bottom: 1px solid var(--color-border-subtle);
  }
  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1.5rem;
    max-width: 1200px;
    margin: 0 auto;
    padding: 1rem 1.5rem;
  }
  nav ul {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    list-style: none;
    margin: 0;
    padding: 0;
  }
  nav a {
    color: var(--color-fg-secondary);
    text-decoration: none;
    font-size: 0.95rem;
    font-weight: 500;
    padding: 0.5rem 0;
    border-bottom: 2px solid transparent;
    transition:
      color 120ms ease-out,
      border-color 120ms ease-out;
  }
  nav a:hover,
  nav a.active {
    color: var(--color-fg-primary);
    border-bottom-color: var(--color-brand-primary);
  }
  .langs {
    display: flex;
    gap: 0.5rem;
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 0.8rem;
  }
  .langs a {
    color: var(--color-fg-muted);
    text-decoration: none;
  }
  .langs a:hover,
  .langs a.active {
    color: var(--color-fg-primary);
  }
  .cta {
    font-size: 0.875rem;
    padding: 0.6rem 1rem;
  }
  @media (max-width: 720px) {
    .bar {
      padding: 0.75rem 1rem;
      gap: 0.75rem;
    }
    nav ul {
      gap: 1rem;
    }
    nav a {
      font-size: 0.85rem;
    }
    .cta :global(svg) {
      display: none;
    }
  }
  @media (max-width: 540px) {
    nav {
      display: none;
    }
  }
</style>
