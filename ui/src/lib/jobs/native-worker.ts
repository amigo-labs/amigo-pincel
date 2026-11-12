// The native fetch worker (docs/specs/performance-desktop-jobs.md): sends a
// job to the desktop app's loopback endpoint (`src-tauri/src/job_server.rs`)
// and answers its bytes, off the page's main thread. No wasm. The body goes
// as a `Blob`: WebView2 uploads a typed-array body at about 16 MB/s and a
// `Blob` at about 200 MB/s (AGENTS.md Gotchas).

import type { NativeFetch, NativeFetched } from './native';

interface Scope {
  onmessage: ((e: MessageEvent<NativeFetch>) => unknown) | null;
  postMessage(msg: NativeFetched, transfer: Transferable[]): void;
}

const scope = self as unknown as Scope;

scope.onmessage = async (e) => {
  const { id, url, token, job, body } = e.data;
  try {
    const res = await fetch(url, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}`, 'pincel-job': job },
      body: new Blob([body as Uint8Array<ArrayBuffer>]),
    });
    if (!res.ok) throw new Error(`native job: ${res.status} ${await res.text()}`);
    const bytes = new Uint8Array(await res.arrayBuffer());
    scope.postMessage({ id, ok: true, res: bytes }, [bytes.buffer]);
  } catch (err) {
    scope.postMessage(
      { id, ok: false, error: err instanceof Error ? err.message : String(err) },
      [],
    );
  }
};
