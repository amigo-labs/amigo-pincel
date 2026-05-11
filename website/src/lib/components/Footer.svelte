<script lang="ts">
  import { crt } from '$lib/stores/crt.svelte';
  import PixelIcon from './PixelIcon.svelte';

  const year = new Date().getFullYear();
</script>

<footer class="site-footer">
  <div class="inner">
    <div class="primary">
      <p class="line-1">
        Pincel — by
        <a href="https://amigo-labs.dev" rel="external">amigo-labs</a>. MIT or Apache-2.0.
      </p>
      <ul class="links" aria-label="Social and feeds">
        <li>
          <a href="https://github.com/amigo-labs/amigo-pincel" rel="external noopener">
            <PixelIcon name="github" size={16} />
            GitHub
          </a>
        </li>
        <li>
          <a href="https://mastodon.gamedev.place/@amigo_labs" rel="external me noopener">
            Mastodon
          </a>
        </li>
        <li>
          <a href="https://bsky.app/profile/amigo-labs.dev" rel="external noopener">Bluesky</a>
        </li>
      </ul>
    </div>
    <div class="meta">
      <p>
        <a href="/about#built-in-pincel">Built with Pincel itself. <span aria-hidden="true">▲</span></a>
      </p>
      <button
        type="button"
        class="crt-toggle"
        aria-pressed={crt.enabled}
        onclick={() => crt.toggle()}
      >
        CRT effect: <span class="state">{crt.enabled ? 'on' : 'off'}</span>
      </button>
      <p class="copy">© {year} amigo-labs</p>
    </div>
  </div>
</footer>

<style>
  .site-footer {
    margin-top: 6rem;
    border-top: 1px solid var(--color-border-subtle);
    padding: 3rem 1.5rem 4rem;
    color: var(--color-fg-secondary);
  }
  .inner {
    max-width: 1200px;
    margin: 0 auto;
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 2rem;
    align-items: start;
  }
  .line-1 {
    margin: 0 0 0.75rem;
    color: var(--color-fg-secondary);
  }
  .line-1 a {
    color: var(--color-fg-primary);
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 1.25rem;
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .links a {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    color: var(--color-fg-secondary);
    text-decoration: none;
    border-bottom: 1px dotted transparent;
  }
  .links a:hover {
    color: var(--color-fg-primary);
    border-bottom-color: var(--color-brand-primary);
  }
  .meta {
    text-align: right;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    align-items: end;
    font-size: 0.875rem;
  }
  .meta a {
    color: var(--color-fg-secondary);
    text-decoration: none;
  }
  .meta a:hover {
    color: var(--color-fg-primary);
  }
  .crt-toggle {
    background: transparent;
    border: 1px solid var(--color-border-subtle);
    padding: 0.4rem 0.75rem;
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--color-fg-secondary);
    cursor: pointer;
    transition: color 120ms, border-color 120ms;
  }
  .crt-toggle:hover {
    color: var(--color-fg-primary);
    border-color: var(--color-border-strong);
  }
  .state {
    color: var(--color-brand-primary);
    font-weight: 500;
  }
  .copy {
    margin: 0;
    color: var(--color-fg-muted);
    font-size: 0.75rem;
  }
  @media (max-width: 720px) {
    .inner {
      grid-template-columns: 1fr;
    }
    .meta {
      text-align: left;
      align-items: start;
    }
  }
</style>
