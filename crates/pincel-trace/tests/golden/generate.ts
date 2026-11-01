// Golden corpus generator for pincel-trace (docs/specs/vectorize-core.md,
// AC-01 and AC-22). Runs the reference implementation — the owner's
// Vectorizer — on the spec's test images and option sets and writes its
// output as text, plus the reference timings of the three benchmark images.
//
//   node --experimental-strip-types generate.ts <vectorizer-checkout> > corpus.txt
//
// It copies the reference core into a temporary directory, adds `.ts` to
// its relative imports (Node needs them) and counts how often the
// `catch` around the curve optimisation fires (the spec's Technical notes:
// the port has no such fallback, so it must never fire on the corpus).
// Generated once from Vectorizer@32dd6c8 before that repository was
// deleted; the committed corpus.txt is the reference from then on.

import { execFileSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const src = process.argv[2];
if (!src) throw new Error("usage: generate.ts <vectorizer-checkout>");
const commit = execFileSync("git", ["-C", src, "rev-parse", "--short", "HEAD"]).toString().trim();

const tmp = mkdtempSync(join(tmpdir(), "vectorizer-"));
cpSync(join(src, "src/lib"), join(tmp, "lib"), { recursive: true });
cpSync(join(src, "src/worker"), join(tmp, "worker"), { recursive: true });
for (const dir of ["lib/potrace", "worker"]) {
  for (const f of readdirSync(join(tmp, dir))) {
    if (!f.endsWith(".ts")) continue;
    const p = join(tmp, dir, f);
    let s = readFileSync(p, "utf8").replace(/(from\s+"\.{1,2}\/[^"]+?)(?<!\.ts)"/g, '$1.ts"');
    s = s.replace(
      "} catch {",
      "} catch { (globalThis as Record<string, number>).__potraceCatch = ((globalThis as Record<string, number>).__potraceCatch ?? 0) + 1;",
    );
    writeFileSync(p, s);
  }
}
const worker = await import(pathToFileURL(join(tmp, "worker/trace.worker.ts")).href);
const onRequest = worker.onRequest as (msg: unknown) => {
  type: string;
  w: number;
  h: number;
  layers: { color: string; d: string }[];
  palette: { hex: string; area: number }[];
  stats: { paths: number; seg: number; curves: number; lines: number };
  usedThreshold: number | null;
  message?: string;
};

// ---------- test images (the spec's table) ----------

interface Img {
  w: number;
  h: number;
  d: Uint8ClampedArray;
}

function rng(seed: number): () => number {
  let s = seed;
  return () => {
    s = (s * 1103515245 + 12345) & 0x7fffffff;
    return s / 0x7fffffff;
  };
}

function img(w: number, h: number, f: (x: number, y: number) => number[]): Img {
  const d = new Uint8ClampedArray(w * h * 4);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) d.set(f(x, y), (y * w + x) * 4);
  return { w, h, d };
}

const bw = (on: boolean) => (on ? [0, 0, 0, 255] : [255, 255, 255, 255]);
const inDisc = (x: number, y: number, cx: number, cy: number, r: number) =>
  (x - cx) ** 2 + (y - cy) ** 2 <= r * r;

function noisy(): Img {
  const rnd = rng(42);
  return img(200, 150, (x, y) => {
    const v = (inDisc(x, y, 60, 75, 40) ? 40 : 220) + (rnd() * 30 - 15);
    return [v, v, v, 255];
  });
}

function speckled(): Img {
  const im = noisy();
  const rnd = rng(7);
  for (let i = 0; i < im.d.length; i += 4)
    if (rnd() < 0.04) {
      const v = rnd() < 0.5 ? 0 : 255;
      im.d[i] = im.d[i + 1] = im.d[i + 2] = v;
    }
  return im;
}

function lineArt(): Img {
  return img(1600, 1600, (x, y) => {
    const dc = Math.sqrt((x - 800) * (x - 800) + (y - 800) * (y - 800));
    for (let r = 20; r <= 800; r += 20) if (Math.abs(dc - r) <= 1.5) return bw(true);
    for (let i = 0; i < 40; i++) {
      const x1 = 0, y1 = 40 * i, x2 = 1599, y2 = 1599 - 40 * i;
      const num = Math.abs((x2 - x1) * (y1 - y) - (x1 - x) * (y2 - y1));
      if (num / Math.sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)) <= 1.5) return bw(true);
    }
    return bw(false);
  });
}

