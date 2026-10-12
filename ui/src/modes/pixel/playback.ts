// Animation playback stepping (spec §3.3 frame durations, §3.6 tag
// directions). Pure: the editor's rAF loop owns the clock and calls
// `advance` whenever the current frame's duration has elapsed.

/** Snake_case tag direction names as `pincel-wasm` reports them. */
export type TagDirection = 'forward' | 'reverse' | 'pingpong' | 'pingpong_reverse';

/** The frame range playback loops over, with its direction. */
export interface PlayRange {
  from: number;
  to: number;
  direction: TagDirection;
}

/** Playback cursor: the frame shown and, for ping-pong, the travel sign. */
export interface PlayCursor {
  frame: number;
  /** `1` moving forward, `-1` moving backward. */
  step: 1 | -1;
}

/** The cursor playback starts from when `frame` is the current frame. */
export function startCursor(range: PlayRange, frame: number): PlayCursor {
  const inRange = frame >= range.from && frame <= range.to;
  const reverse = range.direction === 'reverse' || range.direction === 'pingpong_reverse';
  return {
    frame: inRange ? frame : reverse ? range.to : range.from,
    step: reverse ? -1 : 1,
  };
}

/** Next cursor after the current frame's duration elapsed. */
export function advance(range: PlayRange, cursor: PlayCursor): PlayCursor {
  const { from, to } = range;
  if (from >= to) return { frame: from, step: cursor.step };
  const next = cursor.frame + cursor.step;
  switch (range.direction) {
    case 'forward':
      return { frame: next > to ? from : next, step: 1 };
    case 'reverse':
      return { frame: next < from ? to : next, step: -1 };
    case 'pingpong':
    case 'pingpong_reverse':
      if (next > to) return { frame: to - 1, step: -1 };
      if (next < from) return { frame: from + 1, step: 1 };
      return { frame: next, step: cursor.step };
  }
}

/** Onion-skin view settings the timeline edits (spec §4.2). */
export interface OnionSettings {
  enabled: boolean;
  back: number;
  forward: number;
  /** `0..=255`, used for both directions. */
  alpha: number;
}

/** Identifier-like tag names keep the animation state machine happy (spec §3.6). */
export function isIdentifierLike(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(name);
}
