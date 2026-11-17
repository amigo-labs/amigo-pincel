import { crc32, deflateSync } from 'node:zlib';
import type { Page } from '@playwright/test';
import { rasterFile } from './helpers';

// Animated GIF, APNG and WebP files for the animated-image-import specs
// (docs/specs/animated-image-import.md), built byte by byte so each test
// states its canvas, frames and delays. The pixel-exact decoding is
// covered by pincel-core's tests; these files drive the open paths.

/** Global GIF palette: 0 red, 1 green, 2 blue, 3 white. */
const GIF_PALETTE = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];

/** One GIF frame: a rectangle filled with palette index `fill`. */
export interface GifFrame {
  left?: number;
  top?: number;
  width: number;
  height: number;
  fill: number;
  /** Hundredths of a second. */
  delay?: number;
  /** An invalid LZW minimum code size: the frame cannot be decoded. */
  broken?: boolean;
}

function u16(v: number): number[] {
  return [v & 0xff, (v >> 8) & 0xff];
}

/** LZW data with 8-bit codes (minimum code size 7): a clear code before
 *  every 64 pixels keeps the code width at 8, so codes are bytes. */
function lzw(pixels: number[]): number[] {
  const codes: number[] = [];
  for (let i = 0; i < pixels.length; i += 64) codes.push(128, ...pixels.slice(i, i + 64));
  codes.push(129);
  const out: number[] = [];
  for (let i = 0; i < codes.length; i += 255) {
    const block = codes.slice(i, i + 255);
    out.push(block.length, ...block);
  }
  out.push(0);
  return out;
}

/** A GIF89a with a `width × height` logical screen and `frames`. */
export function animatedGif(width: number, height: number, frames: GifFrame[]): Buffer {
  const out: number[] = [...Buffer.from('GIF89a', 'latin1'), ...u16(width), ...u16(height)];
  out.push(0x81, 0, 0, ...GIF_PALETTE);
  for (const f of frames) {
    out.push(0x21, 0xf9, 4, 1 << 2, ...u16(f.delay ?? 10), 0, 0);
    out.push(0x2c, ...u16(f.left ?? 0), ...u16(f.top ?? 0), ...u16(f.width), ...u16(f.height), 0);
    out.push(f.broken ? 12 : 7, ...lzw(new Array<number>(f.width * f.height).fill(f.fill)));
  }
  out.push(0x3b);
  return Buffer.from(out);
}

/** `count` full-screen frames cycling red, green, blue, 70 ms each. */
export function walkGif(count = 3, width = 6, height = 4): Buffer {
  return animatedGif(
    width,
    height,
    Array.from({ length: count }, (_, i) => ({ width, height, fill: i % 3, delay: 7 })),
  );
}

function pngChunk(type: string, data: Buffer): Buffer {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(data.length, 0);
  head.write(type, 4, 'latin1');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), data])) >>> 0, 0);
  return Buffer.concat([head, data, crc]);
}

/** An RGBA APNG of `width × height` with one full-canvas frame per colour
 *  in `colors`, each shown `delayMs`. */
export function apng(width: number, height: number, colors: number[][], delayMs = 100): Buffer {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  const actl = Buffer.alloc(8);
  actl.writeUInt32BE(colors.length, 0);
  const chunks = [pngChunk('IHDR', ihdr), pngChunk('acTL', actl)];
  let seq = 0;
  colors.forEach((color, i) => {
    const fctl = Buffer.alloc(26);
    fctl.writeUInt32BE(seq++, 0);
    fctl.writeUInt32BE(width, 4);
    fctl.writeUInt32BE(height, 8);
    fctl.writeUInt16BE(delayMs, 20);
    fctl.writeUInt16BE(1000, 22);
    chunks.push(pngChunk('fcTL', fctl));
    const raw: number[] = [];
    for (let y = 0; y < height; y++) {
      raw.push(0);
      for (let x = 0; x < width; x++) raw.push(...color);
    }
    const data = deflateSync(Buffer.from(raw));
    if (i === 0) {
      chunks.push(pngChunk('IDAT', data));
    } else {
      const seqBuf = Buffer.alloc(4);
      seqBuf.writeUInt32BE(seq++, 0);
      chunks.push(pngChunk('fdAT', Buffer.concat([seqBuf, data])));
    }
  });
  chunks.push(pngChunk('IEND', Buffer.alloc(0)));
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), ...chunks]);
}

function riffChunk(fourcc: string, data: Buffer): Buffer {
  const head = Buffer.alloc(8);
  head.write(fourcc, 0, 'latin1');
  head.writeUInt32LE(data.length, 4);
  return Buffer.concat([head, data, Buffer.alloc(data.length % 2)]);
}

function u24le(v: number): number[] {
  return [v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff];
}

/** An animated 8×6 WebP of `count` frames, each the browser's still WebP
 *  sample (`rasterFile`), `durationMs` each. */
export async function animatedWebp(page: Page, count: number, durationMs = 80): Promise<Buffer> {
  const still = await rasterFile(page, 'webp');
  // The image chunks of the still file: after its VP8X header if it has one.
  const parts: Buffer[] = [];
  let alpha = false;
  for (let at = 12; at + 8 <= still.length;) {
    const fourcc = still.toString('latin1', at, at + 4);
    const size = still.readUInt32LE(at + 4);
    const end = at + 8 + size + (size % 2);
    if (fourcc === 'ALPH' || fourcc === 'VP8 ' || fourcc === 'VP8L') {
      parts.push(still.subarray(at, end));
      if (fourcc !== 'VP8 ') alpha = true;
    }
    at = end;
  }
  const [w, h] = [8, 6];
  const body: Buffer[] = [Buffer.from('WEBP', 'latin1')];
  body.push(
    riffChunk(
      'VP8X',
      Buffer.from([0x02 | (alpha ? 0x10 : 0), 0, 0, 0, ...u24le(w - 1), ...u24le(h - 1)]),
    ),
  );
  body.push(riffChunk('ANIM', Buffer.from([0, 0, 0, 0, 0, 0])));
  for (let i = 0; i < count; i++) {
    const header = Buffer.from([
      ...u24le(0),
      ...u24le(0),
      ...u24le(w - 1),
      ...u24le(h - 1),
      ...u24le(durationMs),
      0,
    ]);
    body.push(riffChunk('ANMF', Buffer.concat([header, ...parts])));
  }
  const all = Buffer.concat(body);
  const head = Buffer.alloc(8);
  head.write('RIFF', 0, 'latin1');
  head.writeUInt32LE(all.length, 4);
  return Buffer.concat([head, all]);
}
