// The live text in the editor (docs/specs/image-live-text.md): the empty box
// a Text press opens, the caret and in-text selection (UI state, never part
// of an undo step), and the edits typing sends to the core's live text. The
// core draws the text and keeps its steps (image-live-objects); the box
// follows the core's reported live object (AC-24).
import { untrack } from 'svelte';
import { t } from '../../../lib/i18n/index.svelte';
import { editor, live, tool } from '../stores/editor.svelte';
import { gestureControl } from '../tools/gesture-control';
import {
  boxOf,
  charsOf,
  deleted,
  hitIndex,
  hitTextBox,
  moved,
  normalized,
  rangeOf,
  replaced,
  type CaretMove,
  type Pt,
} from '../tools/text-caret';
import { beginText, cancelLive, commitLive, editLive } from './live';
import {
  currentFace,
  ensureFace,
  fontEditPending,
  fontEditsDone,
  imageFont,
  loadedFace,
} from './text-font';
import { core, type LiveObjectSpec, type TextLayout } from './wasm';

/** The core's live text. */
export type LiveTextSpec = Extract<LiveObjectSpec, { type: 'text' }>;

/** Editor-side state of the live text. */
export const textEdit = $state({
  /** Top-left of a box opened by a press, before its first character
   *  begins the live text (AC-01); `null` otherwise. */
  pending: null as Pt | null,
  /** A press waiting for the font to load (AC-19). */
  loading: null as Pt | null,
  /** Caret and selection anchor, character indices (AC-03, AC-04). */
  caret: 0,
  anchor: 0,
  /** Keyboard focus belongs in the live text; the text box takes it as
   *  soon as no pointer is down and no other field is being typed in. */
  focusWanted: false,
  /** Bumped with every focus request. */
  focusSeq: 0,
});

/** The live text, or `null` when no live text is open. */
export function textObject(): LiveTextSpec | null {
  const obj = live.object;
  return obj?.type === 'text' ? obj : null;
}

/** Whether a text box shows: a live text or an empty box. */
export function textBoxOpen(): boolean {
  return textObject() !== null || textEdit.pending !== null;
}

/** The text box's layout: the core's layout of the text (or of an empty
 *  line with the tool options), its box and its font size. */
export interface TextView {
  readonly text: string;
  readonly layout: TextLayout;
  readonly box: [number, number, number, number];
}

/** The open text box's view, or `null` when none is open. */
export function textView(): TextView | null {
  const obj = textObject();
  if (obj !== null) {
    const layout = core.textLayout(obj.font_id, {
      text: obj.text,
      x: obj.x,
      y: obj.y,
      size: obj.size,
      bold: obj.bold,
      italic: obj.italic,
      align: obj.align,
    });
    return { text: obj.text, layout, box: boxOf(layout, obj.x, obj.y, obj.size) };
  }
  const at = textEdit.pending;
  const face = loadedFace();
  if (at === null || face === null) return null;
  const layout = core.textLayout(face.id, {
    text: '',
    x: at[0],
    y: at[1],
    size: tool.fontSize,
    bold: face.synthBold,
    italic: face.synthItalic,
    align: tool.textAlign,
  });
  return { text: '', layout, box: boxOf(layout, at[0], at[1], tool.fontSize) };
}

/** Asks for keyboard focus in the live text. */
export function requestTextFocus(): void {
  textEdit.focusWanted = true;
  textEdit.focusSeq += 1;
}

// A typing burst shares one gesture id, so the core keeps it as one step
// (AC-03); a caret jump, an option change, a move or Undo starts the next.
// Ids above the pointer code's stroke ids.
let burst = 0;
let nextBurst = 2 ** 30;
// The text as the editor last set it: a different reported text came from
// Undo / Redo or another route, and the caret goes to the end (AC-13).
let known: string | null = null;

/** Ends the current typing burst. */
export function endBurst(): void {
  burst = 0;
}

function setSel(caret: number, anchor = caret): void {
  textEdit.caret = caret;
  textEdit.anchor = anchor;
}

/** Opens an empty box with its top-left at the pressed pixel. */
function openBox(p: Pt): void {
  textEdit.pending = [Math.floor(p[0]), Math.floor(p[1])];
  setSel(0);
  endBurst();
  requestTextFocus();
}

/**
 * A Text-tool press that hits no open box: closes an empty box and opens a
 * new one at `p` once the font is loaded (AC-01, AC-19). A press while the
 * font loads moves the pending box. The press asks for an undecided
 * local-font permission (docs/specs/system-fonts.md AC-22).
 */
export async function pressText(p: Pt): Promise<void> {
  if (textEdit.loading !== null) {
    textEdit.loading = p;
    return;
  }
  textEdit.pending = null;
  if (currentFace() !== null) {
    openBox(p);
    return;
  }
  textEdit.loading = p;
  try {
    await ensureFace(true);
  } catch {
    textEdit.loading = null;
    editor.notice = t('img.fontFailed');
    return;
  }
  const at = textEdit.loading;
  textEdit.loading = null;
  if (at !== null && tool.kind === 'text') openBox(at);
}

