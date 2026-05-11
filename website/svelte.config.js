import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    // All routes are prerendered (see +layout.ts). The static adapter writes the
    // build/ directory that Cloudflare Pages serves directly. We don't set a SPA
    // fallback here because that would overwrite the prerendered build/index.html.
    adapter: adapter({
      pages: 'build',
      assets: 'build',
      precompress: false,
      strict: true,
    }),
    prerender: {
      entries: ['*'],
      handleHttpError: 'warn',
    },
  },
  compilerOptions: {
    runes: true,
  },
};

export default config;
