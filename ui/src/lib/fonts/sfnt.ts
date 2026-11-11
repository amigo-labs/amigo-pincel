// A small reader for TrueType / OpenType files and collections
// (docs/specs/system-fonts.md): the faces a file holds with their names,
// weight, slant and width, read from the `name`, `OS/2` and `head` tables.
// It also refuses what the cores cannot rasterize (AC-21): web fonts,
// non-sfnt files, files missing the tables `ab_glyph` needs, and faces
// without outlines. Pure and rune-free (Node-side specs import it).

import { styleItalic, styleStretch, styleWeight } from './faces';

/** One face of a font file. */
export interface SfntFace {
  /** Index of the face inside a collection; 0 for a single-face file. */
  readonly index: number;
  /** Typographic family (name ID 16), else the legacy family (ID 1). */
  readonly family: string;
  /** Typographic subfamily (ID 17), else the legacy subfamily (ID 2). */
  readonly style: string;
  readonly weight: number;
  readonly italic: boolean;
  readonly stretch: number;
  /** PostScript name (ID 6). */
  readonly postscript: string;
}

/** Why a file is not a font the cores can rasterize. */
export type SfntRejection = 'webfont' | 'notFont' | 'damaged' | 'noOutlines';

/** The faces of a file, or why it was refused. */
export type SfntResult =
  | { readonly ok: true; readonly faces: readonly SfntFace[] }
  | { readonly ok: false; readonly reason: SfntRejection };

const TAG_TTCF = 0x74746366; // 'ttcf'
const TAG_WOFF = 0x774f4646; // 'wOFF'
const TAG_WOFF2 = 0x774f4632; // 'wOF2'
const SFNT_VERSIONS = new Set([0x00010000, 0x4f54544f /* OTTO */, 0x74727565 /* true */]);

/** Thrown inside the reader for a read past the end; mapped to 'damaged'. */
class Truncated extends Error {}

