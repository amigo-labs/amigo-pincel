import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';
import {
  BUNDLED_FACE,
  bundledFamilyFaces,
  chooseFaces,
  faceFor,
  faceFromStyle,
  familyNames,
  filterFamilies,
  replaceFaces,
  styleWeight,
  type FaceInfo,
} from '../src/lib/fonts/faces';
import { faceIndexOf, readFaces } from '../src/lib/fonts/sfnt';

// docs/specs/system-fonts.md: the pure rules behind the font list — face
// choice per family ("Terms", AC-07, AC-23), the family list (AC-02,
// AC-03), replacement of added faces (AC-24) and the font-file reader
// (AC-08, AC-20, AC-21). Node-side: no page.

const FIXTURES = fileURLToPath(
  new URL('../../crates/pincel-core/tests/fixtures/', import.meta.url),
);
const fixture = async (name: string): Promise<Uint8Array> =>
  new Uint8Array(await readFile(join(FIXTURES, name)));

const face = (family: string, style: string, stretch?: number): FaceInfo => {
  const f = faceFromStyle('system', `${family}/${style}`, family, style, `${family}-${style}`);
  return stretch === undefined ? f : { ...f, stretch };
};

const styles = (faces: FaceInfo[]) => {
  const c = chooseFaces(faces);
  if (!c) throw new Error('no faces');
  return {
    regular: c.regular.style,
    bold: c.bold?.style ?? null,
    italic: c.italic?.style ?? null,
    boldItalic: c.boldItalic?.style ?? null,
  };
};

test('style names map to weights, slant and width', () => {
  expect(
    ['Thin', 'ExtraLight', 'Ultra Light', 'Light', 'Book', 'Roman', 'Medium'].map(styleWeight),
  ).toEqual([100, 200, 200, 300, 400, 400, 500]);
  expect(
    ['SemiBold', 'Demi Bold', 'Bold', 'ExtraBold', 'Black', 'Heavy', 'Fancy'].map(styleWeight),
  ).toEqual([600, 600, 700, 800, 900, 900, 400]);
  expect(face('A', 'Bold Oblique').italic).toBe(true);
  expect(face('A', 'Condensed').stretch).toBe(3);
  expect(face('A', 'Regular').stretch).toBe(5);
});

test('face choice: Regular nearest 400, Bold nearest 700 at 600 or more', () => {
  const family = ['Light', 'Regular', 'SemiBold', 'Bold'].map((s) => face('F', s));
  expect(styles(family)).toEqual({
    regular: 'Regular',
    bold: 'Bold',
    italic: null,
    boldItalic: null,
  });
  expect(styles(family.filter((f) => f.style !== 'Bold')).bold).toBe('SemiBold');
  // A family whose only face is 900 ("Arial Black"): that face is Regular,
  // Bold stays synthesized.
  const black = chooseFaces([face('Arial Black', 'Black')]);
  expect(black?.regular.style).toBe('Black');
  expect(black?.bold).toBeNull();
  // Ties prefer normal width, then name order.
  expect(styles([face('W', 'Regular Condensed', 3), face('W', 'Book')]).regular).toBe('Book');
  expect(styles([face('W', 'Roman'), face('W', 'Book')]).regular).toBe('Book');
  // Italic and Bold Italic among the slanted faces.
  const full = ['Regular', 'Bold', 'Italic', 'Bold Italic', 'Light Italic'].map((s) =>
    face('G', s),
  );
  expect(styles(full)).toEqual({
    regular: 'Regular',
    bold: 'Bold',
    italic: 'Italic',
    boldItalic: 'Bold Italic',
  });
  // Without upright faces the slanted ones serve as upright.
  expect(styles([face('S', 'Italic'), face('S', 'Bold Italic')])).toEqual({
    regular: 'Italic',
    bold: 'Bold Italic',
    italic: 'Italic',
    boldItalic: 'Bold Italic',
  });
});

test('AC-07: Bold / Italic use real faces, the missing style is synthesized', () => {
  const pick = (have: string[], bold: boolean, italic: boolean): string => {
    const c = chooseFaces(have.map((s) => face('H', s)));
    if (!c) throw new Error('no faces');
    const u = faceFor(c, bold, italic);
    return `${u.face.style}${u.synthBold ? '+b' : ''}${u.synthItalic ? '+i' : ''}`;
  };
  const all = ['Regular', 'Bold', 'Italic', 'Bold Italic'];
  expect(pick(all, false, false)).toBe('Regular');
  expect(pick(all, true, false)).toBe('Bold');
  expect(pick(all, false, true)).toBe('Italic');
  expect(pick(all, true, true)).toBe('Bold Italic');
  expect(pick(['Regular', 'Bold', 'Italic'], true, true)).toBe('Bold+i');
  expect(pick(['Regular', 'Italic'], true, true)).toBe('Italic+b');
  expect(pick(['Regular'], true, true)).toBe('Regular+b+i');
  expect(pick(['Regular', 'Italic'], true, false)).toBe('Regular+b');
  expect(pick(['Regular', 'Bold'], false, true)).toBe('Regular+i');
});

