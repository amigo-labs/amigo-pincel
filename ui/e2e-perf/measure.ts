// Measurement helpers for the browser performance script (AC-03).

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { cpus, platform, release, totalmem } from 'node:os';
import { dirname } from 'node:path';
import { deflateSync } from 'node:zlib';
import type { Page } from '@playwright/test';

/** Nearest-rank percentile of `values` (`p` in 0..100); 0 for no values. */
export function percentile(values: readonly number[], p: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.min(sorted.length, Math.max(1, Math.ceil((p / 100) * sorted.length)));
  return sorted[rank - 1] ?? 0;
}

const round = (v: number): number => Math.round(v * 100) / 100;

/** Starts recording frames, draws and pointer inputs (clears earlier samples). */
export async function startRecording(page: Page): Promise<void> {
  await page.evaluate(() => {
    const perf = window.__perf;
    perf.frames = [];
    perf.draws = [];
    perf.inputs = [];
    perf.recording = true;
  });
}

/** What one recording window measured. */
export interface Recording {
  readonly frames: readonly number[];
  /** Pointer-move → presented latencies (painting). */
  readonly latencies: readonly number[];
  /** Pointer-down → first draw (a click's effect reaching the canvas). */
  readonly clicks: readonly number[];
  readonly draws: readonly number[];
  readonly marks: Readonly<Record<string, number>>;
}

/**
 * Lets pending work reach the screen (`settleMs`), stops recording and
 * returns frame times and pointer → presented latencies.
 */
export async function stopRecording(page: Page, settleMs = 500): Promise<Recording> {
  await page.waitForTimeout(settleMs);
  return page.evaluate(() => {
    const perf = window.__perf;
    perf.recording = false;
    const frames = perf.frames.slice(1).map((t, i) => t - (perf.frames[i] ?? t));
    const latencies = perf.inputs
      .filter((i) => i.type === 'pointermove' && i.presentedAt !== null)
      .map((i) => (i.presentedAt ?? 0) - i.t);
    const clicks = perf.inputs
      .filter((i) => i.type === 'pointerdown' && i.drawnAt !== null)
      .map((i) => (i.drawnAt ?? 0) - i.t);
    return { frames, latencies, clicks, draws: [...perf.draws], marks: { ...perf.marks } };
  });
}

/** p50 / p95 / max frame time of a recording, in ms. */
export function frameMetrics(rec: Recording): Record<string, number> {
  return {
    'frame-p50': round(percentile(rec.frames, 50)),
    'frame-p95': round(percentile(rec.frames, 95)),
    'frame-max': round(Math.max(0, ...rec.frames)),
  };
}

/** p50 / p95 dab latency (pointer event → pixels presented), in ms. */
export function latencyMetrics(rec: Recording): Record<string, number> {
  return {
    'dab-latency-p50': round(percentile(rec.latencies, 50)),
    'dab-latency-p95': round(percentile(rec.latencies, 95)),
  };
}

/** p50 / p95 of plain durations under `name`. */
export function durationMetrics(name: string, values: readonly number[]): Record<string, number> {
  return {
    [`${name}-p50`]: round(percentile(values, 50)),
    [`${name}-p95`]: round(percentile(values, 95)),
  };
}

/** Marks `selector`'s first match as the render target whose draws count. */
export async function targetCanvas(page: Page, selector: string): Promise<void> {
  await page.evaluate((sel) => {
    window.__perf.target = document.querySelector<HTMLCanvasElement>(sel);
  }, selector);
}

/** Largest `WebAssembly.Memory` the app instantiated, in MiB. */
export async function wasmMemoryMb(page: Page): Promise<number> {
  const bytes = await page.evaluate(() =>
    Math.max(0, ...window.__perf.memories.map((m) => m.buffer.byteLength)),
  );
  return round(bytes / (1024 * 1024));
}

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (const b of bytes) c = (CRC_TABLE[(c ^ b) & 0xff] ?? 0) ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Uint8Array): Buffer {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'ascii');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])), 0);
  return Buffer.concat([head, data, crc]);
}

/** An RGBA8 PNG of `w × h` with a gradient (not trivially compressible). */
export function gradientPng(w: number, h: number): Buffer {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    const row = y * (w * 4 + 1);
    for (let x = 0; x < w; x++) {
      const o = row + 1 + x * 4;
      raw[o] = (x * 255) / w;
      raw[o + 1] = (y * 255) / h;
      raw[o + 2] = (x ^ y) & 0xff;
      raw[o + 3] = 255;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 6 })),
    chunk('IEND', new Uint8Array()),
  ]);
}

/** Collected scenario results, written once at the end of the run. */
export const results: Record<string, { label: string; metrics: Record<string, number> }> = {};

/** Records `metrics` for scenario `id` (merged with earlier ones). */
export function record(id: string, label: string, metrics: Record<string, number>): void {
  const entry = (results[id] ??= { label, metrics: {} });
  Object.assign(entry.metrics, metrics);
}

/**
 * Writes `results` to `path` with the date and a machine description,
 * merged with the scenarios already there: a failed test restarts the
 * worker, and each worker writes what it measured (`global-setup.ts`
 * clears the file at the start of a run).
 */
export function writeResults(path: string, browserVersion: string): void {
  const cpu = cpus()[0]?.model.trim() ?? 'unknown CPU';
  const machine = `${cpu}, ${cpus().length} threads, ${Math.round(totalmem() / 2 ** 30)} GB, ${platform()} ${release()}, Chromium ${browserVersion}`;
  const earlier = existsSync(path)
    ? (JSON.parse(readFileSync(path, 'utf8')) as { scenarios: typeof results }).scenarios
    : {};
  mkdirSync(dirname(path), { recursive: true });
  const scenarios = { ...earlier, ...results };
  const date = new Date().toISOString().slice(0, 10);
  writeFileSync(
    path,
    `${JSON.stringify({ date, machine, scenarios }, null, 2)}
`,
  );
}
