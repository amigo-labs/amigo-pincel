// Static checks for a challenger's UI change, before anything is built or judged.
//
// Usage: node ui-lint.mjs [base-ref=HEAD]   (run from the repo root of the checkout)
//
// Looks only at lines the working tree adds against <base-ref> under ui/src:
// - error: raw colours in .svelte files — rgb()/rgba()/hsl(), #hex, color-mix(), and
//   Tailwind neutral-*/gray-*/zinc-*/black/white colour classes (AGENTS.md §5.4: pc-* tokens;
//   the bg-black/NN modal scrim is allowed);
// - warning: other Tailwind palette colours (e.g. amber-400 — used as a warning colour
//   elsewhere, but not a pc-* token);
// - error: i18n keys present in en.ts but not de.ts or the other way round.
// Exit code 1 on any error.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';

const base = process.argv[2] ?? 'HEAD';
const diff = execFileSync('git', ['diff', '-U0', base, '--', 'ui/src'], { encoding: 'utf8' });

const PALETTE =
  'slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose';
const UTIL = 'bg|text|border|ring|outline|fill|stroke|from|via|to|divide|accent|caret|decoration|shadow';
const RAW = /rgba?\(|hsla?\(|color-mix\(|#[0-9a-fA-F]{3,8}\b/;
// `bg-black/NN` is the project's modal scrim (DropDialog, NewDocumentDialog, …) and stays allowed.
const NEUTRAL = new RegExp(`\\b(?:${UTIL})-(?:(?:neutral|gray|zinc|slate|stone)-\\d{2,3}|black(?!/\\d)|white)\\b`);
const COLOURED = new RegExp(`\\b(?:${UTIL})-(?:${PALETTE})-\\d{2,3}\\b`);

const errors = [];
const warnings = [];
let file = '';
let line = 0;
for (const row of diff.split('\n')) {
  if (row.startsWith('+++ ')) {
    file = row.slice(6);
    continue;
  }
  const hunk = /^@@ -\S+ \+(\d+)/.exec(row);
  if (hunk) {
    line = Number(hunk[1]);
    continue;
  }
  if (!row.startsWith('+') || row.startsWith('+++')) continue;
  const text = row.slice(1);
  const at = `${file}:${line}`;
  line++;
  if (!file.endsWith('.svelte')) continue;
  if (/^\s*(\/\/|<!--|\*)/.test(text)) continue;
  if (RAW.test(text) || NEUTRAL.test(text)) errors.push(`${at} raw colour: ${text.trim().slice(0, 100)}`);
  else if (COLOURED.test(text)) warnings.push(`${at} palette colour, not a pc-* token: ${text.trim().slice(0, 100)}`);
}

const keys = (p) =>
  new Set([...fs.readFileSync(p, 'utf8').matchAll(/^\s*'([^']+)'\s*:/gm)].map((m) => m[1]));
const en = keys('ui/src/lib/i18n/en.ts');
const de = keys('ui/src/lib/i18n/de.ts');
for (const k of en) if (!de.has(k)) errors.push(`i18n key missing in de.ts: ${k}`);
for (const k of de) if (!en.has(k)) errors.push(`i18n key missing in en.ts: ${k}`);

for (const w of warnings) console.log(`warning ${w}`);
for (const e of errors) console.log(`error   ${e}`);
console.log(`ui-lint: ${errors.length} error(s), ${warnings.length} warning(s)`);
process.exit(errors.length ? 1 : 0);
