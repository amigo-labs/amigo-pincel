import { browser } from '$app/environment';

const STORAGE_KEY = 'pincel:crt-enabled';

function read(): boolean {
  if (!browser) return true;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === null) return true;
    return raw === '1';
  } catch {
    return true;
  }
}

function write(value: boolean): void {
  if (!browser) return;
  try {
    localStorage.setItem(STORAGE_KEY, value ? '1' : '0');
  } catch {
    /* ignore quota / disabled storage */
  }
}

class CrtState {
  enabled = $state(true);

  hydrate(): void {
    this.enabled = read();
  }

  toggle(): void {
    this.enabled = !this.enabled;
    write(this.enabled);
  }
}

export const crt = new CrtState();
