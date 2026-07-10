import type { ComposeFrame } from '../core';
import type { CanvasRenderer } from './types';

// Full-screen-triangle blit of the composed sprite texture (spec §4.4).
// `pincel-core` composes at 1×, so the texture is sprite-sized and the
// canvas backing store matches it 1:1 — nearest sampling is exact and the
// element's CSS upscale (`image-rendering: pixelated`) does the zoom.
//
// The composed RGBA is non-premultiplied; the canvas is configured
// `premultiplied`, so the fragment shader multiplies rgb by alpha. That
// reproduces how the Canvas2D path composites non-premultiplied
// `ImageData` over the page, so transparent sprite pixels reveal the
// wrapper background identically on both backends.
const SHADER = /* wgsl */ `
@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var tex: texture_2d<f32>;

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
  var corners = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0),
  );
  let xy = corners[vi];
  var out: VsOut;
  out.pos = vec4<f32>(xy, 0.0, 1.0);
  // Clip space y is up; texture v is down — flip so row 0 is at top.
  out.uv = vec2<f32>((xy.x + 1.0) * 0.5, (1.0 - xy.y) * 0.5);
  return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
  let c = textureSample(tex, samp, in.uv);
  return vec4<f32>(c.rgb * c.a, c.a);
}
`;

/**
 * WebGPU implementation of [`CanvasRenderer`] (spec §4.4). Construct via
 * the async [`WebGPURenderer.create`] factory, which resolves to `null`
 * when WebGPU is unavailable or device acquisition fails so the caller
 * can fall back to Canvas2D.
 */
export class WebGPURenderer implements CanvasRenderer {
  readonly backend = 'webgpu' as const;

  readonly #canvas: HTMLCanvasElement;
  readonly #device: GPUDevice;
  readonly #context: GPUCanvasContext;
  readonly #pipeline: GPURenderPipeline;
  readonly #sampler: GPUSampler;

  #texture: GPUTexture | null = null;
  #bindGroup: GPUBindGroup | null = null;
  #texW = 0;
  #texH = 0;
  #lost = false;

  private constructor(
    canvas: HTMLCanvasElement,
    device: GPUDevice,
    context: GPUCanvasContext,
    pipeline: GPURenderPipeline,
    sampler: GPUSampler,
  ) {
    this.#canvas = canvas;
    this.#device = device;
    this.#context = context;
    this.#pipeline = pipeline;
    this.#sampler = sampler;
    // A lost device can't be revived in place; flag so subsequent draws
    // no-op. The caller keeps the (now inert) instance; a reload picks a
    // fresh device. Best-effort — not a hot-swap path.
    void device.lost.then((info) => {
      this.#lost = true;
      if (info.reason !== 'destroyed') {
        console.error(`WebGPU device lost: ${info.message}`);
      }
    });
  }

  /**
   * Acquire a WebGPU device and configure the canvas. Resolves to `null`
   * (never throws) when WebGPU is missing, no adapter is available, or
   * device / context setup fails.
   */
  static async create(canvas: HTMLCanvasElement): Promise<WebGPURenderer | null> {
    if (typeof navigator === 'undefined' || !navigator.gpu) return null;
    try {
      const adapter = await navigator.gpu.requestAdapter();
      if (!adapter) return null;
      const device = await adapter.requestDevice();
      const format = navigator.gpu.getPreferredCanvasFormat();
      const module = device.createShaderModule({ code: SHADER });
      const pipeline = device.createRenderPipeline({
        layout: 'auto',
        vertex: { module, entryPoint: 'vs' },
        fragment: { module, entryPoint: 'fs', targets: [{ format }] },
        primitive: { topology: 'triangle-list' },
      });
      const sampler = device.createSampler({ magFilter: 'nearest', minFilter: 'nearest' });
      // Touch the canvas last: a canvas locks to its first context type,
      // so we only claim 'webgpu' once everything fallible has succeeded.
      // That keeps the Canvas2D fallback viable if any step above fails.
      const context = canvas.getContext('webgpu');
      if (!context) {
        device.destroy();
        return null;
      }
      context.configure({ device, format, alphaMode: 'premultiplied' });
      return new WebGPURenderer(canvas, device, context, pipeline, sampler);
    } catch (err) {
      console.error('WebGPU init failed', err);
      return null;
    }
  }

