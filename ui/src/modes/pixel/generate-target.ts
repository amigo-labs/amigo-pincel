// Pixel mode's side of image generation
// (docs/specs/comfyui-generate-pixel.md): the target box, input and mask
// come from the sprite through the core — the input is the active layer's
// cel in the current frame as the sprite shows it, enlarged with hard
// edges (AC-07, AC-09) — and each variant is scaled to the target box and
// mapped into the sprite's color mode before it is previewed or applied
// as a new layer (AC-02 – AC-06). The panel, the job and the variants live
// in `lib/comfyui/generation.svelte`.

import { generationMaskPng, generationSize } from '../../lib/core';
import { targetBox, type Box, type GenTarget } from '../../lib/comfyui/generate-params';
import { syncMeta, syncSelection } from './render';
import { ed, live, tabs, type PixelTab } from './stores/editor.svelte';

/** The generation key of a Pixel tab. */
export function pixelGenKey(tab: PixelTab): string {
  return `pixel:${tab.id}`;
}

function area(box: Box): Int32Array {
  return Int32Array.of(box.x, box.y, box.w, box.h);
}

/** The generation target of `tab`'s current document. */
export function pixelTarget(tab: PixelTab): GenTarget {
  const doc = tab.doc.doc;
  return {
    key: pixelGenKey(tab),
    alive: () => doc !== null && tab.doc.doc === doc,
    box: () => {
      if (doc === null) return null;
      const sel = doc.hasSelection
        ? ([doc.selectionX, doc.selectionY, doc.selectionWidth, doc.selectionHeight] as const)
        : null;
      return targetBox(sel, doc.width, doc.height);
    },
    size: (native, box) => {
      const [w = 64, h = 64] = generationSize(native, box.w, box.h);
      return [w, h];
    },
    mask: (box) => (doc === null ? new Uint8Array() : doc.generationMask(box.x, box.y, box.w, box.h)),
    input: (box, width, height) =>
      doc === null ? new Uint8Array() : doc.generationInput(box.x, box.y, box.w, box.h, width, height),
    maskPng: (mask, box, width, height) => generationMaskPng(mask, box.w, box.h, width, height),
    blocked: () => (doc === null || doc.canHoldGenerated ? null : 'noVisibleColor'),
    // The strip shows each variant as Apply would write it (AC-02).
    thumbnail: (variant, box, mask) => (doc === null ? variant : doc.generationPreview(variant, area(box), mask)),
    apply: (variant, box, mask, name) => {
      if (doc === null) return;
      tab.doc.activeLayerId = doc.addGeneratedLayer(variant, area(box), mask, name);
      if (tabs.active !== tab) return;
      live.needsRecompose = true;
      syncMeta();
      syncSelection();
      ed.docRev += 1;
    },
  };
}
