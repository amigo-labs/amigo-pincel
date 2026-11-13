import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

// The desktop app's file associations (docs/specs/image-formats-more.md
// AC-14, docs/specs/image-formats-more-modern.md AC-10): static checks of
// the bundle config, the Windows installer hooks and the Linux MIME list —
// no browser, no app. pincel-tauri's own tests, which need GTK / WebKit,
// only check that these three agree.

const root = fileURLToPath(new URL('../../', import.meta.url));
const read = (path: string): string => readFileSync(`${root}${path}`, 'utf8');

test('the desktop bundle associates TGA, DDS, ICO and JPEG XL on every platform (image-formats-more AC-14, image-formats-more-modern AC-10)', () => {
  const conf = JSON.parse(read('src-tauri/tauri.conf.json')) as {
    bundle: { fileAssociations: { ext: string[]; mimeType?: string }[] };
  };
  const exts = conf.bundle.fileAssociations.flatMap((a) => a.ext);
  const nsis = read('src-tauri/windows/hooks.nsh');
  const linux = read('src-tauri/src/linux_install.rs');
  for (const [ext, mime] of [
    ['tga', 'image/x-tga'],
    ['dds', 'image/x-dds'],
    ['ico', 'image/vnd.microsoft.icon'],
    ['jxl', 'image/jxl'],
  ] as const) {
    expect(exts, `tauri.conf.json associates .${ext}`).toContain(ext);
    expect(nsis, `the installer registers .${ext}`).toContain(`"${ext}"`);
    expect(linux, `the Linux MIME list has ${mime}`).toContain(`"${mime}"`);
  }
});