/** What a press at `p` hits on the open box (AC-06, AC-07). */
export function hitText(p: Pt, zoom: number, touch: boolean): 'handle' | 'inside' | null {
  const v = textView();
  if (v === null) return null;
  const hit = hitTextBox(v.box, p, zoom, touch);
  return hit === 'handle' && textObject() === null ? 'inside' : hit;
}

/** Puts the caret at the boundary nearest `p`; `extend` keeps the anchor
 *  (a drag selecting characters, AC-06). */
export function caretToPoint(p: Pt, extend: boolean): void {
  const v = textView();
  if (v === null) return;
  const i = hitIndex(charsOf(v.text), v.layout, p);
  endBurst();
  setSel(i, extend ? textEdit.anchor : i);
}

/** Sends the edited text as part of the current burst. */
function editText(text: string, caret: number): void {
  if (burst === 0) burst = nextBurst++;
  known = text;
  setSel(caret);
  editLive({ gesture: burst, text });
}

/** Types `raw` at the caret, replacing the selection (AC-03, AC-14). The
 *  first character of an empty box begins the live text (AC-01). */
export function typeText(raw: string): void {
  const s = normalized(raw);
  if (s.length === 0) return;
  const obj = textObject();
  if (obj !== null) {
    const r = replaced(charsOf(obj.text), textEdit, s);
    editText(r.text, r.caret);
    return;
  }
  const at = textEdit.pending;
  const face = currentFace() ?? loadedFace();
  if (at === null || face === null) return;
  known = s;
  textEdit.pending = null;
  endBurst();
  if (beginText(at, s, face)) setSel(charsOf(s).length);
}

/** Backspace (`forward` false) or Delete (AC-03). */
export function deleteChar(forward: boolean): void {
  const obj = textObject();
  if (obj === null) return;
  const r = deleted(charsOf(obj.text), textEdit, forward);
  if (r !== null) editText(r.text, r.caret);
}

/** Moves the caret; `extend` extends the selection instead (AC-04). */
export function moveCaret(move: CaretMove, extend: boolean): void {
  const v = textView();
  if (v === null) return;
  const next = moved(charsOf(v.text), v.layout, textEdit.caret, move);
  endBurst();
  setSel(next, extend ? textEdit.anchor : next);
}

/** Selects every character of the live text (AC-04). */
export function selectAllText(): void {
  const v = textView();
  if (v === null) return;
  endBurst();
  setSel(charsOf(v.text).length, 0);
}

/** The selected characters. */
export function selectedText(): string {
  const obj = textObject();
  if (obj === null) return '';
  const [s, e] = rangeOf(textEdit);
  return charsOf(obj.text).slice(s, e).join('');
}

/** Removes the selected characters as a step of their own (Cut, AC-14). */
export function deleteSelection(): void {
  const obj = textObject();
  const [s, e] = rangeOf(textEdit);
  if (obj === null || s === e) return;
  endBurst();
  const r = replaced(charsOf(obj.text), textEdit, '');
  editText(r.text, r.caret);
  endBurst();
}

/** Closes an empty or loading box; `true` when one was open. */
export function closeTextBox(): boolean {
  const open = textEdit.pending !== null || textEdit.loading !== null;
  textEdit.pending = null;
  textEdit.loading = null;
  return open;
}

/** Commits the live text, or closes an empty box (AC-11); a font change
 *  still being read is waited for (docs/specs/system-fonts.md AC-18). */
export function commitText(): void {
  if (fontEditPending()) {
    void fontEditsDone().then(() => {
      commitLive();
      closeTextBox();
    });
    return;
  }
  commitLive();
  closeTextBox();
}

/** Escape in the text: drops a handle drag in progress, else cancels the
 *  live text or closes an empty box (AC-07, AC-21). */
export function cancelText(): void {
  if (gestureControl.cancelLiveDrag()) return;
  cancelLive();
  closeTextBox();
}

/** Installs the box's bookkeeping; call once while the editor mounts. */
export function installLiveText(): void {
  gestureControl.historyMoved = () => {
    known = null;
    endBurst();
    textEdit.pending = null;
  };
  gestureControl.closeTextBox = () => {
    closeTextBox();
  };
  // The box follows the core's live text (AC-13, AC-24).
  let had = false;
  $effect(() => {
    const obj = textObject();
    untrack(() => {
      if (obj === null) {
        had = false;
        return;
      }
      const end = charsOf(obj.text).length;
      if (!had) {
        had = true;
        textEdit.pending = null;
        known = obj.text;
        setSel(end);
        requestTextFocus();
      } else if (obj.text !== known) {
        known = obj.text;
        endBurst();
        setSel(end);
      }
    });
  });
  // Selecting the Text tool needs its family: a missing one is announced
  // (docs/specs/system-fonts.md AC-11).
  $effect(() => {
    if (tool.kind === 'text') untrack(() => void imageFont.needed());
  });
  // Another tool or another document drops an empty or loading box (AC-19).
  $effect(() => {
    void tool.kind;
    void editor.handle;
    untrack(closeTextBox);
  });
}
