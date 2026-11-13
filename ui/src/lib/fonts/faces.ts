// Font families and faces for the Text tools (docs/specs/system-fonts.md,
// "Terms"): which face of a family a Text tool uses for Regular, Bold, Italic
// and Bold Italic, and the sorted, searchable family list. Pure and rune-free,
// so Node-side Playwright specs can test it (AGENTS.md Gotchas).

/** Where a face comes from. */
export type FaceSource = 'bundled' | 'system' | 'added';

/** One face as a font source lists it; reading its bytes is separate. */
export interface FaceInfo {
  readonly source: FaceSource;
  /** Identifies the face within its source (read cache, IndexedDB row). */
  readonly key: string;
  readonly family: string;
  /** The style name the source reports ("Bold Italic", "Condensed", …). */
  readonly style: string;
  /** 100–900. */
  readonly weight: number;
  /** Italic or oblique. */
  readonly italic: boolean;
  /** Width class 1–9; 5 is normal width. */
  readonly stretch: number;
  /** PostScript name, used to find the face inside a collection. */
  readonly postscript: string;
}

/** The family name of the bundled face. */
export const BUNDLED_FAMILY = 'Liberation Sans';

/** The bundled Liberation Sans Regular. */
export const BUNDLED_FACE: FaceInfo = {
  source: 'bundled',
  key: 'bundled',
  family: BUNDLED_FAMILY,
  style: 'Regular',
  weight: 400,
  italic: false,
  stretch: 5,
  postscript: 'LiberationSans',
};

/** Normal width class (OpenType `usWidthClass`). */
export const NORMAL_STRETCH = 5;

// Style-name words to weights, longest first so "ExtraBold" wins over "Bold".
const WEIGHT_WORDS: readonly (readonly [string, number])[] = [
  ['extralight', 200],
  ['ultralight', 200],
  ['extrabold', 800],
  ['ultrabold', 800],
  ['semibold', 600],
  ['demibold', 600],
  ['regular', 400],
  ['normal', 400],
  ['medium', 500],
  ['black', 900],
  ['heavy', 900],
  ['light', 300],
  ['roman', 400],
  ['thin', 100],
  ['bold', 700],
  ['book', 400],
];

/** The weight a style name implies; an unknown name counts as 400. */
export function styleWeight(style: string): number {
  const s = style.toLowerCase().replace(/[\s_-]+/g, '');
  for (const [word, weight] of WEIGHT_WORDS) if (s.includes(word)) return weight;
  return 400;
}

/** Whether a style name means a slanted face ("Italic", "Oblique"). */
export function styleItalic(style: string): boolean {
  return /italic|oblique/i.test(style);
}

// Width words to OpenType width classes.
const STRETCH_WORDS: readonly (readonly [string, number])[] = [
  ['ultracondensed', 1],
  ['extracondensed', 2],
  ['semicondensed', 4],
  ['condensed', 3],
  ['narrow', 3],
  ['semiexpanded', 6],
  ['extraexpanded', 8],
  ['ultraexpanded', 9],
  ['expanded', 7],
  ['wide', 7],
];

/** The width class a style name implies; normal width when it names none. */
export function styleStretch(style: string): number {
  const s = style.toLowerCase().replace(/[\s_-]+/g, '');
  for (const [word, stretch] of STRETCH_WORDS) if (s.includes(word)) return stretch;
  return NORMAL_STRETCH;
}

/** A face whose weight, slant and width come from its style name. */
export function faceFromStyle(
  source: FaceSource,
  key: string,
  family: string,
  style: string,
  postscript: string,
): FaceInfo {
  return {
    source,
    key,
    family,
    style,
    weight: styleWeight(style),
    italic: styleItalic(style),
    stretch: styleStretch(style),
    postscript,
  };
}

/** The faces a Text tool uses for each Bold / Italic combination. */
export interface FamilyFaces {
  readonly regular: FaceInfo;
  readonly bold: FaceInfo | null;
  readonly italic: FaceInfo | null;
  readonly boldItalic: FaceInfo | null;
}

/** Orders candidates: weight distance to `target`, then normal width, then
 *  name order. */
function closest(faces: readonly FaceInfo[], target: number): FaceInfo | null {
  let best: FaceInfo | null = null;
  for (const f of faces) {
    if (best === null || better(f, best, target)) best = f;
  }
  return best;
}

function better(a: FaceInfo, b: FaceInfo, target: number): boolean {
  const da = Math.abs(a.weight - target);
  const db = Math.abs(b.weight - target);
  if (da !== db) return da < db;
  const na = a.stretch === NORMAL_STRETCH;
  const nb = b.stretch === NORMAL_STRETCH;
  if (na !== nb) return na;
  return a.style < b.style;
}

/** The bold face among `faces` other than `regular`: weight ≥ 600, closest
 *  to 700. */
