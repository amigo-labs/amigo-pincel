// The managed ComfyUI install in View ▸ Image generation… (docs/specs/comfyui-install.md):
// the wizard's state and the actions on the managed installation. The
// backend (src-tauri/src/comfyui/managed.rs) runs every job.

import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { decimal, t } from '../i18n/index.svelte';
import type { MessageKey } from '../i18n/en';
import {
  acceptLicenses,
  asInstallError,
  cancelInstall,
  changePackages,
  checkLocation,
  installInfo,
  onInstallEvent,
  resumeInstall,
  startInstall,
  uninstallComfy,
  updateComfy,
  useInstallation,
  type InstallError,
  type InstallInfo,
  type JobKind,
  type LocationCheck,
  type Package,
  type Progress,
} from './install-bridge';
import { defaultSelection, requiredBytes, toggle } from './selection';
import { chooseServer, comfy, reconnect } from './state.svelte';

type Page = 'setup' | 'licenses' | 'progress' | 'failed';

export const install = $state<{
  /** What the backend reported last; `null` until loaded. */
  info: InstallInfo | null;
  /** The wizard is shown instead of the connection dialog. */
  open: boolean;
  /** A new install, or adding / removing packages later (AC-11). */
  mode: 'install' | 'change';
  page: Page;
  location: string | null;
  check: LocationCheck | null;
  selected: string[];
  /** Packages whose "I accept" is checked on the licenses page. */
  accepting: string[];
  /** Change mode: unchecking SDXL base asks before the LoRA goes (AC-11). */
  confirmRemove: string | null;
  job: JobKind | null;
  progress: Progress | null;
  error: InstallError | null;
  /** The connection dialog asks before Uninstall (AC-13). */
  confirmUninstall: boolean;
}>({
  info: null,
  open: false,
  mode: 'install',
  page: 'setup',
  location: null,
  check: null,
  selected: [],
  accepting: [],
  confirmRemove: null,
  job: null,
  progress: null,
  error: null,
  confirmUninstall: false,
});

let unlisten: Promise<UnlistenFn> | null = null;
/** The last package change, for Retry. */
let lastChange: { add: string[]; remove: string[] } = { add: [], remove: [] };

const GIB = 1024 ** 3;
const MIB = 1024 ** 2;

/** Sizes as "512 MB" / "6.5 GB". */
export function bytes(n: number): string {
  return n < GIB ? t('comfy.mb', { n: decimal(n / MIB, 0) }) : t('comfy.gbValue', { n: decimal(n / GIB, 1) });
}

/** Whole gigabytes of a memory size ("6 GB VRAM"). */
export function gib(n: number): string {
  return decimal(n / GIB, 0);
}

const ERRORS: Record<InstallError['kind'], MessageKey> = {
  corrupted: 'comfy.errCorrupted',
  gone: 'comfy.errGone',
  offline: 'comfy.errOffline',
  diskFull: 'comfy.errDiskFull',
  setupFailed: 'comfy.errSetupFailed',
  http: 'comfy.errHttp',
  cancelled: 'comfy.errCancelled',
  updateNotStarted: 'comfy.errUpdateNotStarted',
  generationRunning: 'comfy.errGenerationRunning',
  busy: 'comfy.errBusy',
  failed: 'comfy.errFailed',
};

/** The message of an install error (AC-07, AC-08). */
export function errorText(error: InstallError): string {
  const params: Record<string, string | number> = {};
  if ('file' in error) params.file = error.file;
  if ('status' in error) params.status = error.status;
  if ('message' in error) params.message = error.message;
  return t(ERRORS[error.kind], params);
}

/** The step's name (AC-06). */
export function stepText(progress: Progress, catalog: readonly Package[]): string {
  switch (progress.step) {
    case 'python':
      return t('comfy.stepPython');
    case 'comfyui':
      return t('comfy.stepComfyui');
    case 'packages':
      return t('comfy.stepPackages');
    case 'model': {
      const name = catalog.find((p) => p.id === progress.package)?.name ?? progress.package ?? '';
      return t('comfy.stepModel', { name });
    }
  }
}

function listen(): void {
  unlisten ??= onInstallEvent((event) => {
    install.job = event.job;
    if (event.kind === 'progress') {
      install.progress = event.progress;
      return;
    }
    install.job = null;
    if (event.kind === 'failed') {
      install.error = event.error;
      install.page = 'failed';
      install.open = true;
      void loadInstall();
      return;
    }
    // Done: the dialog shows the started server (AC-10).
    install.open = false;
    install.progress = null;
    comfy.status = event.status;
    void reconnect(false);
    void loadInstall();
  });
}

/** Loads hardware, catalog and the managed installation; reopens the
 *  wizard on a job that still runs. */
export async function loadInstall(): Promise<void> {
  listen();
  try {
    const info = await installInfo();
    if (info === null) return;
    install.info = info;
    if (info.job !== null) {
      install.job = info.job.job;
      install.progress = info.job.progress;
      install.page = 'progress';
      install.open = true;
    }
  } catch (err) {
    comfy.error = String(err);
  }
}

/** "Install ComfyUI…" (AC-01). */
export function openInstallWizard(location: string | null = null): void {
  install.mode = 'install';
  install.page = 'setup';
  install.error = null;
  install.location = null;
  install.check = null;
  install.confirmRemove = null;
  install.selected = defaultSelection(install.info?.catalog ?? []);
  install.open = true;
  if (location !== null) void setLocation(location);
}

