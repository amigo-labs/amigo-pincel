// Builds the blind judge pool: copies each version's folder under a shuffled label and strips
// whatever would tell the judge which version is the old one or who built it.
//
// Usage: node blind.mjs <out-dir> <name>=<dir> <name>=<dir> [--strip w1,w2]
//
// - Every *.md in a copied folder is sanitized: sections whose heading mentions the champion, a
//   comparison, a checklist or run notes are dropped; the first line becomes "# Version <label>";
//   champion/incumbent/baseline, round ids (R0, r2c1, c3), builder ids, ports and --strip words
//   become "…". Other files (screenshots) are copied as they are.
// - <out-dir>.key.json (beside the pool, not in it) maps labels to names; never hand it to the judge.
import fs from 'node:fs';
import path from 'node:path';

const args = process.argv.slice(2);
const out = args.shift();
const flag = (name) => {
  const i = args.indexOf(`--${name}`);
  if (i < 0) return undefined;
  const [, value] = args.splice(i, 2);
  return value;
};
const extra = (flag('strip') ?? '').split(',').filter(Boolean);
const versions = args.map((a) => {
  const [name, dir] = a.split('=');
  return { name, dir };
});
if (!out || versions.length < 2 || versions.some((v) => !v.dir)) {
  console.error('usage: node blind.mjs <out-dir> <name>=<dir> <name>=<dir> [--strip w1,w2]');
  process.exit(2);
}

// Fisher–Yates with crypto randomness, so labels carry no order.
const pool = [...versions];
for (let i = pool.length - 1; i > 0; i--) {
  const j = crypto.getRandomValues(new Uint32Array(1))[0] % (i + 1);
  [pool[i], pool[j]] = [pool[j], pool[i]];
}

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const words = [...extra, ...versions.map((v) => path.basename(v.dir))].filter((w) => w.length > 2);
const SECTION = /^#+ .*(champion|incumbent|comparison|compared|next to|checklist|run notes|baseline)/i;
const TOKENS = new RegExp(
  [
    '\\b(champion|incumbent|baseline)\\b',
    '\\b[Rr]\\d+(?:[-/ ]?c\\d+)?\\b',
    '\\bc\\d\\b',
    '\\b(?:compose|humanize|ev)-r\\d+-c\\d+\\b',
    ':\\d{4,5}\\b',
    ...words.map((w) => `\\b${escape(w)}\\b`),
  ].join('|'),
  'gi',
);

function sanitize(text, label) {
  const lines = text.split('\n');
  const kept = [];
  let skipLevel = 0;
  for (const [i, line] of lines.entries()) {
    const heading = /^(#+) /.exec(line);
    if (heading) {
      const level = heading[1].length;
      if (skipLevel && level <= skipLevel) skipLevel = 0;
      if (i > 0 && SECTION.test(line)) {
        skipLevel = level;
        continue;
      }
    }
    if (skipLevel) continue;
    kept.push(i === 0 ? `# Version ${label}` : line.replace(TOKENS, '…'));
  }
  return kept.join('\n');
}

function copy(src, dst, label) {
  fs.mkdirSync(dst, { recursive: true });
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name);
    const to = path.join(dst, entry.name);
    if (entry.isDirectory()) copy(from, to, label);
    else if (entry.name.endsWith('.md')) fs.writeFileSync(to, sanitize(fs.readFileSync(from, 'utf8'), label));
    else fs.copyFileSync(from, to);
  }
}

fs.mkdirSync(out, { recursive: true });
const key = {};
pool.forEach((v, i) => {
  const label = String.fromCharCode(65 + i);
  key[label] = v.name;
  copy(v.dir, path.join(out, label), label);
});
const keyFile = `${out.replace(/\/+$/, '')}.key.json`;
fs.writeFileSync(keyFile, JSON.stringify(key, null, 2));
console.log(`blind pool: ${pool.length} versions → ${out}/{${Object.keys(key).join(',')}}; key in ${keyFile}`);