function textScan(): Img {
  const bg = rng(1);
  const im = img(1200, 1200, () => {
    const v = 235 + (bg() * 20 - 10);
    return [v, v, v, 255];
  });
  const rnd = rng(5);
  for (let row = 0; row < 30; row++)
    for (let col = 0; col < 50; col++) {
      const gw = 8 + Math.floor(rnd() * 9);
      const gh = 12 + Math.floor(rnd() * 13);
      for (let y = 0; y < gh; y++)
        for (let x = 0; x < gw; x++) im.d.set([30, 30, 30, 255], ((row * 40 + 6 + y) * 1200 + col * 24 + 4 + x) * 4);
    }
  return im;
}

function noise(): Img {
  const rnd = rng(3);
  return img(800, 800, () => bw(rnd() < 0.5));
}

const images: Record<string, () => Img> = {
  disc: () => img(100, 100, (x, y) => bw(inDisc(x, y, 50, 50, 30))),
  ring: () =>
    img(120, 120, (x, y) => {
      const d = (x - 60) ** 2 + (y - 60) ** 2;
      return bw(d >= 400 && d <= 2025);
    }),
  rectangle: () => img(80, 60, (x, y) => bw(x >= 10 && x < 70 && y >= 10 && y < 50)),
  square_with_stray_pixel: () =>
    img(60, 60, (x, y) => bw((x >= 10 && x < 40 && y >= 10 && y < 40) || (x === 50 && y === 50))),
  four_colour: () =>
    img(200, 150, (x, y) => {
      if (inDisc(x, y, 60, 75, 40)) return [46, 134, 171, 255];
      if (x > 120 && x < 175 && y > 20 && y < 70) return [192, 57, 43, 255];
      if (x > 120 && x < 175 && y > 90 && y < 130) return [39, 174, 96, 255];
      return [244, 239, 230, 255];
    }),
  noisy_two_tone: noisy,
  speckled_two_tone: speckled,
  transparent_1x1: () => img(1, 1, () => [0, 0, 0, 0]),
};

// ---------- option sets ----------

interface Opts {
  mode: "color" | "mono";
  colors?: number;
  pinned?: string[];
  threshold?: number | "auto";
  invert?: boolean;
  fill?: string;
  despeckle?: boolean;
  turn?: string;
  minArea?: number;
  alpha?: number;
  optcurve?: boolean;
  tol?: number;
}

const mono: Opts = { mode: "mono", threshold: "auto" };
const color: Opts = { mode: "color", colors: 6 };
const cases: [string, string, Opts][] = [
  ["disc", "disc", mono],
  ["ring", "ring", mono],
  ["rectangle", "rectangle", mono],
  ["square_min_area_2", "square_with_stray_pixel", mono],
  ["square_min_area_0", "square_with_stray_pixel", { ...mono, minArea: 0 }],
  ["noisy", "noisy_two_tone", mono],
  ["speckled", "speckled_two_tone", { ...mono, threshold: 130, minArea: 0 }],
  ["speckled_despeckle", "speckled_two_tone", { ...mono, threshold: 130, minArea: 0, despeckle: true }],
  ["four_colour", "four_colour", color],
  ["four_colour_2", "four_colour", { ...color, colors: 2 }],
  ["four_colour_3", "four_colour", { ...color, colors: 3 }],
  ["four_colour_16", "four_colour", { ...color, colors: 16 }],
  ["four_colour_despeckle", "four_colour", { ...color, despeckle: true }],
  ["four_colour_pinned_2", "four_colour", { ...color, pinned: ["#f4efe6", "#2e86ab"] }],
  ["four_colour_pinned_bw", "four_colour", { ...color, pinned: ["#000000", "#ffffff"] }],
  ["four_colour_mono_invert", "four_colour", { ...mono, invert: true }],
  ["noisy_colour", "noisy_two_tone", color],
  ["transparent_1x1", "transparent_1x1", color],
  ...["black", "white", "left", "right", "minority", "majority"].map(
    (turn) => [`ring_turn_${turn}`, "ring", { ...mono, turn }] as [string, string, Opts],
  ),
  ["ring_alpha_0", "ring", { ...mono, alpha: 0 }],
  ["ring_alpha_1.34", "ring", { ...mono, alpha: 1.34 }],
  ["ring_optcurve_off", "ring", { ...mono, optcurve: false }],
  ["disc_alpha_0", "disc", { ...mono, alpha: 0 }],
];

