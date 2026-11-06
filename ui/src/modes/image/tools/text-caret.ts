// Caret, selection and hit test of a live text (docs/specs/image-live-text.md
// AC-03 – AC-06): pure functions over the text's characters and the layout
// the core paints with (`core.textLayout`, AC-22). Indices count Unicode
// characters (code points) across the whole text, line breaks included, as
// the layout's caret lists do.
import type { TextLayout } from '../core/wasm';

/** A point in canvas (document) space. */
export type Pt = [number, number];

/** The caret and the selection anchor; equal when nothing is selected. */
export interface TextSel {
  readonly caret: number;
  readonly anchor: number;
}

/** The characters of `text`. */
export function charsOf(text: string): string[] {
  return Array.from(text);
}

/** The selected range `[start, end)`. */
export function rangeOf(sel: TextSel): [number, number] {
  return [Math.min(sel.caret, sel.anchor), Math.max(sel.caret, sel.anchor)];
}

/** Line and column of character boundary `index`. */
export function lineCol(chars: readonly string[], index: number): [number, number] {
  let line = 0;
  let col = 0;
  for (let i = 0; i < index && i < chars.length; i += 1) {
    if (chars[i] === '\n') {
      line += 1;
      col = 0;
    } else {
      col += 1;
    }
  }
  return [line, col];
}

/** The boundary index of `col` on `line` (clamped to the line). */
export function indexAt(chars: readonly string[], line: number, col: number): number {
  let start = 0;
  for (let l = 0; l < line; l += 1) {
    const nl = chars.indexOf('\n', start);
    if (nl < 0) return chars.length;
    start = nl + 1;
  }
  const nl = chars.indexOf('\n', start);
  const end = nl < 0 ? chars.length : nl;
  return Math.min(start + Math.max(0, col), end);
}

/** Text normalized for insertion: CR LF and lone CR become LF. */
export function normalized(text: string): string {
  return text.replace(/\r\n?/g, '\n');
}

/** `chars` with the selection replaced by `insert`; the caret after it. */
export function replaced(
  chars: readonly string[],
  sel: TextSel,
  insert: string,
): { text: string; caret: number } {
  const [s, e] = rangeOf(sel);
  const add = charsOf(insert);
  return { text: [...chars.slice(0, s), ...add, ...chars.slice(e)].join(''), caret: s + add.length };
}

/** Backspace (`forward` false) or Delete: the selection, else one
 *  character before / after the caret. `null` when nothing is removed. */
export function deleted(
  chars: readonly string[],
  sel: TextSel,
  forward: boolean,
): { text: string; caret: number } | null {
  const [s, e] = rangeOf(sel);
  if (s !== e) return replaced(chars, sel, '');
  if (forward ? s >= chars.length : s <= 0) return null;
  const from = forward ? s : s - 1;
  return replaced(chars, { caret: from, anchor: from + 1 }, '');
}

/** A caret movement key. */
export type CaretMove = 'left' | 'right' | 'up' | 'down' | 'home' | 'end';

/** The caret x of boundary `col` on layout line `line`. */
function caretX(layout: TextLayout, line: number, col: number): number {
  const l = layout.lines[line];
  if (!l) return 0;
  return l.carets[Math.min(col, l.carets.length - 1)] ?? l.left;
}

/** The column on `line` whose caret is nearest to `x`. */
function nearestCol(layout: TextLayout, line: number, x: number): number {
  const carets = layout.lines[line]?.carets ?? [];
  let best = 0;
  carets.forEach((c, i) => {
    if (Math.abs(c - x) < Math.abs((carets[best] ?? 0) - x)) best = i;
  });
  return best;
}

