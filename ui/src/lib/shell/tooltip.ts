// Placement and text helpers for the tooltip layer (`TooltipLayer.svelte`).
// Plain functions: no DOM, no runes.

/** A box in viewport coordinates. */
export interface TipBox {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Where a tooltip sits relative to its element. */
export type TipSide = 'below' | 'right';

/** Gap between the element and its tooltip, and the least distance to the viewport edge. */
export const TIP_GAP = 6;

/**
 * Top-left corner of a `width` × `height` tooltip for `anchor`.
 *
 * `below` centres it under the element and flips above when the viewport
 * is too short; `right` puts it beside the element (the vertical tool
 * rail, whose next button sits below) and flips to the left. The result
 * stays inside the viewport.
 */
export function placeTip(
  anchor: TipBox,
  width: number,
  height: number,
  viewport: { width: number; height: number },
  side: TipSide,
): { left: number; top: number } {
  let left: number;
  let top: number;
  if (side === 'right') {
    left = anchor.right + TIP_GAP;
    if (left + width > viewport.width - TIP_GAP) left = anchor.left - TIP_GAP - width;
    top = (anchor.top + anchor.bottom) / 2 - height / 2;
  } else {
    left = (anchor.left + anchor.right) / 2 - width / 2;
    top = anchor.bottom + TIP_GAP;
    if (top + height > viewport.height - TIP_GAP) top = anchor.top - TIP_GAP - height;
  }
  const clamp = (v: number, max: number): number => Math.round(Math.max(TIP_GAP, Math.min(v, max)));
  return {
    left: clamp(left, viewport.width - TIP_GAP - width),
    top: clamp(top, viewport.height - TIP_GAP - height),
  };
}

/**
 * Splits a tooltip text that ends in ` (shortcut)` into label and key, so
 * the key can be drawn as a keycap. Without a known shortcut, or when the
 * text does not end in it, the whole text is the label.
 */
export function splitShortcut(
  text: string,
  shortcut: string | undefined,
): { label: string; key: string | null } {
  const tail = shortcut ? ` (${shortcut})` : '';
  if (tail && text.endsWith(tail) && text.length > tail.length) {
    return { label: text.slice(0, -tail.length), key: shortcut ?? null };
  }
  return { label: text, key: null };
}