  draw(frame: ComposeFrame): void {
    if (this.#lost) return;
    const { width, height } = frame;
    if (width === 0 || height === 0) return;
    if (this.#canvas.width !== width) this.#canvas.width = width;
    if (this.#canvas.height !== height) this.#canvas.height = height;
    this.#ensureTexture(width, height);
    this.#upload(frame.pixels, 0, 0, width, height);
    this.#render();
  }

  drawDirty(frame: ComposeFrame): void {
    if (this.#lost) return;
    const { width, height, dirtyX, dirtyY } = frame;
    if (width === 0 || height === 0) return;
    // A full `draw` always precedes the first dirty blit (the tick loop
    // routes the initial paint through the full path), so the texture is
    // sized. Guard anyway — a missing texture just waits for that draw.
    if (!this.#texture) return;
    this.#upload(frame.pixels, dirtyX, dirtyY, width, height);
    this.#render();
  }

  destroy(): void {
    this.#texture?.destroy();
    this.#texture = null;
    this.#bindGroup = null;
    this.#device.destroy();
  }

  // (Re)create the sprite texture + its bind group when the dimensions
  // change. The bind group binds the texture view, so it's rebuilt in
  // lockstep with the texture.
  #ensureTexture(width: number, height: number): void {
    if (this.#texture && this.#texW === width && this.#texH === height) return;
    this.#texture?.destroy();
    this.#texture = this.#device.createTexture({
      size: { width, height },
      format: 'rgba8unorm',
      usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
    });
    this.#texW = width;
    this.#texH = height;
    this.#bindGroup = this.#device.createBindGroup({
      layout: this.#pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: this.#sampler },
        { binding: 1, resource: this.#texture.createView() },
      ],
    });
  }

  // Copy a tightly-packed RGBA8 sub-image into the texture at (x, y).
  // `writeTexture` from CPU data has no row-alignment constraint, so
  // `bytesPerRow = width * 4` is valid for any width.
  #upload(pixels: Uint8Array, x: number, y: number, width: number, height: number): void {
    // Re-view over a concrete ArrayBuffer: the wasm getter types `pixels`
    // as `Uint8Array<ArrayBufferLike>`, which doesn't satisfy
    // `GPUAllowSharedBufferSource`. The runtime buffer is always a plain
    // ArrayBuffer (same cast canvas2d.ts uses for `ImageData`).
    const buffer = pixels.buffer as ArrayBuffer;
    const data = new Uint8Array(buffer, pixels.byteOffset, pixels.byteLength);
    this.#device.queue.writeTexture(
      { texture: this.#texture!, origin: { x, y } },
      data,
      { bytesPerRow: width * 4, rowsPerImage: height },
      { width, height },
    );
  }

  // Redraw the whole canvas from the (possibly partially-updated)
  // texture. One textured triangle — trivially cheap; the real cost the
  // dirty path saves is the compose + upload, not this blit.
  #render(): void {
    if (!this.#bindGroup) return;
    const encoder = this.#device.createCommandEncoder();
    const view = this.#context.getCurrentTexture().createView();
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        { view, clearValue: { r: 0, g: 0, b: 0, a: 0 }, loadOp: 'clear', storeOp: 'store' },
      ],
    });
    pass.setPipeline(this.#pipeline);
    pass.setBindGroup(0, this.#bindGroup);
    pass.draw(3);
    pass.end();
    this.#device.queue.submit([encoder.finish()]);
  }
}