/** Where `move` takes the caret at `caret` (AC-04). */
export function moved(
  chars: readonly string[],
  layout: TextLayout,
  caret: number,
  move: CaretMove,
): number {
  const [line, col] = lineCol(chars, caret);
  switch (move) {
    case 'left':
      return Math.max(0, caret - 1);
    case 'right':
      return Math.min(chars.length, caret + 1);
    case 'home':
      return indexAt(chars, line, 0);
    case 'end':
      return indexAt(chars, line, Number.MAX_SAFE_INTEGER);
    case 'up':
    case 'down': {
      const next = line + (move === 'up' ? -1 : 1);
      if (next < 0) return indexAt(chars, 0, 0);
      if (next >= layout.lines.length) return chars.length;
      return indexAt(chars, next, nearestCol(layout, next, caretX(layout, line, col)));
    }
  }
}

/** The caret of boundary `index`: x, line top and height (AC-05). */
export function caretRect(
  chars: readonly string[],
  layout: TextLayout,
  index: number,
): { x: number; top: number; height: number } {
  const [line, col] = lineCol(chars, index);
  return {
    x: caretX(layout, line, col),
    top: layout.lines[line]?.top ?? 0,
    height: layout.line_height,
  };
}

/** The boundary nearest to canvas point `p` (AC-06). */
export function hitIndex(chars: readonly string[], layout: TextLayout, p: Pt): number {
  const first = layout.lines[0]?.top ?? 0;
  const line = Math.min(
    layout.lines.length - 1,
    Math.max(0, Math.floor((p[1] - first) / (layout.line_height || 1))),
  );
  return indexAt(chars, line, nearestCol(layout, line, p[0]));
}

/** The highlight rects of the selected range, one per line it touches. */
export function selectionRects(
  chars: readonly string[],
  layout: TextLayout,
  sel: TextSel,
): Array<{ x: number; y: number; w: number; h: number }> {
  const [s, e] = rangeOf(sel);
  if (s === e) return [];
  const [l0, c0] = lineCol(chars, s);
  const [l1, c1] = lineCol(chars, e);
  const out: Array<{ x: number; y: number; w: number; h: number }> = [];
  for (let line = l0; line <= l1; line += 1) {
    const l = layout.lines[line];
    if (!l) continue;
    const from = line === l0 ? caretX(layout, line, c0) : l.left;
    const to = line === l1 ? caretX(layout, line, c1) : l.left + l.width;
    // A selected line break shows as a sliver past the line's end.
    const w = Math.max(to - from, line < l1 ? layout.line_height / 4 : 0);
    if (w > 0) out.push({ x: from, y: l.top, w, h: layout.line_height });
  }
  return out;
}

/** The text box `[x0, y0, x1, y1]`: as wide as the widest line, at least
 *  half the font size; one line high at least (AC-06, AC-09). */
export function boxOf(
  layout: TextLayout,
  left: number,
  top: number,
  size: number,
): [number, number, number, number] {
  const w = Math.max(layout.width, size / 2);
  const h = Math.max(layout.lines.length, 1) * layout.line_height;
  return [left, top, left + w, top + h];
}

/** Hit radius of the move handle, CSS px: mouse, and touch / pen
 *  (image-live-objects-ui AC-05). */
const HANDLE_REACH = { mouse: 8, touch: 16 } as const;
/** Margin of the hit area around the box, CSS px (AC-06). */
const BOX_MARGIN = 4;

/** What a press at canvas point `p` hits on the text box `box`: the move
 *  handle at its bottom-right corner, the hit area, or nothing. `zoom` is
 *  CSS px per canvas px. */
export function hitTextBox(
  box: readonly [number, number, number, number],
  p: Pt,
  zoom: number,
  touch: boolean,
): 'handle' | 'inside' | null {
  const [x0, y0, x1, y1] = box;
  const reach = (touch ? HANDLE_REACH.touch : HANDLE_REACH.mouse) / zoom;
  if (Math.hypot(p[0] - x1, p[1] - y1) <= reach) return 'handle';
  const m = BOX_MARGIN / zoom;
  return p[0] >= x0 - m && p[0] <= x1 + m && p[1] >= y0 - m && p[1] <= y1 + m ? 'inside' : null;
}
