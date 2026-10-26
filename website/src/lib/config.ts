// Site-wide configuration. Edit here, not in components.
//
// `siteUrl` is the canonical production origin used for absolute URLs in
// `<link rel="canonical">`, Open Graph tags, sitemap.xml, and robots.txt. It
// must be an absolute URL with no trailing slash because routes are appended
// directly (e.g. `${siteUrl}/features`).
//
// During prerender SvelteKit uses `http://sveltekit-prerender/` as its dummy
// origin, which is not safe to emit. We hard-code the production origin and
// derive everything from it.
export const siteUrl = 'https://pincel.amigo-labs.dev';

// The editor (`ui/`) is copied into this site's build at `/app/` by
// scripts/cf-build.sh — it is not a SvelteKit route of this site, so links
// to it carry `rel="external"`: the client router does a full page load and
// the prerender crawler skips them. docs/specs/pincel.md §15, 2026-10-13.
export const appPath = '/app/';

export function absoluteUrl(pathname: string): string {
  if (!pathname.startsWith('/')) {
    return `${siteUrl}/${pathname}`;
  }
  return `${siteUrl}${pathname}`;
}

// The released version (ui/package.json, which release-please bumps with the
// release), injected by vite.config.ts.
declare const __PINCEL_VERSION__: string;
export const version = __PINCEL_VERSION__;

export const repoUrl = 'https://github.com/amigo-labs/amigo-pincel';
export const releasesUrl = `${repoUrl}/releases/latest`;

// Desktop downloads: release.yml publishes every file under a stable name,
// so `releases/latest/download/<name>` always serves the newest release
// (docs/RELEASING.md, "Release files"; website spec §12, 2026-10-26).
export const downloads = {
  windowsSetup: 'pincel-windows-x64-setup.exe',
  windowsMsi: 'pincel-windows-x64.msi',
  macos: 'pincel-macos-universal.dmg',
  linuxAppImage: 'pincel-linux-x64.AppImage',
  linuxDeb: 'pincel-linux-x64.deb',
  linuxRpm: 'pincel-linux-x64.rpm',
  sums: 'SHA256SUMS',
} as const;

export function downloadUrl(file: (typeof downloads)[keyof typeof downloads]): string {
  return `${releasesUrl}/download/${file}`;
}
