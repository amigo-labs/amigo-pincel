// The one encoder the job worker and its main-thread fallback share, so a
// file is byte-identical wherever it was encoded
// (docs/specs/performance-background-jobs.md AC-01).

import { core } from '../core/wasm';

/** What a document can be written as. */
export type EncodeFormat = 'ora' | 'png' | 'jpeg' | 'webp' | 'bmp' | 'tiff' | 'tga' | 'ico';

/** One encoding job: a document snapshot and how to write it. */
export interface EncodeJob {
  /** `core.snapshotDocument` bytes — the document as the job started. */
  readonly snapshot: Uint8Array;
  readonly format: EncodeFormat;
  /** PNG / OpenRaster compression level 0–9. */
  readonly level: number;
  /** JPEG quality 1–100. */
  readonly quality: number;
}

/** Encodes the open document `handle` as `format`. */
export function encodeHandle(
  handle: number,
  format: EncodeFormat,
  level: number,
  quality: number,
): Uint8Array {
  switch (format) {
    case 'ora':
      return core.saveProject(handle, level);
    case 'png':
      return core.exportPng(handle, level);
    case 'jpeg':
      return core.exportJpeg(handle, Math.min(100, Math.max(1, Math.round(quality))));
    case 'webp':
      return core.exportWebp(handle);
    case 'bmp':
      return core.exportBmp(handle);
    case 'tiff':
      return core.exportTiff(handle);
    case 'tga':
      return core.exportTga(handle);
    case 'ico':
      return core.exportIco(handle);
  }
}

/** Runs `job` in this thread's wasm instance. */
export function encodeSnapshot(job: EncodeJob): Uint8Array {
  const handle = core.openSnapshot(job.snapshot);
  try {
    return encodeHandle(handle, job.format, job.level, job.quality);
  } finally {
    core.closeDocument(handle);
  }
}
