// The desktop app's clipboard offer on the start screen
// (docs/specs/copy-cut-paste.md AC-27 – AC-32, Decision Log 2026-11-05):
// the system clipboard image is read natively through the clipboard-manager
// plugin when the start screen appears and when the window gains focus —
// never by polling — and offered once per app session. The browser app
// never calls this module (AC-30).
import { readImage } from '@tauri-apps/plugin-clipboard-manager';
import { SvelteSet } from 'svelte/reactivity';

/** A clipboard image the card shows: straight RGBA plus its content key. */
export interface OfferImage {
  readonly width: number;
  readonly height: number;
  /** Straight RGBA, row-major, top to bottom. */
  readonly rgba: Uint8Array<ArrayBuffer>;
  /** Width, height and a hash of the pixels: "the same image". */
  readonly key: string;
}

class ClipboardOffer {
  /** The image the card offers; `null` while there is no card. */
  image = $state.raw<OfferImage | null>(null);
  /** True from Open until the offered image has opened (or failed). */
  opening = $state(false);
}

export const offer = new ClipboardOffer();

// Keys of the images the card has shown in this app session (AC-28).
const offered = new SvelteSet<string>();
// The newest read; an answer of an older one is dropped (AC-32).
let latest = 0;

interface RawImage {
  width: number;
  height: number;
  rgba: Uint8Array<ArrayBuffer>;
}

/** The clipboard image, or `null` for no image, a refused or unsupported
 *  read, and an image that cannot be decoded or is empty (AC-31). */
async function readClipboard(): Promise<RawImage | null> {
  try {
    const image = await readImage();
    try {
      const { width, height } = await image.size();
      const data = await image.rgba();
      if (!(data.buffer instanceof ArrayBuffer)) return null;
      const rgba = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      if (width <= 0 || height <= 0 || rgba.length !== width * height * 4) return null;
      return { width, height, rgba };
    } finally {
      image.close().catch(() => undefined);
    }
  } catch {
    return null;
  }
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** Two 32-bit FNV-1a hashes with different seeds: the fallback where the
 *  webview has no `crypto.subtle` (an insecure context). */
function fnvKey(rgba: Uint8Array): string {
  let a = 0x811c9dc5;
  let b = 0x050c5d1f;
  for (const v of rgba) {
    a = Math.imul(a ^ v, 0x01000193);
    b = Math.imul(b ^ v, 0x01000193);
  }
  return `${(a >>> 0).toString(16)}${(b >>> 0).toString(16)}`;
}

async function contentKey(img: RawImage): Promise<string> {
  const size = `${img.width}x${img.height}`;
  if (globalThis.crypto?.subtle) {
    const digest = await crypto.subtle.digest('SHA-256', img.rgba);
    return `${size}:${hex(new Uint8Array(digest))}`;
  }
  return `${size}:fnv:${fnvKey(img.rgba)}`;
}

/**
 * Reads the clipboard and lets the card follow it (AC-27, AC-28, AC-31):
 * a new image shows the card, the image it shows keeps it, anything else
 * hides it. `eligible` is asked when the answer arrives — a read started
 * before a tab opened, before Ignore or before a newer read changes
 * nothing (AC-32).
 */
export async function checkClipboard(eligible: () => boolean): Promise<void> {
  latest += 1;
  const seq = latest;
  const read = await readClipboard();
  const key = read ? await contentKey(read).catch(() => null) : null;
  if (seq !== latest || !eligible()) return;
  if (!read || key === null) {
    offer.image = null;
    return;
  }
  if (offer.image?.key === key) return;
  offer.image = offered.has(key) ? null : { ...read, key };
}

/** The card has shown `image`: it is offered for this app session. */
export function markOffered(image: OfferImage): void {
  offered.add(image.key);
}

/** Hides the card and drops the reads in flight (a tab opened, Ignore). */
export function dismissOffer(): void {
  latest += 1;
  offer.image = null;
}

/** Takes the card's image for Open and hides the card. */
export function takeOffer(): OfferImage | null {
  const image = offer.image;
  dismissOffer();
  return image;
}