test('AC-23: the Liberation Sans entry keeps the bundled face for upright regular text', () => {
  const others = ['Regular', 'Bold', 'Italic', 'Bold Italic'].map((s) =>
    face('Liberation Sans', s),
  );
  const c = bundledFamilyFaces(others);
  expect(faceFor(c, false, false).face).toBe(BUNDLED_FACE);
  expect(faceFor(c, true, false).face.style).toBe('Bold');
  expect(faceFor(c, false, true).face.style).toBe('Italic');
  expect(faceFor(c, true, true).face.style).toBe('Bold Italic');
  const alone = bundledFamilyFaces([]);
  expect(faceFor(alone, true, true)).toEqual({
    face: BUNDLED_FACE,
    synthBold: true,
    synthItalic: true,
  });
});

test('AC-02 / AC-03: each family once, sorted case-insensitively, filtered by substring', () => {
  const faces = [
    face('beta', 'Regular'),
    face('Alpha', 'Regular'),
    face('Alpha', 'Bold'),
    face('alpha', 'Regular'),
    face('Gamma Sans', 'Italic'),
  ];
  const names = familyNames(faces);
  expect(names).toEqual(['Alpha', 'alpha', 'beta', 'Gamma Sans']);
  expect(filterFamilies(names, 'ALP')).toEqual(['Alpha', 'alpha']);
  expect(filterFamilies(names, 'a s')).toEqual(['Gamma Sans']);
  expect(filterFamilies(names, '')).toEqual(names);
  expect(filterFamilies(names, 'zzz')).toEqual([]);
});

test('AC-24: an added face with the same family and style replaces the earlier one', () => {
  const old = [
    { family: 'A', style: 'Regular', v: 1 },
    { family: 'A', style: 'Bold', v: 1 },
  ];
  const merged = replaceFaces(old, [{ family: 'A', style: 'Regular', v: 2 }]);
  expect(merged).toHaveLength(2);
  expect(merged.find((f) => f.style === 'Regular')?.v).toBe(2);
  expect(merged.find((f) => f.style === 'Bold')?.v).toBe(1);
});

test('AC-20: the reader lists each face with names, weight and slant', async () => {
  const bold = readFaces(await fixture('PincelTestSerif-BoldItalic.ttf'));
  expect(bold).toEqual({
    ok: true,
    faces: [
      {
        index: 0,
        family: 'Pincel Test Serif',
        style: 'Bold Italic',
        weight: 700,
        italic: true,
        stretch: 5,
        postscript: 'PincelTestSerif-BoldItalic',
      },
    ],
  });
  const ttc = readFaces(await fixture('PincelTestSerif.ttc'));
  expect(ttc.ok && ttc.faces.map((f) => [f.index, f.style, f.weight])).toEqual([
    [0, 'Regular', 400],
    [1, 'Bold', 700],
  ]);
  const sans = readFaces(await fixture('LiberationSans-Regular.ttf'));
  expect(sans.ok && sans.faces[0]?.family).toBe('Liberation Sans');
});

test('AC-08: a face is found by its PostScript name inside a collection', async () => {
  const ttc = await fixture('PincelTestSerif.ttc');
  expect(faceIndexOf(ttc, 'PincelTestSerif-Bold')).toBe(1);
  expect(faceIndexOf(ttc, 'PincelTestSerif-Regular')).toBe(0);
  expect(faceIndexOf(await fixture('PincelTestSerif-Italic.ttf'), 'Whatever')).toBe(0);
  expect(faceIndexOf(new Uint8Array([1, 2, 3]), 'X')).toBeNull();
});

test('AC-21: web fonts, other files and damaged fonts are refused with a reason', async () => {
  const bytes = (...b: number[]) => new Uint8Array([...b, ...new Array<number>(64).fill(0)]);
  expect(readFaces(bytes(0x77, 0x4f, 0x46, 0x32))).toEqual({ ok: false, reason: 'webfont' });
  expect(readFaces(bytes(0x77, 0x4f, 0x46, 0x46))).toEqual({ ok: false, reason: 'webfont' });
  expect(readFaces(bytes(0x89, 0x50, 0x4e, 0x47))).toEqual({ ok: false, reason: 'notFont' });
  expect(readFaces(new Uint8Array([0x80, 0x01]))).toEqual({ ok: false, reason: 'notFont' });
  const ttf = await fixture('PincelTestSerif-Regular.ttf');
  expect(readFaces(ttf.slice(0, 200))).toEqual({ ok: false, reason: 'damaged' });
});
