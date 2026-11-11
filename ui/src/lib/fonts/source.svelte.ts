// The fonts both Text tools offer (docs/specs/system-fonts.md): the bundled
// Liberation Sans, the system fonts the host lists (Local Font Access in the
// PWA, the desktop shell's `fontdb` list) and the font files the user added.
// Listing a font never reads it; a face's bytes are read on first use and
// kept for the session (AC-05).

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '../platform';
import { faceKey, storeFontFile, storedFaces, storedFontFile, type StoredFace } from '../idb/fonts';
import {
  BUNDLED_FACE,
  BUNDLED_FAMILY,
  bundledFamilyFaces,
  chooseFaces,
  faceFromStyle,
  familyNames,
  type FaceInfo,
  type FamilyFaces,
} from './faces';
import {
  addedFaces,
  cached,
  cssAliases,
  desktopFaces,
  faceReads,
  fileReads,
  localData,
  type FaceBytes,
  type LocalFontData,
} from './lookups';
import { faceIndexOf, readFaces, type SfntRejection } from './sfnt';

export type { FaceBytes } from './lookups';

/**
 * Whether the host's system fonts can be listed: `prompt` (PWA, not yet
 * decided), `granted`, `denied`, `dismissed` (the prompt was closed without
 * an answer), `unsupported` (no Local Font Access), `desktop` (the desktop
 * shell lists them), or `pending` until the check has run.
 */
export type FontAccess =
  'pending' | 'prompt' | 'granted' | 'denied' | 'dismissed' | 'unsupported' | 'desktop';

/** The font sources' state, shared by both modes. */
export const fontSource = $state({
  access: 'pending' as FontAccess,
  /** The system faces, once enumerated. */
  system: [] as FaceInfo[],
  /** Whether the system faces have been enumerated this session. */
  systemListed: false,
  /** An enumeration is running. */
  listing: false,
  /** The added faces. */
  added: [] as FaceInfo[],
  /** Whether the added faces have been loaded from IndexedDB. */
  addedLoaded: false,
});

// --- Local Font Access (PWA) -----------------------------------------------

type QueryLocalFonts = () => Promise<LocalFontData[]>;

function localFontsApi(): QueryLocalFonts | null {
  const q = (globalThis as { queryLocalFonts?: QueryLocalFonts }).queryLocalFonts;
  return typeof q === 'function' ? q : null;
}

async function localFontsPermission(): Promise<PermissionState | null> {
  try {
    const status = await navigator.permissions.query({
      name: 'local-fonts' as PermissionName,
    });
    return status.state;
  } catch {
    return null;
  }
}

// --- Desktop shell ---------------------------------------------------------

/** One face as the desktop shell lists it (`system_font_faces`). */
interface DesktopFace {
  readonly file: number;
  readonly index: number;
  readonly family: string;
  readonly style: string;
  readonly weight: number;
  readonly italic: boolean;
  readonly stretch: number;
  readonly postscript: string;
}

let initDone: Promise<void> | null = null;
let listing: Promise<void> | null = null;

/** Prepares the sources once per page load: checks the access, lists the
 *  system fonts where that needs no prompt (AC-14) and loads the names of
 *  the added fonts. */
export function initFontSource(): Promise<void> {
  initDone ??= (async () => {
    const added = loadAdded();
    if (isTauri()) {
      fontSource.access = 'desktop';
      void listSystemFonts();
    } else if (localFontsApi() === null) {
      fontSource.access = 'unsupported';
    } else {
      const state = await localFontsPermission();
      fontSource.access =
        state === 'granted' ? 'granted' : state === 'denied' ? 'denied' : 'prompt';
      if (state === 'granted') void listSystemFonts();
    }
    await added;
  })();
  return initDone;
}

/** Waits for the system list once an enumeration runs. */
export function systemFontsListed(): Promise<void> {
  return listing ?? Promise.resolve();
}

/**
 * Lists the system fonts if they are not listed yet, asking for the
 * permission when it is undecided. Call it synchronously from a click or a
 * press (AC-12, AC-22): the browser's prompt needs that user activation.
 */
export function requestSystemFonts(): Promise<void> {
  const access = fontSource.access;
  if (fontSource.systemListed && access !== 'dismissed') return Promise.resolve();
  if (
    access === 'prompt' ||
    access === 'dismissed' ||
    access === 'granted' ||
    access === 'desktop'
  ) {
    return listSystemFonts();
  }
  return Promise.resolve();
}

function listSystemFonts(): Promise<void> {
  if (listing) return listing;
  fontSource.listing = true;
  const run = fontSource.access === 'desktop' ? listDesktop() : listLocal();
  listing = run.finally(() => {
    fontSource.listing = false;
    listing = null;
  });
  return listing;
}

