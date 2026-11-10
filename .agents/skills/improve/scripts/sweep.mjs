// Width sweep of a served page: finds overlapping text, horizontal scroll and an unreachable top.
//
// Usage: node sweep.mjs <url> [--setup <file.mjs>] [--pkg <dir>] [--from 320] [--to 1440] [--step 8]
//
// Playwright is resolved from <dir>/package.json (default: the first of ".", "ui", "web",
// "frontend", "app" that has one), so the script needs no install of its own.
//
// <file.mjs> may export `states`: an array of { name, run(page, lang) } that brings a freshly
// loaded page into the state to sweep (seed IndexedDB, open a card, …), and `langs` (default
// ['en']). Without --setup the page is swept as it loads. Exit code 1 when anything is found.
//
// Text runs are clipped to their nearest overflow-clipping ancestor, so ellipsis-truncated
// text does not count as overlapping its neighbour.
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const args = process.argv.slice(2);
const url = args[0];
const opt = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i > 0 ? args[i + 1] : fallback;
};
if (!url) {
  console.error(
    'usage: node sweep.mjs <url> [--setup file.mjs] [--pkg dir] [--from 320] [--to 1440] [--step 8]',
  );
  process.exit(2);
}
const from = Number(opt('from', 320));
const to = Number(opt('to', 1440));
const step = Number(opt('step', 8));
const setupPath = opt('setup');
const setup = setupPath ? await import(pathToFileURL(path.resolve(setupPath)).href) : {};
const states = setup.states ?? [{ name: 'default', run: async () => {} }];
const langs = setup.langs ?? ['en'];

const pkgDir =
  opt('pkg') ?? ['.', 'ui', 'web', 'frontend', 'app'].find((d) => fs.existsSync(path.join(d, 'package.json')));
if (!pkgDir) {
  console.error('sweep: no package.json found; pass --pkg <dir> of the package that has Playwright');
  process.exit(2);
}
const require = createRequire(path.resolve(pkgDir, 'package.json'));
const load = (name) => {
  try {
    return require(name);
  } catch {
    return null;
  }
};
const playwright = load('@playwright/test') ?? load('playwright');
if (!playwright) {
  console.error(`sweep: neither @playwright/test nor playwright resolves from ${pkgDir}/package.json`);
  process.exit(2);
}
const { chromium } = playwright;

/** Runs in the page: one string per problem found at the current width. */
function probe() {
  const root = document.querySelector('main') ?? document.body;
  const issues = [];
  const runs = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) => (n.textContent.trim() ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT),
  });
  for (let t = walker.nextNode(); t; t = walker.nextNode()) {
    const parent = t.parentElement;
    if (!parent || parent.closest('option, select')) continue;
    let clip = parent;
    while (clip && clip !== root && getComputedStyle(clip).overflowX === 'visible') clip = clip.parentElement;
    const box = clip && clip !== root ? clip.getBoundingClientRect() : null;
    const range = document.createRange();
    range.selectNodeContents(t);
    const rects = [...range.getClientRects()]
      .map((r) => {
        if (!box) return r;
        const left = Math.max(r.left, box.left);
        return new DOMRect(left, r.top, Math.min(r.right, box.right) - left, r.height);
      })
      .filter((r) => r.width > 0.5 && r.height > 0.5);
    if (rects.length) runs.push({ text: t.textContent.trim().slice(0, 24), rects });
  }
  for (let i = 0; i < runs.length; i++) {
    for (let j = i + 1; j < runs.length; j++) {
      const hit = runs[i].rects.some((a) =>
        runs[j].rects.some(
          (b) =>
            Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 &&
            Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1,
        ),
      );
      if (hit) issues.push(`overlap "${runs[i].text}" × "${runs[j].text}"`);
    }
  }
  const doc = document.documentElement;
  if (doc.scrollWidth > doc.clientWidth) issues.push('horizontal scroll');
  const first = root.getBoundingClientRect();
  if (first.top + window.scrollY < -1) issues.push('top of the page cannot be scrolled to');
  return issues;
}

const browser = await chromium.launch();
const found = new Map();
let frames = 0;
for (const lang of langs) {
  for (const state of states) {
    const context = await browser.newContext({ viewport: { width: to, height: 900 }, serviceWorkers: 'block' });
    const page = await context.newPage();
    await page.goto(url);
    await state.run(page, lang);
    for (let w = from; w <= to; w += step) {
      await page.setViewportSize({ width: w, height: 900 });
      await page.waitForTimeout(30);
      frames++;
      for (const issue of await page.evaluate(probe)) {
        const key = `${lang}/${state.name}: ${issue}`;
        if (!found.has(key)) found.set(key, []);
        found.get(key).push(w);
      }
    }
    await context.close();
  }
}
await browser.close();

console.log(`sweep: ${frames} frames, ${found.size} problem(s)`);
for (const [key, widths] of found) {
  console.log(`  ${key} @ ${widths[0]}–${widths.at(-1)} px (${widths.length} widths)`);
}
process.exit(found.size ? 1 : 0);
