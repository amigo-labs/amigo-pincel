import { readdirSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';
import { expect, test } from '@playwright/test';

// A runes module `foo.svelte.ts` is imported as `./foo.svelte`. On a
// case-insensitive file system (Windows, macOS by default) that specifier
// already names a sibling component `Foo.svelte`, so the bundler takes the
// component before it tries the `.ts` extension and the build fails with
// MISSING_EXPORT; on Linux it finds the module and passes. Such a module
// needs a name of its own, e.g. `keymap-state.svelte.ts`.

const SRC = resolve(process.cwd(), 'src');

function ambiguousRunesModules(dir: string): readonly string[] {
  const entries = readdirSync(dir, { withFileTypes: true });
  const names = entries.map((e) => e.name);
  const here = entries
    .filter((e) => e.isFile() && e.name.endsWith('.svelte.ts'))
    .flatMap((e) => {
      const specifier = e.name.slice(0, -'.ts'.length).toLowerCase();
      const module = relative(SRC, join(dir, e.name)).replaceAll('\\', '/');
      return names
        .filter((n) => n !== e.name && n.toLowerCase() === specifier)
        .map((n) => `${module} is shadowed by ${n}`);
    });
  const below = entries
    .filter((e) => e.isDirectory())
    .flatMap((e) => ambiguousRunesModules(join(dir, e.name)));
  return [...here, ...below];
}

test('no runes module shares its import specifier with a sibling file', () => {
  expect(ambiguousRunesModules(SRC)).toEqual([]);
});
