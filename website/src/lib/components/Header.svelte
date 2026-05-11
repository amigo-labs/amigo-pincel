<script lang="ts">
  import { page } from '$app/state';
  import Logo from './Logo.svelte';
  import PixelIcon from './PixelIcon.svelte';

  const navItems = [
    { href: '/', label: 'Home' },
    { href: '/features', label: 'Features' },
    { href: '/embed', label: 'For Devs' },
    { href: '/about', label: 'About' },
  ];

  function isActive(href: string): boolean {
    if (href === '/') return page.url.pathname === '/';
    return page.url.pathname.startsWith(href);
  }
</script>

<header class="site-header">
  <div class="bar">
    <Logo />
    <nav aria-label="Primary">
      <ul>
        {#each navItems as item}
          <li>
            <a href={item.href} class:active={isActive(item.href)}>{item.label}</a>
          </li>
        {/each}
      </ul>
    </nav>
    <a href="/app" class="btn-pixel cta">
      Open Editor
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
    transition: color 120ms ease-out, border-color 120ms ease-out;
  }
  nav a:hover,
  nav a.active {
    color: var(--color-fg-primary);
    border-bottom-color: var(--color-brand-primary);
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
