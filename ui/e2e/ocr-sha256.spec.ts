import { createHash } from 'node:crypto';
import { expect, test } from '@playwright/test';
import { OCR_MODELS } from '../src/modes/image/ocr/catalog';
import { sha256Plain } from '../src/modes/image/ocr/sha256';
import { modelBytes } from './ocr-helpers';

// The plain SHA-256 the OCR model check falls back to where a webview has
// no `crypto.subtle` (docs/specs/ocr-dialog.md AC-05, AC-20). Node-side:
// no page.

const nodeSha = (data: Uint8Array): string => createHash('sha256').update(data).digest('hex');

test('the plain SHA-256 matches node:crypto around every padding boundary', () => {
  for (const size of [0, 1, 55, 56, 57, 63, 64, 65, 119, 120, 128, 1000]) {
    const data = Uint8Array.from({ length: size }, (_, i) => (i * 31 + 7) & 0xff);
    expect(sha256Plain(data), `size ${size}`).toBe(nodeSha(data));
  }
});

test('the pinned hashes are the committed models’ SHA-256, plain and node:crypto alike', () => {
  for (const lang of ['deu', 'eng'] as const) {
    const bytes = new Uint8Array(modelBytes(lang));
    expect(nodeSha(bytes)).toBe(OCR_MODELS[lang].sha256);
    expect(sha256Plain(bytes)).toBe(OCR_MODELS[lang].sha256);
  }
});
