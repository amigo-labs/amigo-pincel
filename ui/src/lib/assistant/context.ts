// The editor-context block in front of every user message of the
// Assistant chat (docs/specs/ai-chat-panel.md AC-15). Plain TypeScript, so
// a Node-side spec can test it.

/** What the block states about the chat's document. */
export interface EditorContext {
  name: string;
  mode: 'pixel' | 'image';
  width: number;
  height: number;
  selection: { x: number; y: number; w: number; h: number } | null;
  /** Bottom to top, as the layer tools list them. */
  layers: { name: string; active: boolean }[];
  /** Names of the undo steps, oldest first, in the app's language. */
  steps: string[];
}

/** Layer names the block lists at most. */
export const MAX_LAYERS = 50;
/** Undo steps the block names. */
export const RECENT_STEPS = 5;

/** A name as the block shows it: one line, and no `<` / `>` / `&` that
 *  could end the block early. */
export function escapeName(name: string): string {
  return name
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/[\r\n]+/g, ' ');
}

/** The block for `c`. */
export function contextBlock(c: EditorContext): string {
  const mode = c.mode === 'pixel' ? 'Pixel' : 'Image';
  const sel = c.selection
    ? `${c.selection.x},${c.selection.y} ${c.selection.w}×${c.selection.h}`
    : 'none';
  const shown = c.layers.slice(0, MAX_LAYERS).map((l) => {
    const name = escapeName(l.name);
    return l.active ? `*${name} (active)` : name;
  });
  const more = c.layers.length - shown.length;
  if (more > 0) shown.push(`… and ${more} more`);
  const steps = c.steps.slice(-RECENT_STEPS).map(escapeName);
  return [
    '<editor-context>',
    `document: ${escapeName(c.name)} (${mode}), ${c.width}×${c.height}`,
    `selection: ${sel}`,
    `layers: ${shown.length > 0 ? shown.join(', ') : 'none'}`,
    `recent steps: ${steps.length > 0 ? steps.join(', ') : 'none'}`,
    '</editor-context>',
  ].join('\n');
}

/** The message the agent receives: the block, then the user's text. */
export function withContext(c: EditorContext, text: string): string {
  return `${contextBlock(c)}\n\n${text}`;
}

/** One piece of agent text (AC-21): plain text, inline code or a code
 *  block. The panel renders each as text, never as HTML. */
export interface TextSegment {
  kind: 'text' | 'code' | 'block';
  text: string;
}

/** Splits agent text into plain text, `inline code` and fenced code
 *  blocks; an unclosed fence runs to the end. */
export function segments(text: string): TextSegment[] {
  const out: TextSegment[] = [];
  const push = (kind: TextSegment['kind'], t: string): void => {
    if (t !== '') out.push({ kind, text: t });
  };
  const fence = /```[^\n]*\n?([\s\S]*?)(?:```|$)/g;
  let at = 0;
  for (let m = fence.exec(text); m !== null; m = fence.exec(text)) {
    inline(text.slice(at, m.index), push);
    push('block', (m[1] ?? '').replace(/\n$/, ''));
    at = m.index + m[0].length;
    if (m[0].length === 0) break;
  }
  inline(text.slice(at), push);
  return out;
}

function inline(text: string, push: (kind: TextSegment['kind'], t: string) => void): void {
  const parts = text.split(/(`[^`\n]+`)/);
  for (const p of parts) {
    if (p.length > 2 && p.startsWith('`') && p.endsWith('`')) push('code', p.slice(1, -1));
    else push('text', p);
  }
}
