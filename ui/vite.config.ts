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
// Base path: the web build is served at the root of
// https://pincel.amigo-labs.dev/ (until 2026-11-11 at /app/; docs/specs/
// pincel.md §15). The web app manifest keeps `id: '/app/'`, the id installed
// copies were given then, so they stay the same app. The Tauri CLI sets
// `TAURI_ENV_PLATFORM` for its before-dev / before-build commands; the native
// shell needs no PWA (see below).
const isTauri = Boolean(process.env.TAURI_ENV_PLATFORM);
const base = '/';

export default defineConfig({
  base,
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  plugins: [
    svelte(),
    tailwindcss(),
    VitePWA({
      // The desktop app embeds its assets, so it needs no offline cache, and
      // WebView2 cannot update a worker served from Tauri's custom protocol:
      // a registered worker froze the UI at its first version
      // (src-tauri/src/webview_data.rs removes the old registration).
      disable: isTauri,
      strategies: 'injectManifest',
      srcDir: 'src',
      filename: 'sw.ts',
      registerType: 'autoUpdate',
      injectRegister: 'auto',
      manifest: {
        name: 'Pincel',
        short_name: 'Pincel',
        description: 'Pixel-art and image editor: Aseprite-compatible sprites, tilemaps and animation, plus layered editing of photos, screenshots and artwork.',
        // The id PWAs installed from /app/ carry (the manifest had no id, so
        // it was the start URL); without it they would be a different app.
        id: '/app/',
        start_url: base,
        scope: base,
        display: 'standalone',
        background_color: '#0a0a0a',
        theme_color: '#0a0a0a',
        // Copies of assets/brand/ (the pixel monogram, generated from
        // docs/site/project.yml by amigo-labs/website scripts/monogram.ts).
        // Not maskable: the frame reaches the edge of the icon.
        icons: [
          { src: 'icon-256.png', sizes: '256x256', type: 'image/png', purpose: 'any' },
          { src: 'icon-512.png', sizes: '512x512', type: 'image/png', purpose: 'any' },
          { src: 'icon.svg', sizes: 'any', type: 'image/svg+xml', purpose: 'any' },
        ],
      },
      injectManifest: {
        // Default globs miss the wasm bytes the wasm-pack output
        // emits into `assets/`. The wasm bundle is the heaviest
        // single asset; precaching it is the difference between
        // "offline reload works" and "white screen".
        globPatterns: ['**/*.{js,css,html,wasm,svg,png,ico,webmanifest,ttf,aseprite,ora}'],
        // Workbox refuses to precache files over 2 MiB by default. The
        // unoptimized dev-profile wasm files (CI builds them via `pnpm
        // wasm:build --dev`; wasm-opt is disabled there) are well above
        // that — ~19 MB (pincel-wasm) and ~21 MB (pincel-image-wasm) on
        // the stable toolchain since both carry the agent tools for live
        // MCP calls (docs/specs/mcp-live-desktop.md) — and excluding them
        // would break offline entirely; give them headroom instead (the
        // limit is per file). The release wasm that ships stays far below
        // this (~5 and ~6 MB before wasm-opt).
        maximumFileSizeToCacheInBytes: 32 * 1024 * 1024,
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
