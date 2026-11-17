// A cheap look at a file's headers: does it announce more than one frame
// (docs/specs/animated-image-import.md AC-01, AC-02)? Only such files are
// handed to the Pixel core's `probeAnimation`, which decodes the first
// frame and has the final say, so a static GIF, PNG or WebP opens without
// loading the Pixel-mode wasm module. Rune-free, so a Node-side spec can
// test it.

/** True when the PNG has an `acTL` before its image data that announces
 *  more than one frame. */
function apngHint(b: Uint8Array): boolean {
  let pos = 8;
  while (pos + 8 <= b.length) {
    const size =
      ((b[pos] ?? 0) << 24) |
      ((b[pos + 1] ?? 0) << 16) |
      ((b[pos + 2] ?? 0) << 8) |
      (b[pos + 3] ?? 0);
    const type = String.fromCharCode(
      b[pos + 4] ?? 0,
      b[pos + 5] ?? 0,
      b[pos + 6] ?? 0,
      b[pos + 7] ?? 0,
    );
    if (type === 'IDAT' || type === 'IEND') return false;
    if (type === 'acTL') {
      const at = pos + 8;
      const frames =
        (((b[at] ?? 0) << 24) |
          ((b[at + 1] ?? 0) << 16) |
          ((b[at + 2] ?? 0) << 8) |
          (b[at + 3] ?? 0)) >>>
        0;
      return frames > 1;
    }
    pos += 12 + (size >>> 0);
  }
  return false;
}

/** True when the WebP has an animated `VP8X` header and more than one
 *  `ANMF` chunk. */
function webpHint(b: Uint8Array): boolean {
  const fourcc = (at: number) =>
    String.fromCharCode(b[at] ?? 0, b[at + 1] ?? 0, b[at + 2] ?? 0, b[at + 3] ?? 0);
  const u32 = (at: number) =>
    ((b[at] ?? 0) |
      ((b[at + 1] ?? 0) << 8) |
      ((b[at + 2] ?? 0) << 16) |
      ((b[at + 3] ?? 0) << 24)) >>>
    0;
  if (fourcc(12) !== 'VP8X' || ((b[20] ?? 0) & 0x02) === 0) return false;
  let frames = 0;
  let pos = 12;
  while (pos + 8 <= b.length) {
    if (fourcc(pos) === 'ANMF' && ++frames > 1) return true;
    const size = u32(pos + 4);
    pos += 8 + size + (size & 1);
  }
  return false;
}

/** Skips a GIF sub-block sequence starting at `pos`; returns the position
 *  after its terminator, or `-1` when the file ends first. */
function skipSubBlocks(b: Uint8Array, pos: number): number {
  let at = pos;
  while (at < b.length) {
    const len = b[at] ?? 0;
    if (len === 0) return at + 1;
    at += len + 1;
  }
  return -1;
}

/** True when the GIF holds at least two image descriptors. */
function gifHint(b: Uint8Array): boolean {
  if (b.length < 13) return false;
  const flags = b[10] ?? 0;
  let pos = 13 + ((flags & 0x80) !== 0 ? 3 * (1 << ((flags & 0x07) + 1)) : 0);
  let images = 0;
  while (pos < b.length) {
    const block = b[pos];
    if (block === 0x2c) {
      if (++images > 1) return true;
      const local = b[pos + 9] ?? 0;
      pos += 10 + ((local & 0x80) !== 0 ? 3 * (1 << ((local & 0x07) + 1)) : 0);
      // LZW minimum code size, then the image data sub-blocks.
      pos = skipSubBlocks(b, pos + 1);
    } else if (block === 0x21) {
      pos = skipSubBlocks(b, pos + 2);
    } else {
      return false;
    }
    if (pos < 0) return false;
  }
  return false;
}

/** Whether `bytes` of `format` announce more than one frame. Always
 *  `false` for other formats. */
export function mayBeAnimated(bytes: Uint8Array, format: string | null): boolean {
  if (format === 'png') return apngHint(bytes);
  if (format === 'webp') return webpHint(bytes);
  if (format === 'gif') return gifHint(bytes);
  return false;
}