function boldOf(faces: readonly FaceInfo[], regular: FaceInfo | null): FaceInfo | null {
  return closest(
    faces.filter((f) => f !== regular && f.weight >= 600),
    700,
  );
}

/**
 * The Regular, Bold, Italic and Bold Italic faces of one family (spec
 * "Terms", Face choice). A family without upright faces uses its slanted
 * faces as its upright ones. `null` when `faces` is empty.
 */
export function chooseFaces(faces: readonly FaceInfo[]): FamilyFaces | null {
  const slanted = faces.filter((f) => f.italic);
  const uprightOnly = faces.filter((f) => !f.italic);
  const upright = uprightOnly.length > 0 ? uprightOnly : slanted;
  const regular = closest(upright, 400);
  if (regular === null) return null;
  const italic = closest(slanted, 400);
  return {
    regular,
    bold: boldOf(upright, regular),
    italic,
    boldItalic: boldOf(slanted, italic),
  };
}

/**
 * The faces of the "Liberation Sans" entry (AC-23): upright regular text is
 * always the bundled face; Liberation Sans faces among the system or added
 * fonts join it only as its Bold, Italic and Bold Italic faces.
 */
export function bundledFamilyFaces(others: readonly FaceInfo[]): FamilyFaces {
  const slanted = others.filter((f) => f.italic);
  const italic = closest(slanted, 400);
  return {
    regular: BUNDLED_FACE,
    bold: boldOf(
      others.filter((f) => !f.italic),
      null,
    ),
    italic,
    boldItalic: boldOf(slanted, italic),
  };
}

/** The face to rasterize with and what is left to synthesize. */
export interface FaceUse {
  readonly face: FaceInfo;
  readonly synthBold: boolean;
  readonly synthItalic: boolean;
}

/**
 * The face for Bold / Italic on or off (AC-07): the matching face where the
 * family has it, otherwise the nearest face plus synthesis — Bold Italic falls
 * back to Bold + synthesized italic, then Italic + synthesized bold, then
 * Regular with both.
 */
export function faceFor(faces: FamilyFaces, bold: boolean, italic: boolean): FaceUse {
  const use = (face: FaceInfo, synthBold: boolean, synthItalic: boolean): FaceUse => ({
    face,
    synthBold,
    synthItalic,
  });
  if (bold && italic) {
    if (faces.boldItalic) return use(faces.boldItalic, false, false);
    if (faces.bold) return use(faces.bold, false, true);
    if (faces.italic) return use(faces.italic, true, false);
    return use(faces.regular, true, true);
  }
  if (bold) return faces.bold ? use(faces.bold, false, false) : use(faces.regular, true, false);
  if (italic) {
    return faces.italic ? use(faces.italic, false, false) : use(faces.regular, false, true);
  }
  return use(faces.regular, false, false);
}

/** Case-insensitive name order; exact order breaks ties. */
export function compareFamilies(a: string, b: string): number {
  const la = a.toLowerCase();
  const lb = b.toLowerCase();
  if (la !== lb) return la < lb ? -1 : 1;
  return a < b ? -1 : a > b ? 1 : 0;
}

/** Every family of `faces` once (exact, case-sensitive names), sorted
 *  case-insensitively (AC-02). */
export function familyNames(faces: readonly FaceInfo[]): string[] {
  return [...new Set(faces.map((f) => f.family))].sort(compareFamilies);
}

/** The families whose name contains `query`, ignoring case; all for an empty
 *  query (AC-03). */
export function filterFamilies(families: readonly string[], query: string): string[] {
  const q = query.trim().toLowerCase();
  if (q === '') return [...families];
  return families.filter((f) => f.toLowerCase().includes(q));
}

/**
 * The family an agent tool's `font` names (docs/specs/system-fonts-mcp.md
 * AC-06, AC-07, AC-13): the exact name, else the one family matching it
 * ignoring case; `null` when none or several match. The same rule as
 * `match_family` in crates/pincel-agent/src/fonts.rs.
 */
export function matchFamily(families: readonly string[], font: string): string | null {
  if (font === '') return null;
  if (families.includes(font)) return font;
  const lower = font.toLowerCase();
  const matches = families.filter((f) => f.toLowerCase() === lower);
  return matches.length === 1 ? (matches[0] ?? null) : null;
}

/**
 * Merges newly added faces into the stored ones (AC-24): a face with the
 * same family and style name as an earlier one replaces it.
 */
export function replaceFaces<T extends { readonly family: string; readonly style: string }>(
  stored: readonly T[],
  added: readonly T[],
): T[] {
  const id = (f: T): string => `${f.family}\u0000${f.style}`;
  const byId = new Map<string, T>();
  for (const f of stored) byId.set(id(f), f);
  for (const f of added) {
    byId.delete(id(f));
    byId.set(id(f), f);
  }
  return [...byId.values()];
}
