// View ▸ Toggle UI and Tab on the canvas stage
// (docs/specs/power-user-shortcuts-coverage.md AC-09, AC-17, AC-18): hides
// the tool rail, the options bar and the panels, keeping the menu bar and
// the status bar so the way back stays visible. Kept in memory only, so a
// reload always shows the full UI.

/** Editor-frame chrome state shared by both editors. */
export const chrome = $state<{ hidden: boolean }>({ hidden: false });

/** Hides or shows the tool rail, options bar and panels. */
export function toggleUi(): void {
  chrome.hidden = !chrome.hidden;
}
