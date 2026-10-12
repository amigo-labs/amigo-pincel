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
//
// Base path: the web build is served at https://pincel.amigo-labs.dev/app/
// (the marketing site owns `/`; docs/specs/pincel.md §15, 2026-10-11), so
// it is built with `base: '/app/'` into `dist/app/` — scripts/cf-build.sh
// copies that folder into the site's build, so one Worker serves both. The Tauri CLI sets `TAURI_ENV_PLATFORM` for
// its before-dev / before-build commands; the native shell loads the bundle
// from its own origin, so it keeps the root base and `dist/`.
const isTauri = Boolean(process.env.TAURI_ENV_PLATFORM);
const base = isTauri ? '/' : '/app/';

export default defineConfig({
  base,
  build: {
    outDir: isTauri ? 'dist' : 'dist/app',
    emptyOutDir: true,
  },
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
        description: 'Pixel-art editor for game asset creation, plus a paint.net-style image editor.',
        start_url: base,
        scope: base,
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
        globPatterns: ['**/*.{js,css,html,wasm,svg,webmanifest,ttf}'],
        // Workbox refuses to precache files over 2 MiB by default. The
        // unoptimized dev-profile wasm files (CI builds them via `pnpm
        // wasm:build --dev`; wasm-opt is disabled there) are well above
        // that — ~3.9 MB (pincel-wasm) and ~8.5 MB (pincel-image-wasm,
        // since OpenRaster brought in zip / deflate) on the stable
        // toolchain — and excluding them would break offline entirely;
        // give them headroom instead (the limit is per file). The release
        // wasm that ships stays far below this.
        maximumFileSizeToCacheInBytes: 16 * 1024 * 1024,
      },
    }),
  ],
  server: {
    strictPort: true,
    fs: {
      allow: ['../crates/pincel-wasm/pkg', '../crates/pincel-image-wasm/pkg'],
    },
  },
});
