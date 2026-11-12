// Recent files whose file is gone (docs/specs/file-access.md AC-30, AC-31):
// a click on one removes it, and the desktop app drops such entries before
// a list shows. Only "not found" counts; a denied permission or a locked
// file keeps its entry, because it may open again later.

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '../platform';
import { listRecents, removeRecents, type RecentFile } from '../idb/recent-files';

/** For each path, whether a file is there (`files_exist`); `null` when the
 *  desktop app cannot tell (an older build without the command). */
async function filesExist(paths: readonly string[]): Promise<boolean[] | null> {
  try {
    const answer = await invoke<unknown>('files_exist', { paths });
    return Array.isArray(answer) && answer.length === paths.length
      ? answer.map((v) => v === true)
      : null;
  } catch (err) {
    console.error('files_exist failed', err);
    return null;
  }
}

/** Whether `err`, from re-opening `recent`, means its file no longer
 *  exists: in the desktop app the path holds no file, in the browser the
 *  handle reports `NotFoundError`. */
export async function recentIsGone(recent: RecentFile, err: unknown): Promise<boolean> {
  if (isTauri() && recent.path) {
    const [present] = (await filesExist([recent.path])) ?? [true];
    return present === false;
  }
  return err instanceof DOMException && err.name === 'NotFoundError';
}

/** Removes `recent` from the registry after `recentIsGone`; every recent
 *  list refreshes through `onRecentsChanged`. */
export async function forgetRecent(recent: RecentFile): Promise<void> {
  try {
    await removeRecents([recent.id]);
  } catch (err) {
    console.error('removeRecents failed', err);
  }
}

/** The recents to show, newest first. In the desktop app, entries whose
 *  path holds no file leave the registry first (AC-31); the browser shows
 *  every entry, because checking a stored handle may ask for permission. */
export async function listPresentRecents(): Promise<RecentFile[]> {
  const rows = await listRecents();
  if (!isTauri()) return rows;
  const withPath = rows.filter((r): r is RecentFile & { path: string } => r.path !== null);
  if (withPath.length === 0) return rows;
  const present = await filesExist(withPath.map((r) => r.path));
  if (present === null) return rows;
  const gone = new Set(withPath.filter((_, i) => present[i] === false).map((r) => r.id));
  if (gone.size === 0) return rows;
  await removeRecents([...gone]);
  return rows.filter((r) => !gone.has(r.id));
}
