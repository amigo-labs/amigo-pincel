import { expect, test } from '@playwright/test';
import { en } from '../src/lib/i18n/en';
import {
  BLOCK_ENGLISH,
  checkGenImage,
  enlarges,
  familyOf,
  layerName,
  parseSeed,
  statusIssue,
  targetBox,
} from '../src/lib/comfyui/generate-params';

// The pure rules of the Generate panel and `gen.image`
// (docs/specs/comfyui-generate.md), run in Node without a page.

test('the target box is the selection clipped to the canvas, empty outside it (AC-04)', () => {
  expect(targetBox(null, 100, 50)).toEqual({ x: 0, y: 0, w: 100, h: 50 });
  expect(targetBox([90, 40, 20, 20], 100, 50)).toEqual({ x: 90, y: 40, w: 10, h: 10 });
  expect(targetBox([-5, -5, 10, 10], 100, 50)).toEqual({ x: 0, y: 0, w: 5, h: 5 });
  expect(targetBox([120, 0, 10, 10], 100, 50)).toBeNull();
  expect(targetBox([0, 60, 10, 10], 100, 50)).toBeNull();
});

test('the layer name is "Generated: " and 30 characters of the prompt (AC-12)', () => {
  expect(layerName('  a red apple on a table, photorealistic  ')).toBe('Generated: a red apple on a table, photor');
  expect(layerName('🍎'.repeat(40))).toBe(`Generated: ${'🍎'.repeat(30)}`);
});

test('seed, model family and status (AC-02, AC-03)', () => {
  expect(parseSeed('')).toBeNull();
  expect(parseSeed(' 42 ')).toBe(42);
  expect(parseSeed('-1')).toBeUndefined();
  expect(parseSeed('1.5')).toBeUndefined();
  expect(familyOf({ name: 'a', family: 'flux' }, { a: 'sd15' })).toBe('flux');
  expect(familyOf({ name: 'a', family: null }, { a: 'sd15' })).toBe('sd15');
  expect(familyOf({ name: 'b', family: null }, {})).toBe('sdxl');
  expect(statusIssue({ kind: 'notFound' })).toEqual({ key: 'comfy.notFound' });
  expect(statusIssue({ kind: 'notConfigured' })).toEqual({ key: 'gen.notSetUp' });
});

test('gen.image defaults to the panel model, else the first checkpoint (AC-16, AC-17)', () => {
  const models = {
    checkpoints: [
      { name: 'a.safetensors', family: null },
      { name: 'b.safetensors', family: null },
    ],
    loras: [],
  };
  expect(checkGenImage({ prompt: 'x' }, models, 'b.safetensors')).toMatchObject({ model: 'b.safetensors' });
  expect(checkGenImage({ prompt: 'x' }, models, 'gone.safetensors')).toMatchObject({ model: 'a.safetensors' });
  expect(checkGenImage({ prompt: 'x' }, models, null)).toEqual({
    prompt: 'x',
    negative: '',
    model: 'a.safetensors',
    lora: null,
    loraStrength: 1,
    strength: 1,
    seed: null,
  });
  expect(checkGenImage({ prompt: 'x', lora: 'l' }, models, null)).toBe('unknown LoRA "l"; valid LoRAs: (none)');
  expect(checkGenImage({ prompt: 'x', seed: 1.5 }, models, null)).toBe('`seed` must be an integer of at least 0');
});

test('Pixel mode notes a target box larger than the generation (comfyui-generate-pixel AC-02)', () => {
  expect(enlarges({ x: 0, y: 0, w: 32, h: 32 }, 512, 512)).toBe(false);
  expect(enlarges({ x: 0, y: 0, w: 512, h: 512 }, 512, 512)).toBe(false);
  expect(enlarges({ x: 0, y: 0, w: 640, h: 640 }, 512, 512)).toBe(true);
  expect(enlarges({ x: 0, y: 0, w: 2048, h: 100 }, 2048, 512)).toBe(false);
  expect(enlarges({ x: 0, y: 0, w: 100, h: 600 }, 512, 512)).toBe(true);
});

test('a palette without a visible color reads the same in the panel and in gen.image (comfyui-generate-pixel AC-10)', () => {
  expect(BLOCK_ENGLISH.noVisibleColor).toBe('The palette has no visible color');
  expect(en['gen.noVisibleColor']).toBe(BLOCK_ENGLISH.noVisibleColor);
});
