import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // All routes are prerendered (see +layout.ts). The static adapter writes the
    // build/ directory that Cloudflare Workers Builds picks up via the
    // root wrangler.toml's [assets] block. We don't set a SPA fallback here
    // because that would overwrite the prerendered build/index.html.
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      precompress: false,
      strict: true,
    }),
    prerender: {
      // The German pages (website spec §11) are linked from the language
      // switch too; listing them keeps them built if a link ever moves.
      entries: ['*', '/de', '/de/features', '/de/about', '/de/embed'],
      handleHttpError: 'warn',
    },
  },
  compilerOptions: {
    runes: true,
  },
};

export default config;
