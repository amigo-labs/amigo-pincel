import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

// The release profile in the root Cargo.toml is the one chosen by
// measurement (docs/specs/performance-build-simd.md AC-01, candidate B under
// Decisions): fat LTO, one codegen unit, nothing else. A change to it needs a
// new measurement and a new Decisions entry, then this test changes with it.

const CARGO_TOML = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'Cargo.toml');

/** Candidate B of the 2026-11-06 measurement, as TOML values. */
const CHOSEN: Readonly<Record<string, string>> = { lto: '"fat"', 'codegen-units': '1' };

/** One TOML table: its header (without brackets) and its `key = value` pairs. */
type Table = { readonly name: string; readonly entries: Readonly<Record<string, string>> };

/** The tables of a TOML file, enough for the flat `[profile.*]` tables:
 *  comments and blank lines dropped, values kept as written. */
function tables(toml: string): Table[] {
  const out: { name: string; entries: Record<string, string> }[] = [];
  for (const raw of toml.split('\n')) {
    const line = raw.replace(/#.*$/, '').trim();
    if (line === '') continue;
    const header = /^\[([^\]]+)\]$/.exec(line);
    if (header) {
      out.push({ name: (header[1] ?? '').trim(), entries: {} });
      continue;
    }
    const pair = /^([A-Za-z0-9_.-]+)\s*=\s*(.+)$/.exec(line);
    const current = out.at(-1);
    if (pair && current) current.entries[pair[1] ?? ''] = (pair[2] ?? '').trim();
  }
  return out;
}

const profileTables = (): Table[] =>
  tables(readFileSync(CARGO_TOML, 'utf8')).filter((t) => t.name === 'profile' || t.name.startsWith('profile.'));

test('the release profile is the measured candidate B (AC-01)', () => {
  const release = profileTables().filter((t) => t.name === 'profile.release');
  expect(release, 'exactly one [profile.release] in Cargo.toml').toHaveLength(1);
  expect(release[0]?.entries).toEqual(CHOSEN);
});

test('the release profile overrides neither opt-level nor panic (AC-01)', () => {
  const release = profileTables().find((t) => t.name === 'profile.release');
  expect(Object.keys(release?.entries ?? {})).not.toContain('opt-level');
  expect(Object.keys(release?.entries ?? {})).not.toContain('panic');
  // `[profile] release.opt-level = …` would set it as well.
  const dotted = profileTables()
    .filter((t) => t.name === 'profile')
    .flatMap((t) => Object.keys(t.entries))
    .filter((k) => k.startsWith('release.'));
  expect(dotted).toEqual([]);
});

test('the release profile has no package or build overrides (AC-01)', () => {
  const overrides = profileTables()
    .map((t) => t.name)
    .filter((name) => name.startsWith('profile.release.'));
  expect(overrides).toEqual([]);
});
