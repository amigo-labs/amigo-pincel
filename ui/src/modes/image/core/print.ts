// Image-mode printing (docs/specs/image-formats-more-print.md): the
// composite as a PNG in a print-only element on <body>; `app.css` shows it
// alone under print media and hides it on screen. Removed after the
// platform dialog closes, or before the next print at the latest (AC-07).
import { core } from './wasm';

/** How the image sits on the page. */
export type PrintPlacement = 'fit' | 'actual';

const CLASS = 'pincel-print';

/** Removes a print element left from an earlier print, and its URL. */
export function clearPrint(): void {
  for (const el of document.querySelectorAll<HTMLElement>(`body > .${CLASS}`)) {
    const url = el.dataset['url'];
    if (url) URL.revokeObjectURL(url);
    el.remove();
  }
}

/**
 * Prints the composite of `handle` (canvas `width` × `height`) placed as
 * `placement`: encodes it, waits until the print image has loaded, then
 * opens the platform print dialog once. Rejects — and opens nothing — when
 * the image cannot be prepared (AC-11).
 */
export async function printComposite(
  handle: number,
  width: number,
  height: number,
  placement: PrintPlacement,
): Promise<void> {
  clearPrint();
  // Low compression: a large print stays fast (Technical notes).
  const png = core.exportPng(handle, 1);
  const url = URL.createObjectURL(new Blob([png.slice()], { type: 'image/png' }));
  const holder = document.createElement('div');
  holder.className = CLASS;
  holder.dataset['url'] = url;
  const img = document.createElement('img');
  img.alt = '';
  img.className = placement;
  if (placement === 'actual') {
    img.style.width = `${width}px`;
    img.style.height = `${height}px`;
  }
  holder.append(img);
  document.body.append(holder);
  try {
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = () => reject(new Error('the print image did not load'));
      img.src = url;
    });
  } catch (err) {
    clearPrint();
    throw err;
  }
  window.addEventListener('afterprint', clearPrint, { once: true });
  window.print();
}
