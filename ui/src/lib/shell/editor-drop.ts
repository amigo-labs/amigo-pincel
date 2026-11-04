// Sorting a drop onto an open editor (docs/specs/file-access.md AC-20,
// AC-24, AC-26): a project file opens as the document, raster images go
// to the add-or-open choice, everything else gets a notice.

import { formatFromName, sniffFormat, type FileFormat, type OpenedFile } from '../fs';
import type { DroppedFile } from '../fs/drop';
import { t } from '../i18n/index.svelte';

/** A raster image of a drop, with the format told by content, else by name. */
export interface DroppedImage {
  file: OpenedFile;
  format: FileFormat;
}

/** What a drop onto an open editor does. */
export type DropPlan =
  /** Open `file` as the document; `others` more files were dropped with it. */
  | { kind: 'project'; file: OpenedFile; others: number }
  /** Ask (or follow the remembered choice) what to do with the images. */
  | { kind: 'images'; images: DroppedImage[]; others: number }
  /** Nothing usable was dropped. */
  | { kind: 'none' };

/** The plan for `dropped`, plus a notice for each file that cannot take part. */
export function planDrop(dropped: readonly DroppedFile[]): { plan: DropPlan; notices: string[] } {
  const notices: string[] = [];
  const images: DroppedImage[] = [];
  let project: OpenedFile | null = null;
  for (const d of dropped) {
    if (!d.file) {
      notices.push(t('drop.readFailed', { name: d.name, error: d.error ?? '' }));
      continue;
    }
    const format = sniffFormat(d.file.bytes) ?? formatFromName(d.file.name);
    if (format === null) {
      notices.push(t('shell.unsupported', { name: d.name }));
    } else if (format === 'aseprite' || format === 'ora') {
      project ??= d.file;
    } else {
      images.push({ file: d.file, format });
    }
  }
  // A project replaces the document: adding layers to the old one first
  // would be thrown away (AC-24).
  if (project) {
    return { plan: { kind: 'project', file: project, others: dropped.length - 1 }, notices };
  }
  if (images.length > 0) {
    return { plan: { kind: 'images', images, others: dropped.length - 1 }, notices };
  }
  return { plan: { kind: 'none' }, notices };
}
