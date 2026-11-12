// Test diagnostics (docs/specs/document-tabs.md AC-16): each mode's core
// publishes how many documents it holds, so e2e specs can check that
// closing tabs and leaving an editor frees them. A read-only counter per
// mode on `globalThis.__pincelDiagnostics`; it exposes no document data.
// `calls` counts named core calls whose frequency e2e specs pin down (e.g.
// that a brush stroke does not re-read the selection mask).
import type { EditorMode } from './shell/types';

interface Diagnostics {
  /** Documents the mode's core holds right now. */
  openDocuments: Partial<Record<EditorMode, () => number>>;
  /** How often each counted core call ran since the page loaded. */
  calls: Record<string, number>;
}

type DiagnosticsHost = typeof globalThis & { __pincelDiagnostics?: Diagnostics };

function diagnostics(): Diagnostics {
  const host = globalThis as DiagnosticsHost;
  host.__pincelDiagnostics ??= { openDocuments: {}, calls: {} };
  return host.__pincelDiagnostics;
}

/** Publish `count` as the mode's open-document counter (core loaded). */
export function publishDocumentCount(mode: EditorMode, count: () => number): void {
  diagnostics().openDocuments[mode] = count;
}

/** Counts one run of the core call `name`. */
export function countCall(name: string): void {
  const { calls } = diagnostics();
  calls[name] = (calls[name] ?? 0) + 1;
}
