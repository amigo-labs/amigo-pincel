// In-page instrumentation for the browser performance script
// (docs/specs/performance-benchmarks.md AC-03). Installed with
// `page.addInitScript(installProbe)`, so it runs before the app and must be
// self-contained (Playwright serializes the function).
//
// - Frames: a requestAnimationFrame loop records frame timestamps while
//   `recording` is on; frame time = the gap between two frames.
// - Draws: `putImageData` / `drawImage` on the render target and WebGPU
//   `writeTexture` are timestamped. The app never reads canvas pixels; the
//   probe only observes that a draw happened.
// - Dab latency: every pointer event (capture phase, its `timeStamp`) is
//   matched with the first draw after it, and "presented" is the first frame
//   that starts after that draw — pointer event → pixels presented.
// - Memory: every `WebAssembly.Memory` the app instantiates is kept, so the
//   script can read `buffer.byteLength`.

/** One pointer event on its way to the screen. */
export interface PerfInput {
  readonly type: string;
  readonly t: number;
  drawnAt: number | null;
  presentedAt: number | null;
}

/** The probe's state on `window.__perf`. */
export interface PerfState {
  recording: boolean;
  frames: number[];
  inputs: PerfInput[];
  draws: number[];
  target: HTMLCanvasElement | null;
  memories: WebAssembly.Memory[];
  /** `performance.now()` of named marks (e.g. a file input's change). */
  marks: Record<string, number>;
}

declare global {
  interface Window {
    __perf: PerfState;
  }
}

/** Installs the probe; passed to `page.addInitScript`. */
export function installProbe(): void {
  const perf: PerfState = {
    recording: false,
    frames: [],
    inputs: [],
    draws: [],
    target: null,
    memories: [],
    marks: {},
  };
  window.__perf = perf;

  const onDraw = (canvas: unknown): void => {
    if (perf.target !== null && canvas !== perf.target) return;
    const now = performance.now();
    if (perf.recording) perf.draws.push(now);
    for (const input of perf.inputs) {
      if (input.drawnAt === null && input.t <= now) input.drawnAt = now;
    }
  };
  for (const proto of [
    CanvasRenderingContext2D.prototype,
    OffscreenCanvasRenderingContext2D.prototype,
  ]) {
    for (const name of ['putImageData', 'drawImage'] as const) {
      const original = proto[name] as (...args: unknown[]) => void;
      Object.defineProperty(proto, name, {
        configurable: true,
        writable: true,
        value(this: { canvas: unknown }, ...args: unknown[]) {
          original.apply(this, args);
          onDraw(this.canvas);
        },
      });
    }
  }
  const gpuQueue = (globalThis as { GPUQueue?: { prototype: GPUQueue } }).GPUQueue;
  if (gpuQueue) {
    const original = gpuQueue.prototype.writeTexture as (...args: unknown[]) => void;
    Object.defineProperty(gpuQueue.prototype, 'writeTexture', {
      configurable: true,
      writable: true,
      value(this: GPUQueue, ...args: unknown[]) {
        original.apply(this, args);
        onDraw(perf.target);
      },
    });
  }

  const onInput = (e: Event): void => {
    if (perf.recording) {
      perf.inputs.push({ type: e.type, t: e.timeStamp, drawnAt: null, presentedAt: null });
    }
  };
  for (const type of ['pointerdown', 'pointermove']) {
    window.addEventListener(type, onInput, { capture: true, passive: true });
  }
  for (const type of ['change', 'input']) {
    document.addEventListener(
      type,
      (e) => {
        perf.marks[type] = e.timeStamp;
      },
      { capture: true },
    );
  }

  const frame = (ts: number): void => {
    if (perf.recording) perf.frames.push(ts);
    for (const input of perf.inputs) {
      if (input.drawnAt !== null && input.presentedAt === null && ts > input.drawnAt) {
        input.presentedAt = ts;
      }
    }
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);

  const keep = (result: unknown): void => {
    const instance =
      result instanceof WebAssembly.Instance
        ? result
        : (result as { instance?: WebAssembly.Instance }).instance;
    for (const value of Object.values(instance?.exports ?? {})) {
      if (value instanceof WebAssembly.Memory) perf.memories.push(value);
    }
  };
  const wasm = WebAssembly as unknown as Record<
    'instantiate' | 'instantiateStreaming',
    (...args: unknown[]) => Promise<unknown>
  >;
  for (const name of ['instantiate', 'instantiateStreaming'] as const) {
    const original = wasm[name];
    wasm[name] = async (...args: unknown[]) => {
      const result = await original.apply(WebAssembly, args);
      keep(result);
      return result;
    };
  }
}