function view(bytes: Uint8Array): DataView {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

function u16(v: DataView, at: number): number {
  if (at < 0 || at + 2 > v.byteLength) throw new Truncated();
  return v.getUint16(at);
}

function u32(v: DataView, at: number): number {
  if (at < 0 || at + 4 > v.byteLength) throw new Truncated();
  return v.getUint32(at);
}

interface Table {
  readonly offset: number;
  readonly length: number;
}

/** The table directory at `at`. */
function tables(v: DataView, at: number): Map<string, Table> {
  const version = u32(v, at);
  if (!SFNT_VERSIONS.has(version)) throw new Truncated();
  const count = u16(v, at + 4);
  const out = new Map<string, Table>();
  for (let i = 0; i < count; i += 1) {
    const rec = at + 12 + i * 16;
    const tag = String.fromCharCode(
      v.getUint8(rec),
      v.getUint8(rec + 1),
      v.getUint8(rec + 2),
      v.getUint8(rec + 3),
    );
    const offset = u32(v, rec + 8);
    const length = u32(v, rec + 12);
    if (offset + length > v.byteLength) throw new Truncated();
    out.set(tag, { offset, length });
  }
  return out;
}

function utf16be(v: DataView, at: number, length: number): string {
  let s = '';
  for (let i = 0; i + 1 < length; i += 2) s += String.fromCharCode(u16(v, at + i));
  return s;
}

function latin1(v: DataView, at: number, length: number): string {
  let s = '';
  for (let i = 0; i < length; i += 1) {
    if (at + i >= v.byteLength) throw new Truncated();
    s += String.fromCharCode(v.getUint8(at + i));
  }
  return s;
}

/** Name records by ID, preferring Windows English, then any Windows or
 *  Unicode record, then Macintosh Roman. */
function names(v: DataView, table: Table | undefined): Map<number, string> {
  const out = new Map<number, string>();
  if (!table) return out;
  const base = table.offset;
  const count = u16(v, base + 2);
  const strings = base + u16(v, base + 4);
  const rank = new Map<number, number>();
  for (let i = 0; i < count; i += 1) {
    const rec = base + 6 + i * 12;
    const platform = u16(v, rec);
    const encoding = u16(v, rec + 2);
    const language = u16(v, rec + 4);
    const id = u16(v, rec + 6);
    const length = u16(v, rec + 8);
    const offset = strings + u16(v, rec + 10);
    let r: number;
    let text: string;
    if (platform === 3 && (encoding === 1 || encoding === 10)) {
      r = language === 0x409 ? 0 : 1;
      text = utf16be(v, offset, length);
    } else if (platform === 0) {
      r = 2;
      text = utf16be(v, offset, length);
    } else if (platform === 1 && encoding === 0) {
      r = 3;
      text = latin1(v, offset, length);
    } else {
      continue;
    }
    if (text.trim() === '') continue;
    if (!rank.has(id) || r < (rank.get(id) ?? 99)) {
      rank.set(id, r);
      out.set(id, text);
    }
  }
  return out;
}

/** The face whose table directory starts at `at`. */
function face(v: DataView, at: number, index: number): SfntFace | 'damaged' | 'noOutlines' {
  const t = tables(v, at);
  // ttf-parser (behind ab_glyph) needs these to parse a face at all.
  if (!t.has('head') || !t.has('hhea') || !t.has('maxp')) return 'damaged';
  const outlines = (t.has('glyf') && t.has('loca')) || t.has('CFF ') || t.has('CFF2');
  if (!outlines) return 'noOutlines';
  const n = names(v, t.get('name'));
  const family = n.get(16) ?? n.get(1);
  if (family === undefined) return 'damaged';
  const style = n.get(17) ?? n.get(2) ?? 'Regular';
  const os2 = t.get('OS/2');
  const head = t.get('head');
  let weight = styleWeight(style);
  let italic = styleItalic(style);
  let stretch = styleStretch(style);
  if (os2 && os2.length >= 64) {
    const w = u16(v, os2.offset + 4);
    if (w >= 1 && w <= 1000) weight = w;
    const width = u16(v, os2.offset + 6);
    if (width >= 1 && width <= 9) stretch = width;
    const fsSelection = u16(v, os2.offset + 62);
    italic = (fsSelection & 0x0001) !== 0 || (fsSelection & 0x0200) !== 0;
  } else if (head) {
    italic = (u16(v, head.offset + 44) & 0x0002) !== 0;
  }
  return {
    index,
    family,
    style,
    weight,
    italic,
    stretch,
    postscript: n.get(6) ?? `${family}-${style}`.replace(/\s+/g, ''),
  };
}

/** The faces of a TrueType / OpenType file or collection (AC-20, AC-21). */
export function readFaces(bytes: Uint8Array): SfntResult {
  const v = view(bytes);
  try {
    if (bytes.byteLength < 12) return { ok: false, reason: 'notFont' };
    const tag = u32(v, 0);
    if (tag === TAG_WOFF || tag === TAG_WOFF2) return { ok: false, reason: 'webfont' };
    let offsets: number[];
    if (tag === TAG_TTCF) {
      const count = u32(v, 8);
      if (count === 0 || count > 10_000) return { ok: false, reason: 'damaged' };
      offsets = Array.from({ length: count }, (_, i) => u32(v, 12 + i * 4));
    } else if (SFNT_VERSIONS.has(tag)) {
      offsets = [0];
    } else {
      return { ok: false, reason: 'notFont' };
    }
    const faces: SfntFace[] = [];
    let reason: SfntRejection | null = null;
    offsets.forEach((at, index) => {
      const f = face(v, at, index);
      if (typeof f === 'string') reason ??= f;
      else faces.push(f);
    });
    if (faces.length === 0) return { ok: false, reason: reason ?? 'damaged' };
    return { ok: true, faces };
  } catch {
    return { ok: false, reason: 'damaged' };
  }
}

/**
 * The index of the face named `postscript` in `bytes` (AC-08): Local Font
 * Access hands out bytes without a face index, and they may hold a whole
 * collection. 0 when the bytes hold one face or none matches; `null` when
 * the bytes are not a font the cores can rasterize.
 */
export function faceIndexOf(bytes: Uint8Array, postscript: string): number | null {
  const r = readFaces(bytes);
  if (!r.ok) return null;
  return r.faces.find((f) => f.postscript === postscript)?.index ?? r.faces[0]?.index ?? 0;
}
