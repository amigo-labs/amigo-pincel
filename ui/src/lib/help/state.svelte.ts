// Help menu state, shared by the shell and both editors (spec §9.2).
//
// The Help menu exists wherever the OS draws no native menu bar — the web
// build, and the desktop app on Windows and Linux (spec §11.3). Its entries
// are the ones the native macOS menu serves there: About Pincel, and the
// updater's manual check and auto-install toggle.
//
// The state lives here rather than in a mode's store because `App.svelte`
// renders the dialog: that covers the start screen, where no editor and so
// no menu bar is mounted at all.

export const help = $state<{ about: boolean }>({ about: false });

/** Open the About dialog (Help ▸ About Pincel, or the start screen's button). */
export function openAbout(): void {
  help.about = true;
}

export function closeAbout(): void {
  help.about = false;
}