async function listLocal(): Promise<void> {
  const query = localFontsApi();
  if (!query) {
    fontSource.access = 'unsupported';
    return;
  }
  // Called before any await: the request keeps the click's activation.
  const request = query();
  let fonts: LocalFontData[];
  try {
    fonts = await request;
  } catch {
    const state = await localFontsPermission();
    fontSource.access = state === 'denied' ? 'denied' : 'dismissed';
    return;
  }
  const faces: FaceInfo[] = [];
  localData.clear();
  for (const f of fonts) {
    const key = `local:${f.postscriptName}`;
    if (localData.has(key)) continue;
    localData.set(key, f);
    faces.push(faceFromStyle('system', key, f.family, f.style, f.postscriptName));
  }
  fontSource.system = faces;
  fontSource.systemListed = true;
  fontSource.access = 'granted';
}

async function listDesktop(): Promise<void> {
  let list: DesktopFace[];
  try {
    list = await invoke<DesktopFace[]>('system_font_faces');
  } catch {
    list = [];
  }
  desktopFaces.clear();
  fontSource.system = list.map((f) => {
    const key = `desktop:${f.file}:${f.index}`;
    desktopFaces.set(key, { file: f.file, index: f.index });
    return {
      source: 'system',
      key,
      family: f.family,
      style: f.style,
      weight: f.weight,
      italic: f.italic,
      stretch: f.stretch,
      postscript: f.postscript,
    };
  });
  fontSource.systemListed = true;
}

// --- Added fonts -----------------------------------------------------------

function addedInfo(f: StoredFace): FaceInfo {
  return {
    source: 'added',
    key: `added:${f.key}`,
    family: f.family,
    style: f.style,
    weight: f.weight,
    italic: f.italic,
    stretch: f.stretch,
    postscript: f.postscript,
  };
}

function setAdded(rows: readonly StoredFace[]): void {
  addedFaces.clear();
  for (const r of rows) addedFaces.set(`added:${r.key}`, { fileId: r.fileId, index: r.index });
  fontSource.added = rows.map(addedInfo);
  fontSource.addedLoaded = true;
}

async function loadAdded(): Promise<void> {
  if (isTauri()) {
    fontSource.addedLoaded = true;
    return;
  }
  try {
    setAdded(await storedFaces());
  } catch {
    fontSource.addedLoaded = true;
  }
}

/** Why a picked file was not added (AC-21). */
export type AddRejection = SfntRejection | 'store';

/** The outcome of an "Add font file…" pick. */
export interface AddResult {
  /** Families added, in pick order, each once. */
  readonly families: readonly string[];
  readonly rejected: readonly { readonly name: string; readonly reason: AddRejection }[];
}

/** Adds the faces of each font file (AC-20, AC-21, AC-24). */
export async function addFontFiles(files: readonly File[]): Promise<AddResult> {
  const families: string[] = [];
  const rejected: { name: string; reason: AddRejection }[] = [];
  for (const file of files) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const parsed = readFaces(bytes);
    if (!parsed.ok) {
      rejected.push({ name: file.name, reason: parsed.reason });
      continue;
    }
    const fileId = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    const rows: StoredFace[] = parsed.faces.map((f) => ({
      key: faceKey(f.family, f.style),
      family: f.family,
      style: f.style,
      weight: f.weight,
      italic: f.italic,
      stretch: f.stretch,
      postscript: f.postscript,
      index: f.index,
      fileId,
    }));
    try {
      await storeFontFile(fileId, file.name, bytes, rows);
    } catch {
      rejected.push({ name: file.name, reason: 'store' });
      continue;
    }
    fileReads.set(`added:${fileId}`, Promise.resolve(bytes));
    for (const r of rows) {
      faceReads.delete(`added:${r.key}`);
      if (!families.includes(r.family)) families.push(r.family);
    }
    const kept = fontSource.added.filter((f) => !rows.some((r) => `added:${r.key}` === f.key));
    const merged = [...kept, ...rows.map(addedInfo)];
    for (const r of rows) addedFaces.set(`added:${r.key}`, { fileId, index: r.index });
    fontSource.added = merged;
  }
  return { families, rejected };
}

// --- The font list -----------------------------------------------------------

/** Every system and added face (the bundled one aside). */
function externalFaces(): FaceInfo[] {
  return [...fontSource.system, ...fontSource.added];
}

/** The font list: the bundled family plus every system and added family,
 *  each once, sorted case-insensitively (AC-01, AC-02). */
