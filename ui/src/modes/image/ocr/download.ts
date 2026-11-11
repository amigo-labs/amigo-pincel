// Downloading an OCR model with progress (docs/specs/ocr-dialog.md AC-04,
// AC-06). Rune-free.

/** Bytes received so far and the total, or `null` when the server sends
 *  no usable length. */
export type DownloadProgress = (loaded: number, total: number | null) => void;

/** Why a download failed: the server's status, or the network. */
export class DownloadError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'DownloadError';
  }
}

/**
 * The total a response announces: its `Content-Length`, unless a content
 * encoding makes that the compressed size.
 */
function totalOf(response: Response): number | null {
  const encoding = response.headers.get('Content-Encoding');
  if (encoding !== null && encoding !== '' && encoding !== 'identity') return null;
  const length = Number(response.headers.get('Content-Length') ?? '');
  return Number.isFinite(length) && length > 0 ? length : null;
}

/**
 * Downloads `url` into memory, reporting progress after each chunk. Throws
 * `DownloadError` on a failed response or connection, and the signal's
 * `AbortError` once `signal` aborts.
 */
export async function download(
  url: string,
  signal: AbortSignal,
  progress: DownloadProgress,
): Promise<Uint8Array<ArrayBuffer>> {
  let response: Response;
  try {
    response = await fetch(url, { signal, cache: 'no-store' });
  } catch (err) {
    if (signal.aborted) throw err;
    throw new DownloadError(err instanceof Error ? err.message : String(err));
  }
  if (!response.ok || !response.body) {
    throw new DownloadError(`HTTP ${response.status}`);
  }
  const total = totalOf(response);
  const chunks: Uint8Array[] = [];
  let loaded = 0;
  progress(0, total);
  const reader = response.body.getReader();
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
      loaded += value.byteLength;
      progress(loaded, total);
    }
  } catch (err) {
    if (signal.aborted) throw err;
    throw new DownloadError(err instanceof Error ? err.message : String(err));
  }
  const out = new Uint8Array(loaded);
  let at = 0;
  for (const chunk of chunks) {
    out.set(chunk, at);
    at += chunk.byteLength;
  }
  return out;
}
