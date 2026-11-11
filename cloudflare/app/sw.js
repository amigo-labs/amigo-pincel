// Stand-in service worker at the editor's old path /app/ (docs/specs/
// pincel.md §15, 2026-11-11: the editor moved to the root).
//
// Copies installed or opened while the editor lived at /app/ still run the
// worker registered as /app/sw.js. It serves the bundle it precached, so on
// its own it would keep showing that old editor forever. Browsers refuse a
// redirected worker script, so /app/sw.js cannot simply redirect: this file
// replaces the old worker on its next update check, deletes the old worker's
// precache, unregisters itself and sends its open windows to the root,
// where the editor's own worker takes over. IndexedDB (documents, recents,
// prefs) belongs to the origin and is not touched.

self.addEventListener('install', () => {
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    (async () => {
      // Workbox names its precache after the registration scope
      // ("workbox-precache-v2-https://pincel.amigo-labs.dev/app/"); the
      // root worker's cache ends in "/" and stays.
      const scope = self.registration.scope;
      const names = await caches.keys();
      await Promise.all(names.filter((name) => name.endsWith(scope)).map((name) => caches.delete(name)));
      // Control the windows the old worker served, so navigate() may move them.
      await self.clients.claim();
      await self.registration.unregister();
      const windows = await self.clients.matchAll({ type: 'window' });
      await Promise.all(windows.map((client) => client.navigate('/')));
    })(),
  );
});
