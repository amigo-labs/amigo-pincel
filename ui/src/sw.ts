/// <reference lib="webworker" />

// Custom service worker for the Pincel PWA. `vite-plugin-pwa`'s
// `injectManifest` strategy hands us `self.__WB_MANIFEST` — the
// precache manifest of every static asset built into `dist/` — and
// `workbox-precaching` registers a fetch handler that serves those
// assets from cache first, falling back to network on misses.
//
// Documents are intentionally NOT cached here. They live in IndexedDB
// (`ui/src/lib/idb/`) — autosave snapshots and the recent-files
// registry — so a fresh tab can pick up where a prior tab left off
// even when the user is fully offline. See docs/specs/pincel.md §10.1.

import { precacheAndRoute } from 'workbox-precaching';

declare const self: ServiceWorkerGlobalScope;

// `__WB_MANIFEST` is the placeholder vite-plugin-pwa rewrites at
// build time into the array of precache entries. `precacheAndRoute`
// hashes the entries into Cache Storage and serves them through a
// `cache-first` strategy, with stale entries swapped out on the next
// activate.
precacheAndRoute(self.__WB_MANIFEST);

// Skip the default "wait for tabs to close" handshake so a fresh
// deploy activates immediately. The cache version is hash-keyed so
// in-flight tabs keep serving the old assets they had open.
self.addEventListener('install', () => {
  void self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(self.clients.claim());
});
