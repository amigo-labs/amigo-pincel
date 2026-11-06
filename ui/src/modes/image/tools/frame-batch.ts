// Brush samples of one animation frame: the pointer handler queues them, and
// once per frame they reach the core in one stroke command, followed by one
// presentation (docs/specs/performance-input-path.md).

/** Pointer samples of a brush drag: canvas-space points with their pen data. */
export interface BrushSamples {
  readonly points: Array<[number, number]>;
  readonly pressure: number[];
  readonly azimuth: Array<number | null>;
}

/** Queues brush samples and flushes them once per animation frame. */
export interface FrameBatch {
  /** Queue samples; `from` (the last point already sent) opens a new batch. */
  queue(from: Readonly<BrushSamples>, samples: Readonly<BrushSamples>): void;
  /** Present in the next animation frame, with or without queued samples. */
  present(): void;
  /** Apply and present what is queued now (stroke end, cancel). */
  flush(): void;
  /** Drop what is queued without applying it (teardown). */
  dispose(): void;
}

/**
 * `apply` paints one batch — the last point sent first, then the new
 * samples, as the core joins consecutive commands of one stroke — and
 * `present` draws the result. Both run at most once per animation frame.
 */
export function frameBatch(apply: (batch: BrushSamples) => void, present: () => void): FrameBatch {
  let pending: BrushSamples | null = null;
  let presentWanted = false;
  let frame: number | null = null;

  const schedule = (): void => {
    frame ??= requestAnimationFrame(() => {
      frame = null;
      flush();
    });
  };

  const flush = (): void => {
    if (frame !== null) {
      cancelAnimationFrame(frame);
      frame = null;
    }
    const batch = pending;
    const wanted = presentWanted;
    pending = null;
    presentWanted = false;
    if (batch) apply(batch);
    if (batch || wanted) present();
  };

  return {
    queue(from, samples) {
      pending ??= {
        points: [...from.points],
        pressure: [...from.pressure],
        azimuth: [...from.azimuth],
      };
      pending.points.push(...samples.points);
      pending.pressure.push(...samples.pressure);
      pending.azimuth.push(...samples.azimuth);
      schedule();
    },
    present() {
      presentWanted = true;
      schedule();
    },
    flush,
    dispose() {
      if (frame !== null) cancelAnimationFrame(frame);
      frame = null;
      pending = null;
      presentWanted = false;
    },
  };
}
