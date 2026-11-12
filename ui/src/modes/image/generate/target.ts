// Image mode's side of image generation (docs/specs/comfyui-generate.md):
// what a generation reads from an Image tab's document — the target box,
// the input and the mask from the core, never from the canvas (AC-08) —
// and how a variant is fitted, previewed and applied as a new layer. The
// panel, the job and the variants live in `lib/comfyui/generation.svelte`.

import { core } from '../core/wasm';
import { syncInfo } from '../core/controller';
import { editor, type ImageTab } from '../stores/editor.svelte';
import { gestureControl } from '../tools/gesture-control';
import { targetBox, type GenTarget } from '../../../lib/comfyui/generate-params';
import type { TabGen } from '../../../lib/comfyui/generation.svelte';

/** The generation key of an Image tab. */
export function genKey(tab: ImageTab): string {
  return `image:${tab.id}`;
}

/** The generation target of `tab`'s current document. */
export function imageTarget(tab: ImageTab): GenTarget {
  const handle = tab.doc.handle;
  return {
    key: genKey(tab),
    alive: () => handle !== null && tab.doc.handle === handle,
    box: () => {
      if (handle === null) return null;
      const info = core.documentInfo(handle);
      return targetBox(info.has_selection ? core.selectionBounds(handle) : null, info.width, info.height);
    },
    size: (native, box) => core.generationSize(native, box.w, box.h),
    mask: (box) => (handle === null ? new Uint8Array() : core.generationMask(handle, box.x, box.y, box.w, box.h)),
    input: (box, width, height) =>
      handle === null ? new Uint8Array() : core.generationInput(handle, box.x, box.y, box.w, box.h, width, height),
    maskPng: (mask, box, width, height) => core.generationMaskPng(mask, box.w, box.h, width, height),
    blocked: () => null,
    // The strip shows the variants as they came (comfyui-generate AC-10).
    thumbnail: (variant) => variant,
    apply: (variant, box, mask, name) => {
      if (handle === null) return;
      // A pending live object is committed first, as before any command.
      gestureControl.commitLive();
      gestureControl.commitTransform();
      const rgba = core.fitVariant(variant, box.w, box.h, mask);
      core.addGeneratedLayer(handle, rgba, box.x, box.y, box.w, box.h, name);
      if (editor.handle === handle) syncInfo();
    },
  };
}

/** The variant fitted to its target box with the mask applied. */
export function fittedVariant(gen: TabGen, index: number): Uint8ClampedArray | null {
  const data = gen.variants[index];
  return data ? core.fitVariant(data, gen.box.w, gen.box.h, gen.mask) : null;
}