export function familyList(): string[] {
  return familyNames([BUNDLED_FACE, ...fontSource.system, ...fontSource.added]);
}

/** Whether `family` is listed by a source right now. */
export function familyListed(family: string): boolean {
  return (
    family === BUNDLED_FAMILY ||
    fontSource.system.some((f) => f.family === family) ||
    fontSource.added.some((f) => f.family === family)
  );
}

/** Where the faces of `family` come from: added wins over system. */
export function familySource(family: string): 'bundled' | 'system' | 'added' | null {
  if (family === BUNDLED_FAMILY) return 'bundled';
  if (fontSource.added.some((f) => f.family === family)) return 'added';
  if (fontSource.system.some((f) => f.family === family)) return 'system';
  return null;
}

/** The faces a Text tool uses for `family`; `null` when it is not listed. */
export function familyFaces(family: string): FamilyFaces | null {
  const faces = externalFaces().filter((f) => f.family === family);
  if (family === BUNDLED_FAMILY) return bundledFamilyFaces(faces);
  return chooseFaces(faces);
}

/** The faces of the bundled entry (AC-11 fallback, AC-23). */
export function bundledFaces(): FamilyFaces {
  return bundledFamilyFaces(externalFaces().filter((f) => f.family === BUNDLED_FAMILY));
}

// --- Reading faces ---------------------------------------------------------

async function bundledBytes(): Promise<Uint8Array> {
  const res = await fetch(`${import.meta.env.BASE_URL}fonts/LiberationSans-Regular.ttf`);
  if (!res.ok) throw new Error(`font fetch failed: ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
}

function readFile(key: string, read: () => Promise<Uint8Array>): Promise<Uint8Array> {
  return cached(fileReads, key, read);
}

async function readUncached(face: FaceInfo): Promise<FaceBytes> {
  if (face.source === 'bundled') {
    const bytes = await readFile('bundled', bundledBytes);
    return { bytes, index: 0 };
  }
  const local = localData.get(face.key);
  if (local) {
    const bytes = new Uint8Array(await (await local.blob()).arrayBuffer());
    const index = faceIndexOf(bytes, local.postscriptName);
    if (index === null) throw new Error('unreadable font');
    return { bytes, index };
  }
  const desk = desktopFaces.get(face.key);
  if (desk) {
    const bytes = await readFile(`desktop:${desk.file}`, async () => {
      const raw = await invoke<ArrayBuffer | number[]>('system_font_file', { file: desk.file });
      return raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
    });
    return { bytes, index: desk.index };
  }
  const added = addedFaces.get(face.key);
  if (added) {
    const bytes = await readFile(`added:${added.fileId}`, async () => {
      const b = await storedFontFile(added.fileId);
      if (!b) throw new Error('font file gone');
      return b;
    });
    return { bytes, index: added.index };
  }
  throw new Error('unknown font');
}

/**
 * The bytes of `face`, read once per session (AC-05); rejects when the face
 * cannot be read or is not a face the cores can rasterize (AC-11, AC-16).
 */
export function readFace(face: FaceInfo): Promise<FaceBytes> {
  return cached(faceReads, face.key, async () => {
    const r = await readUncached(face);
    const parsed = readFaces(r.bytes);
    if (!parsed.ok || !parsed.faces.some((f) => f.index === r.index)) {
      throw new Error('unparseable font');
    }
    return r;
  });
}

/** The bundled face's bytes. */
export function readBundled(): Promise<FaceBytes> {
  return readFace(BUNDLED_FACE);
}

// --- Rendering list entries --------------------------------------------------

let aliasCount = 0;

/**
 * The CSS `font-family` that renders `family`'s name in its own font
 * (AC-04): a system family by its name, which the browser resolves without
 * Pincel reading the face; the bundled and added families through a
 * `FontFace` built from their bytes.
 */
export function cssFamily(family: string): string {
  const source = familySource(family);
  const quoted = `"${family.replace(/["\\]/g, '\\$&')}"`;
  if (source === 'system' || source === null) return `${quoted}, sans-serif`;
  let alias = cssAliases.get(family);
  if (!alias) {
    aliasCount += 1;
    alias = `pincel-font-${aliasCount}`;
    cssAliases.set(family, alias);
    const faces = familyFaces(family);
    if (faces) {
      const name = alias;
      void readFace(faces.regular)
        .then(({ bytes }) => {
          const copy = new Uint8Array(bytes);
          const ff = new FontFace(name, copy.buffer);
          document.fonts.add(ff);
          return ff.load();
        })
        .catch(() => undefined);
    }
  }
  return `"${alias}", ${quoted}, sans-serif`;
}