/** "Packages…": the package list for adding and removing (AC-11). */
export function openPackages(): void {
  const managed = install.info?.managed;
  install.mode = 'change';
  install.page = 'setup';
  install.error = null;
  install.confirmRemove = null;
  install.selected = managed?.kind === 'complete' ? [...managed.models] : [];
  install.open = true;
}

export function closeWizard(): void {
  install.open = false;
}

async function setLocation(path: string): Promise<void> {
  install.location = path;
  install.check = null;
  try {
    install.check = await checkLocation(path);
  } catch (err) {
    comfy.error = String(err);
  }
}

/** "Choose location…" (AC-03). */
export async function pickLocation(): Promise<void> {
  const picked = await openDialog({ directory: true, multiple: false }).catch(() => null);
  if (typeof picked === 'string') await setLocation(picked);
}

/** Checks or unchecks a package (AC-04); in change mode unchecking SDXL
 *  base asks first when the LoRA would go too (AC-11). */
export function setPackage(id: string, on: boolean): void {
  const catalog = install.info?.catalog ?? [];
  const next = toggle(catalog, install.selected, id, on);
  const loses = install.selected.filter((s) => s !== id && !next.includes(s));
  if (!on && install.mode === 'change' && loses.length > 0) {
    install.confirmRemove = id;
    return;
  }
  install.selected = next;
}

/** "Remove both" / "Keep" of the AC-11 confirmation. */
export function confirmRemoval(yes: boolean): void {
  const id = install.confirmRemove;
  install.confirmRemove = null;
  if (yes && id !== null) {
    install.selected = toggle(install.info?.catalog ?? [], install.selected, id, false);
  }
}

/** Bytes the selection needs (AC-03). */
export function required(): number {
  const info = install.info;
  if (info === null) return 0;
  return requiredBytes(info.catalog, install.selected, info.baseSize ?? 0, info.reserve);
}

/** Packages to download that the user has not accepted yet (AC-05). */
export function unaccepted(): Package[] {
  const info = install.info;
  if (info === null) return [];
  const installed = info.managed.kind === 'complete' ? info.managed.models : [];
  return info.catalog.filter(
    (p) => install.selected.includes(p.id) && !installed.includes(p.id) && !info.accepted.includes(p.id),
  );
}

/** Install / Apply: the licenses page first (AC-05). */
export function toLicenses(): void {
  install.accepting = [];
  install.page = 'licenses';
}

async function run(job: JobKind, call: () => Promise<void>): Promise<void> {
  install.error = null;
  install.progress = null;
  install.job = job;
  install.page = 'progress';
  try {
    await call();
  } catch (err) {
    install.job = null;
    install.error = asInstallError(err);
    install.page = 'failed';
  }
}

/** Accepts the licenses and starts the job (AC-05, AC-06, AC-11). */
export async function startJob(): Promise<void> {
  const ids = unaccepted().map((p) => p.id);
  if (ids.length > 0) {
    try {
      const accepted = await acceptLicenses(ids);
      if (install.info !== null) install.info.accepted = accepted;
    } catch (err) {
      install.error = asInstallError(err);
      install.page = 'failed';
      return;
    }
  }
  if (install.mode === 'install') {
    const location = install.location ?? '';
    const packages = [...install.selected];
    await run('install', () => startInstall(location, packages));
    return;
  }
  const managed = install.info?.managed;
  const installed = managed?.kind === 'complete' ? managed.models : [];
  lastChange = {
    add: install.selected.filter((id) => !installed.includes(id)),
    remove: installed.filter((id) => !install.selected.includes(id)),
  };
  const { add, remove } = lastChange;
  await run('packages', () => changePackages(add, remove));
}

/** Retry after a cancel or an error (AC-09). */
export async function retry(): Promise<void> {
  if (install.mode === 'change') {
    const { add, remove } = lastChange;
    await run('packages', () => changePackages(add, remove));
  } else if (install.job === 'update' || install.error?.kind === 'updateNotStarted') {
    await run('update', updateComfy);
  } else {
    await run('install', resumeInstall);
  }
}

export function cancelJob(): void {
  void cancelInstall();
}

/** "Use this installation" (edge case). */
export async function useThisInstallation(): Promise<void> {
  const location = install.location;
  if (location === null) return;
  try {
    comfy.status = await useInstallation(location);
    install.open = false;
    await reconnect(false);
    await loadInstall();
  } catch (err) {
    install.error = asInstallError(err);
    install.page = 'failed';
  }
}

/** "Resume" of an incomplete install (edge case). */
export async function resumeIncomplete(): Promise<void> {
  install.mode = 'install';
  install.open = true;
  await run('install', resumeInstall);
}

/** "Update ComfyUI" (AC-12). */
export async function startUpdate(): Promise<void> {
  install.mode = 'install';
  install.open = true;
  await run('update', updateComfy);
}

/** Uninstall (AC-13) and Delete of an incomplete install. */
export async function uninstall(): Promise<void> {
  install.confirmUninstall = false;
  try {
    await uninstallComfy();
  } catch (err) {
    comfy.error = errorText(asInstallError(err));
    return;
  }
  await loadInstall();
  await reconnect(true);
}

/** AC-15: "Choose another server" leaves Managed for Automatic. */
export async function chooseAnotherServer(): Promise<void> {
  await chooseServer('automatic');
}

/** AC-15: "Install again…" at the old location. */
export function installAgain(dir: string): void {
  const parent = dir.replace(/[\\/][^\\/]*$/, '');
  openInstallWizard(parent === dir ? null : parent);
}
