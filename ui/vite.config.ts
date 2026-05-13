import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import { VitePWA } from 'vite-plugin-pwa';

// `injectManifest` is the strategy mandated by docs/specs/pincel.md
// §10.1 — `src/sw.ts` is the custom service worker source. The
// plugin handles the build-time rewrite of `self.__WB_MANIFEST`,
// injects the registration script into `index.html`, and emits the
// resolved web app manifest from the `manifest:` block below. WASM
// bytes are explicitly added to the glob patterns so the wasm-pack
// output is precached alongside the JS / CSS / HTML.
//
// Tauri 2 integration:
//   * `clearScreen: false` keeps the Tauri runner's startup logs
//     visible (Vite would otherwise clear them on dev-server boot).
//   * `envPrefix` adds `TAURI_ENV_` so the CLI-injected env vars
//     (`TAURI_ENV_PLATFORM`, `_ARCH`, `_FAMILY`, `_DEBUG`,
//     `_TARGET_TRIPLE`) are exposed via `import.meta.env`. Vite
//     matches prefixes via `startsWith`, so a literal trailing `*`
//     would never match anything.
//   * `server.strictPort` makes Vite fail loudly if 5173 is taken so
//     Tauri's `devUrl` never points at the wrong process.
export default defineConfig({
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  plugins: [
    svelte(),
    tailwindcss(),
    VitePWA({
      strategies: 'injectManifest',
      srcDir: 'src',
      filename: 'sw.ts',
      registerType: 'autoUpdate',
      injectRegister: 'auto',
      manifest: {
        name: 'Pincel',
        short_name: 'Pincel',
        description: 'Pixel-art editor for game asset creation.',
        start_url: '/',
        scope: '/',
        display: 'standalone',
        background_color: '#0a0a0a',
        theme_color: '#0a0a0a',
        icons: [
          {
            src: 'favicon.svg',
            sizes: 'any',
            type: 'image/svg+xml',
            purpose: 'any maskable',
          },
        ],
      },
      injectManifest: {
        // Default globs miss the wasm bytes the wasm-pack output
        // emits into `assets/`. The wasm bundle is the heaviest
        // single asset; precaching it is the difference between
        // "offline reload works" and "white screen".
        globPatterns: ['**/*.{js,css,html,wasm,svg,webmanifest}'],
      },
    }),
  ],
  server: {
    strictPort: true,
    fs: {
      allow: ['../crates/pincel-wasm/pkg'],
    },
  },
});