function message(o: Opts, job: number) {
  return {
    type: "trace",
    job,
    colorMode: o.mode === "color",
    paletteSize: o.colors ?? 6,
    params: {
      turnpolicy: o.turn ?? "minority",
      turdsize: o.minArea ?? 2,
      alphamax: o.alpha ?? 1.0,
      optcurve: o.optcurve ?? true,
      opttolerance: o.tol ?? 0.2,
    },
    fill: o.fill ?? "#000000",
    despeckle: o.despeckle ?? false,
    autoThreshold: o.threshold === "auto",
    threshold: typeof o.threshold === "number" ? o.threshold : 128,
    invert: o.invert ?? false,
    lockedPalette: o.pinned,
  };
}

function optString(o: Opts): string {
  const parts = [`mode=${o.mode}`];
  if (o.mode === "color") parts.push(`colors=${o.colors ?? 6}`);
  if (o.pinned) parts.push(`pinned=${o.pinned.join(",")}`);
  if (o.mode === "mono") {
    parts.push(`threshold=${o.threshold ?? "auto"}`, `invert=${o.invert ? 1 : 0}`, `fill=${o.fill ?? "#000000"}`);
  }
  parts.push(
    `despeckle=${o.despeckle ? 1 : 0}`,
    `turn=${o.turn ?? "minority"}`,
    `min_area=${o.minArea ?? 2}`,
    `alpha=${o.alpha ?? 1}`,
    `optcurve=${(o.optcurve ?? true) ? 1 : 0}`,
    `tol=${o.tol ?? 0.2}`,
  );
  return parts.join(";");
}

function load(im: Img): void {
  const r = onRequest({ type: "image", buf: im.d.buffer.slice(0), w: im.w, h: im.h });
  if (r.type !== "ready") throw new Error("image not accepted");
}

const out: string[] = [];
let job = 0;
for (const [name, image, o] of cases) {
  load(images[image]());
  const r = onRequest(message(o, ++job));
  if (r.type !== "result") throw new Error(`${name}: ${r.message}`);
  if (r.layers.some((l) => l.d === "")) throw new Error(`${name}: a layer without outlines (spec Assumptions)`);
  out.push(`case ${name}|${image}|${optString(o)}`);
  out.push(
    `result ${r.w} ${r.h} ${r.usedThreshold ?? "-"}|${r.stats.paths} ${r.stats.seg} ${r.stats.curves} ${r.stats.lines}`,
  );
  r.layers.forEach((l, i) => {
    const area = o.mode === "color" ? r.palette[i].area : "-";
    out.push(`layer ${l.color} ${area} ${l.d}`);
  });
  out.push("end");
}

// ---------- reference timings (AC-22) ----------

const bench: [string, () => Img, Opts][] = [
  ["line_art", lineArt, mono],
  ["text_scan", textScan, mono],
  ["noise", noise, { ...mono, threshold: 128, minArea: 0 }],
];
const timings: string[] = [];
for (const [name, make, o] of bench) {
  const im = make();
  const runs: number[] = [];
  for (let i = 0; i < 10; i++) {
    const t0 = performance.now();
    load(im);
    const r = onRequest(message(o, ++job));
    runs.push(performance.now() - t0);
    if (r.type !== "result") throw new Error(`${name}: ${r.message}`);
  }
  runs.sort((a, b) => a - b);
  timings.push(`timing ${name} ${((runs[4] + runs[5]) / 2).toFixed(1)}`);
}

const fired = (globalThis as Record<string, number>).__potraceCatch ?? 0;
console.log(`# pincel-trace golden corpus from daniel-rck/Vectorizer@${commit}, Node ${process.version}`);
console.log(`# reference catch around the curve optimisation fired: ${fired}`);
console.log(out.join("\n"));
console.log(timings.join("\n"));
