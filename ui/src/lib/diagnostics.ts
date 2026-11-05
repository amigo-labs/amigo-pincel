// Test diagnostics (docs/specs/document-tabs.md AC-16): each mode's core
// publishes how many documents it holds, so e2e specs can check that
// closing tabs and leaving an editor frees them. A read-only counter per
// mode on `globalThis.__pincelDiagnostics`; it exposes no document data.
import type { EditorMode } from './shell/types';

interface Diagnostics {
  /** Documents the mode's core holds right now. */
  openDocuments: Partial<Record<EditorMode, () => number>>;
}

type DiagnosticsHost = typeof globalThis & { __pincelDiagnostics?: Diagnostics };

/** Publish `count` as the mode's open-document counter (core loaded). */
export function publishDocumentCount(mode: EditorMode, count: () => number): void {
  const host = globalThis as DiagnosticsHost;
  host.__pincelDiagnostics ??= { openDocuments: {} };
  host.__pincelDiagnostics.openDocuments[mode] = count;
}
