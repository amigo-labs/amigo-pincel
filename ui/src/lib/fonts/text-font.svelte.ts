// One mode's Text-tool font (docs/specs/system-fonts.md): the chosen family,
// remembered per mode across sessions (AC-10), the face it resolves to for
// Bold / Italic (AC-07) registered with the mode's core, and the fallback to
// Liberation Sans when the family is unavailable (AC-11).

import { SvelteMap, SvelteSet } from 'svelte/reactivity';
import { t } from '../i18n/index.svelte';
import { getPref, setPref } from '../idb/prefs';
import { BUNDLED_FAMILY, faceFor, type FaceInfo, type FamilyFaces } from './faces';
import {
  type AddResult,
  bundledFaces,
  familyFaces,
  familyListed,
  familySource,
  fontSource,
  initFontSource,
  readFace,
  requestSystemFonts,
  systemFontsListed,
} from './source.svelte';

/** The face a text is drawn with: its id in the mode's core and what is left
 *  to synthesize. */
export interface UsedFace {
  readonly id: number;
  readonly synthBold: boolean;
  readonly synthItalic: boolean;
  /** The family the face belongs to (Liberation Sans after a fallback). */
  readonly family: string;
}

/** Registers face `index` of `bytes` with the mode's core; returns its id. */
export type RegisterFont = (bytes: Uint8Array, index: number) => number | Promise<number>;

/** Where a family's faces come from. */
type FamilyKind = 'bundled' | 'system' | 'added';

/** Whether a family can be used now: `ok`, `missing` (AC-11), `undecided`
 *  (a system family while the permission is undecided, AC-22) or `unknown`
 *  (its source is still being listed). */
export type Availability = 'ok' | 'missing' | 'undecided' | 'unknown';

/** The remembered family of a mode. */
interface FontPref {
  readonly family: string;
  readonly source: FamilyKind;
}

function isFontPref(v: unknown): v is FontPref {
  if (typeof v !== 'object' || v === null) return false;
  const p = v as Record<string, unknown>;
  return (
    typeof p.family === 'string' &&
    (p.source === 'bundled' || p.source === 'system' || p.source === 'added')
  );
}

/** A mode's Text-tool font. */
export interface TextFont {
  /** The chosen family and where it came from. */
  readonly state: { family: string; source: FamilyKind };
  /** Reads the remembered family (AC-10). */
  load(): Promise<void>;
  /** Chooses `family` and remembers it. */
  pick(family: string): void;
  /** Whether the chosen family can be used now. */
  availability(): Availability;
  /** The Text tool needs the family (Image: tool selected; Pixel: dialog
   *  opened): says once per session when it falls back (AC-11). */
  needed(): Promise<void>;
  /**
   * The face for Bold / Italic of the current family, read and registered;
   * Liberation Sans when the family is unavailable. Call it synchronously
   * from a press when `gesture` is set: an undecided permission is asked
   * for from that press (AC-22). Rejects only when Liberation Sans itself
   * cannot be loaded.
   */
  resolve(bold: boolean, italic: boolean, gesture?: boolean): Promise<UsedFace>;
  /** After "Add font file…": the first added family becomes the current one
   *  and one status message reports the pick (AC-20, AC-21). */
  added(result: AddResult): void;
}

/** The status message for an "Add font file…" pick, or `null`. */
export function addResultMessage(result: AddResult): string | null {
  const parts: string[] = [];
  if (result.families.length > 0) {
    parts.push(t('font.added', { families: result.families.join(', ') }));
  }
  if (result.rejected.length > 0) {
    const files = result.rejected
      .map((r) =>
        t('font.rejectedFile', { name: r.name, reason: t(`font.reason.${r.reason}` as const) }),
      )
      .join('; ');
    parts.push(t('font.rejected', { files }));
  }
  return parts.length > 0 ? parts.join(' — ') : null;
}

/**
 * The Text-tool font of `mode`. `register` hands a face to the mode's core;
 * `notify` shows a status-bar message.
 */
export function createTextFont(
  mode: 'pixel' | 'image',
  register: RegisterFont,
  notify: (message: string) => void,
): TextFont {
  const state = $state({ family: BUNDLED_FAMILY, source: 'bundled' as FamilyKind });
  const prefKey = `textFont.${mode}`;
  /** Registered ids by face key and index. */
  const ids = new SvelteMap<string, Promise<number>>();
  /** Families whose face could not be read or parsed this session. */
  const failed = new SvelteSet<string>();
  /** Families whose fallback was announced this session. */
  const announced = new SvelteSet<string>();

  function availabilityOf(family: string, source: FamilyKind): Availability {
    if (family === BUNDLED_FAMILY) return 'ok';
    if (failed.has(family)) return 'missing';
    if (familyListed(family)) return 'ok';
    if (source === 'added') return fontSource.addedLoaded ? 'missing' : 'unknown';
    if (source === 'system') {
      switch (fontSource.access) {
        case 'pending':
          return 'unknown';
        case 'prompt':
          return 'undecided';
        case 'granted':
        case 'desktop':
          return fontSource.systemListed ? 'missing' : 'unknown';
        default:
          return 'missing';
      }
    }
    return 'missing';
  }

  function announce(family: string): void {
    if (announced.has(family)) return;
    announced.add(family);
    notify(t('font.fallback', { family }));
  }

  async function idOf(face: FaceInfo): Promise<number> {
    const { bytes, index } = await readFace(face);
    const key = `${face.key}#${index}`;
    let id = ids.get(key);
    if (!id) {
      id = Promise.resolve(register(bytes, index));
      ids.set(key, id);
    }
    return id;
  }

  async function use(faces: FamilyFaces, bold: boolean, italic: boolean, family: string) {
    const u = faceFor(faces, bold, italic);
    const id = await idOf(u.face);
    return { id, synthBold: u.synthBold, synthItalic: u.synthItalic, family };
  }

  async function settled(): Promise<void> {
    await initFontSource();
    await systemFontsListed();
  }

  const font: TextFont = {
    state,
    async load() {
      try {
        const v = await getPref(prefKey);
        if (isFontPref(v)) {
          state.family = v.family;
          state.source = v.source;
        }
      } catch {
        // No stored choice: Liberation Sans.
      }
    },
    pick(family) {
      state.family = family;
      state.source = familySource(family) ?? state.source;
      void setPref(prefKey, { family: state.family, source: state.source }).catch(() => undefined);
    },
    availability() {
      return availabilityOf(state.family, state.source);
    },
    async needed() {
      await settled();
      if (availabilityOf(state.family, state.source) === 'missing') announce(state.family);
    },
    async resolve(bold, italic, gesture = false) {
      const family = state.family;
      const source = state.source;
      // Before any await: the prompt needs the press's user activation.
      const asked =
        gesture && availabilityOf(family, source) === 'undecided' ? requestSystemFonts() : null;
      await settled();
      if (asked) await asked;
      const faces = availabilityOf(family, source) === 'ok' ? familyFaces(family) : null;
      if (faces) {
        try {
          return await use(faces, bold, italic, family);
        } catch {
          failed.add(family);
        }
      }
      announce(family);
      return use(bundledFaces(), bold, italic, BUNDLED_FAMILY);
    },
    added(result) {
      const first = result.families[0];
      if (first !== undefined) font.pick(first);
      const message = addResultMessage(result);
      if (message !== null) notify(message);
    },
  };
  return font;
}
