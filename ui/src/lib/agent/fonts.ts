// The editor's fonts for live agent calls (docs/specs/system-fonts-mcp.md
// AC-11): `font.list` answers from the app's own font list, and before a
// `draw.text` with `font` is forwarded to the wasm tools, the face the
// editor would use is read here — the tools run in wasm, which cannot read
// a font file synchronously — and handed over with the font list
// (`pincel_agent::live::WireFonts`).
import {
  BUNDLED_FACE,
  BUNDLED_FAMILY,
  NORMAL_STRETCH,
  faceFor,
  matchFamily,
  type FaceInfo,
} from '../fonts/faces';
import {
  familyFaces,
  familyList,
  fontSource,
  initFontSource,
  readFace,
  requestSystemFonts,
} from '../fonts/source.svelte';
import { ok, type Wire } from './wire';

/** One face the webview read for a call (by its index in the font list),
 *  or why it could not. */
export type AgentFaceRead =
  | { readonly face: number; readonly bytes: Uint8Array; readonly index: number }
  | { readonly face: number; readonly error: string };

/** The editor's font list for one call: its faces (the bundled one aside)
 *  and the faces read for the call. */
export interface AgentFonts {
  readonly faces: readonly FaceInfo[];
  readonly read: readonly AgentFaceRead[];
}

/** Waits until the system fonts are listed, as the font list does. */
async function listed(): Promise<void> {
  await initFontSource();
  await requestSystemFonts();
}

/** The faces the font list holds besides the bundled one. */
function listFaces(): FaceInfo[] {
  return [...fontSource.system, ...fontSource.added];
}

/** Style names of one family's faces: weight, upright before slanted, then
 *  width and name, each once (AC-02; `style_names` in pincel-agent). */
function styleNames(faces: readonly FaceInfo[]): string[] {
  const sorted = [...faces].sort(
    (a, b) =>
      a.weight - b.weight ||
      Number(a.italic) - Number(b.italic) ||
      Math.abs(a.stretch - NORMAL_STRETCH) - Math.abs(b.stretch - NORMAL_STRETCH) ||
      a.stretch - b.stretch ||
      (a.style < b.style ? -1 : a.style > b.style ? 1 : 0),
  );
  return [...new Set(sorted.map((f) => f.style))];
}

/** `font.list`: the families of the app's font list with their style
 *  names; a non-empty `query` keeps the names containing it, ignoring case
 *  (AC-02, AC-03). Reads no face (AC-10). */
export async function fontList(args: Record<string, unknown>): Promise<Wire> {
  await listed();
  const query = typeof args.query === 'string' ? args.query.toLowerCase() : '';
  const faces = listFaces();
  const families = familyList()
    .filter((name) => query === '' || name.toLowerCase().includes(query))
    .map((name) => {
      const own = faces.filter((f) => f.family === name);
      return { name, styles: styleNames(name === BUNDLED_FAMILY ? [...own, BUNDLED_FACE] : own) };
    });
  return ok({ families });
}

function reason(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/**
 * The fonts a call of `tool` needs: for a `draw.text` with `font`, the
 * font list plus the face the editor uses for that family, Bold and Italic,
 * read once per session (`readFace`); `null` for any other call, which
 * keeps the bundled face alone (AC-04). A family the list does not match
 * reads nothing: the tools answer with their own error (AC-07, AC-13).
 */
export async function callFonts(
  tool: string,
  args: Record<string, unknown>,
): Promise<AgentFonts | null> {
  if (tool !== 'draw.text' || args.font === undefined) return null;
  await listed();
  const faces = listFaces();
  const read: AgentFaceRead[] = [];
  const family = typeof args.font === 'string' ? matchFamily(familyList(), args.font) : null;
  const chosen = family === null ? null : familyFaces(family);
  if (chosen !== null) {
    const use = faceFor(chosen, args.bold === true, args.italic === true);
    const face = faces.findIndex((f) => f.key === use.face.key);
    if (face >= 0) {
      try {
        const { bytes, index } = await readFace(use.face);
        read.push({ face, bytes, index });
      } catch (err) {
        read.push({ face, error: reason(err) });
      }
    }
  }
  return { faces, read };
}

/** `fonts` as the wasm tools take them (`pincel_agent::live::WireFonts`),
 *  each read face registered with the mode's core by `register`. */
export function agentFontsJson(
  fonts: AgentFonts,
  register: (bytes: Uint8Array, index: number) => number,
): string {
  return JSON.stringify({
    faces: fonts.faces.map((f) => ({
      family: f.family,
      style: f.style,
      weight: f.weight,
      italic: f.italic,
      stretch: f.stretch,
    })),
    read: fonts.read.map((r) =>
      'error' in r
        ? { face: r.face, error: r.error }
        : { face: r.face, font: register(r.bytes, r.index) },
    ),
  });
}
