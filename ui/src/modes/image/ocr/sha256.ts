// SHA-256 for checking a downloaded OCR model against its pinned hash
// (docs/specs/ocr-dialog.md AC-05, AC-20). `crypto.subtle` where the page
// is a secure context; a plain implementation (FIPS 180-4) where a webview
// lacks it, so the check never depends on the host. Rune-free.

const K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

/** Lower-case hex of `bytes`. */
function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** Rotates `x` right by `n` bits. */
const rotr = (x: number, n: number): number => (x >>> n) | (x << (32 - n));

/** SHA-256 of `data` in plain TypeScript, as lower-case hex. */
export function sha256Plain(data: Uint8Array): string {
  const h = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
  ];
  // The message is followed by 0x80, zero padding and its 64-bit
  // big-endian bit length; the blocks after the last full one are copied.
  const full = Math.floor(data.length / 64) * 64;
  const blocks = Math.ceil((data.length + 9) / 64);
  const tail = new Uint8Array(blocks * 64 - full);
  tail.set(data.subarray(full));
  tail[data.length - full] = 0x80;
  const bits = data.length * 8;
  const tailView = new DataView(tail.buffer);
  tailView.setUint32(tail.length - 8, Math.floor(bits / 0x100000000));
  tailView.setUint32(tail.length - 4, bits >>> 0);

  const w = new Array<number>(64).fill(0);
  const word = (i: number): number => w[i] ?? 0;
  const block = (view: DataView, at: number): void => {
    for (let i = 0; i < 16; i += 1) w[i] = view.getUint32(at + i * 4);
    for (let i = 16; i < 64; i += 1) {
      const a = word(i - 15);
      const b = word(i - 2);
      const s0 = rotr(a, 7) ^ rotr(a, 18) ^ (a >>> 3);
      const s1 = rotr(b, 17) ^ rotr(b, 19) ^ (b >>> 10);
      w[i] = (word(i - 16) + s0 + word(i - 7) + s1) >>> 0;
    }
    let [a, b, c, d, e, f, g, hh] = h as [
      number,
      number,
      number,
      number,
      number,
      number,
      number,
      number,
    ];
    for (let i = 0; i < 64; i += 1) {
      const t1 =
        (hh +
          (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) +
          ((e & f) ^ (~e & g)) +
          (K[i] ?? 0) +
          word(i)) >>>
        0;
      const t2 = ((rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) + ((a & b) ^ (a & c) ^ (b & c))) >>> 0;
      hh = g;
      g = f;
      f = e;
      e = (d + t1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (t1 + t2) >>> 0;
    }
    [a, b, c, d, e, f, g, hh].forEach((v, i) => {
      h[i] = ((h[i] ?? 0) + v) >>> 0;
    });
  };
  const dataView = new DataView(data.buffer, data.byteOffset, data.byteLength);
  for (let at = 0; at < full; at += 64) block(dataView, at);
  for (let at = 0; at < tail.length; at += 64) block(tailView, at);

  const out = new Uint8Array(32);
  const outView = new DataView(out.buffer);
  h.forEach((v, i) => outView.setUint32(i * 4, v));
  return hex(out);
}

/** SHA-256 of `data` as lower-case hex. */
export async function sha256Hex(data: Uint8Array<ArrayBuffer>): Promise<string> {
  const subtle = globalThis.crypto?.subtle;
  if (subtle) {
    try {
      return hex(new Uint8Array(await subtle.digest('SHA-256', data)));
    } catch {
      // Fall through to the plain implementation.
    }
  }
  return sha256Plain(data);
}
