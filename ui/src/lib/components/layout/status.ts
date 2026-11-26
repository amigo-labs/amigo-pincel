/** What an editor hands the frame for the status bar; both modes show
 *  the same layout (docs/specs/frame-layout.md AC-06 – AC-08). */
export interface StatusInfo {
  /** The document is loaded: until then the bar shows only `text`. */
  ready: boolean;
  /** The notice, or the active tool's name while there is none. */
  text: string;
  /** The notice to show as a toast, or `null` while `text` is the tool's
   *  name or an idle text (docs/specs/notifications.md AC-06). */
  notice?: string | null;
  /** `text` is a warning (drawn as a warning chip). */
  warning?: boolean;
  /** The pointer position, or `null` while there is none to show. */
  position: string | null;
  /** The document size. */
  size: string;
  /** Pixel mode: the current frame, e.g. `Frame 1/4`. */
  frame?: string | null;
}

/** Room one status-bar item takes beside its neighbour (the row's gap). */
const GAP = 12;
/** The row's own padding plus the gap after the text region. */
const ROW_FIXED = 18 + GAP;

/**
 * Which optional items the status bar leaves out when the row is too
 * narrow for everything, even with an empty text region: `1` drops the
 * pointer position, `2` the probe readout as well (AC-07). `tail` is the
 * width of the items that always show, `position` and `probe` the widths
 * of the optional ones (`0` while absent).
 */
export function statusDrop(row: number, tail: number, position: number, probe: number): 0 | 1 | 2 {
  const item = (w: number): number => (w > 0 ? w + GAP : 0);
  if (ROW_FIXED + tail + item(position) + item(probe) <= row) return 0;
  if (ROW_FIXED + tail + item(probe) <= row) return 1;
  return 2;
}
