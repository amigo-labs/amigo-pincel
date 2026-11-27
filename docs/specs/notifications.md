---
status: implemented
created: 2026-11-26
size: S
mode: both
design: [pincel.md §9.2]
---

# Notices as toasts

## Summary

> Completed: 2026-11-26

- Every editor notice shows as a toast (`role="status"`, Dismiss): centred on the status bar on windows of 640 px and wider, above the zoom control and the panels sheet on phones.
- It goes after 5 seconds, held while hovered or focused; a newer notice replaces it; tool names and Pixel mode's idle texts raise none (`StatusInfo.notice`).
- The status bar keeps the last notice; a shown toast stays usable while an effect is open or an answer runs.

## Context

Part of the UI refresh (owner request 2026-11-25; mock-up `docs/design/ui-refresh-mockup.html`: "confirmations and failures appear centred above the status bar, also on the phone"). It replaces the backlog entry "Error surfacing".

Both editors report what happened in one line of text: "Saved held.aseprite", "Layer 1 is locked", a failed export. That line is the status bar's text region (`frame-layout`). Two problems:

- A window narrower than 640 px has no status bar, so a phone shows no notice at all. A failed save there is silent.
- On a wide window the notice appears in the bottom-left corner in small, muted text, far from where the user looks; a failure is as quiet as the tool's name it replaces.

Terms:

- **Notice**: the one-line text an editor reports after an action — a confirmation, a refusal or a failure. Not a notice: the active tool's name and Pixel mode's idle texts "initializing…" and "ready", which the status bar shows while there is nothing to report.
- **Toast**: the small box this spec adds, showing the current notice over the canvas area.
- **Status bar**, **canvas area**: as in `frame-layout`.

## Scope

### In scope

- A toast for every notice of both modes, on every window width
- How long it stays, how it is dismissed, how a newer notice replaces it
- Its name and role for assistive technology
- Strings: the dismiss control reuses the shell's "Dismiss" text (all six UI languages have it)

### Out of scope

- Severity (a different look for failures) — because the editors report notices as plain text without a kind; telling a failure from a confirmation needs every one of the ~120 places that set a notice to say which it is. Backlog.
- A list or history of past notices — because the status bar keeps the last one on windows that have it.
- Changing the status bar — because it stays the place where the last notice can be read again.
- Notices of the shell outside an editor (start screen) — because they have their own inline messages.
- Showing the same text again when an action reports exactly the notice that is still the current one — because the editors hold a notice as a value, not as an event; see Edge cases.

## Acceptance criteria

- [x] AC-01 (event): When an editor reports a notice, then a toast with the notice's text appears, in both modes and on every window width: centred on the status bar on windows of 640 px and wider, not higher than the bar, so it covers nothing of the canvas area; centred at the lower edge of the canvas area on narrower windows, which have no status bar.
- [x] AC-02 (always): The toast is announced as a status message (`role="status"`) and has a control named "Dismiss"; it takes no pointer input outside its own box.
- [x] AC-03 (event): When 5 seconds have passed since the toast appeared, then it goes; while the pointer is over the toast or the focus is inside it, it stays, and it goes 5 seconds after both have left.
- [x] AC-04 (event): When the user activates Dismiss, then the toast goes at once.
- [x] AC-05 (event): When an editor reports another notice while a toast shows, then the toast shows the new text and its 5 seconds start again; there is never more than one toast.
- [x] AC-06 (state): While the status bar's text is the active tool's name or one of Pixel mode's idle texts ("initializing…", "ready"), no toast appears for it — also not when the document has just been opened or the tool changes.
- [x] AC-07 (state): While the window is 640 px wide or wider, the status bar shows the notice as before, also after the toast has gone.
- [x] AC-08 (state): While the window is narrower than 640 px, the toast sits above the floating zoom control and the panels sheet, wholly inside the canvas area.
- [x] AC-09 (state): While an effect is open or an Assistant answer runs, a toast that shows stays on screen and its Dismiss works.
- [x] AC-10 (option): Where the UI language is German, the dismiss control is named "Schließen" and the toast shows the German notice.

## Technical notes

Hints, not contracts.

- The editors hand the frame a `StatusInfo` (`ui/src/lib/components/layout/status.ts`): `text` is the notice or the tool name. Pixel mode: `ed.status || toolLabel`, with the idle texts `px.initializing` and `px.ready`; Image mode: `statusNotice ?? toolLabel`. The frame needs to know which it is.
- The canvas area is `main[data-stage]` in `EditorFrame.svelte`; the floating zoom control's lift (`zoomLift`) already accounts for the sheet and the docked effect panel on narrow windows.
- `shell.dismiss` is the existing "Dismiss" string.
- e2e: Image mode reports "… is locked" for a press on a locked layer; Pixel mode reports saves and refusals through `ed.status`. Many specs find a notice by its text page-wide; with the toast the text is on the page twice on wide windows.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The same notice is reported again while it is still the status bar's text and its toast has gone | No new toast: the editors hold the notice as a value, and it did not change. A different notice in between shows it again |
| The notice is cleared (the status bar returns to the tool name) while its toast shows | The toast goes |
| A notice longer than the canvas area is wide | The toast is at most as wide as the canvas area minus a margin and wraps |
| Toggle UI hides the bars | A notice still shows as a toast |
| The editor's tab goes into the background while a toast shows | The toast goes with the editor; it does not reappear on return |
| The language changes while a toast shows | The toast follows the notice's text as the editor reports it |
| Reduce motion | The toast appears and goes without animation |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 5 seconds is long enough to read one line | Common for toasts of this length; hover and focus hold it, and the status bar keeps the text on wide windows | Low |
| A toast for every notice is not too noisy | Notices are reported on the user's own actions (save, refusal, failure), not continuously | Medium — a chatty notice would need a kind (see Out of scope) |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/notifications.spec.ts`: "wide › {pixel, image}: a notice shows as a toast on the status bar; idle texts and tool names do not", "narrow › {pixel, image}: on a phone the toast sits above the zoom control and the panels sheet, inside the canvas area" | verified | d73b48a |
| AC-02 | `ui/e2e/notifications.spec.ts`: "wide › {pixel, image}: a notice shows as a toast on the status bar; idle texts and tool names do not" | verified | d73b48a |
| AC-03 | `ui/e2e/notifications.spec.ts`: "wide › {pixel, image}: the toast goes after 5 seconds, not while the pointer is over it; the status bar keeps the text", "wide › {pixel, image}: without the pointer the toast is gone after 5 seconds; focus inside holds it" | verified | d73b48a |
| AC-04 | `ui/e2e/notifications.spec.ts`: "wide › {pixel, image}: Dismiss removes the toast at once; a newer notice replaces the shown one" | verified | d73b48a |
| AC-05 | `ui/e2e/notifications.spec.ts`: "wide › {pixel, image}: Dismiss removes the toast at once; a newer notice replaces the shown one" | verified | d73b48a |
| AC-06 | `ui/e2e/notifications.spec.ts`: "wide › {pixel, image}: a notice shows as a toast on the status bar; idle texts and tool names do not" | verified | d73b48a |
| AC-07 | `ui/e2e/notifications.spec.ts`: "wide › {pixel, image}: a notice shows as a toast …", "wide › {pixel, image}: the toast goes after 5 seconds, not while the pointer is over it; the status bar keeps the text" | verified | d73b48a |
| AC-08 | `ui/e2e/notifications.spec.ts`: "narrow › {pixel, image}: on a phone the toast sits above the zoom control and the panels sheet, inside the canvas area" | verified | d73b48a |
| AC-09 | `ui/e2e/notifications.spec.ts`: "wide › {pixel, image}: while an effect is open the toast shows and can be dismissed"; `ui/e2e/refresh-while-answering.spec.ts`: "1280 × 720 › image: a toast that shows stays while an answer runs and its Dismiss works" | verified | d73b48a |
| AC-10 | `ui/e2e/notifications.spec.ts`: "wide › German: the toast shows the German notice and its control reads \"Schließen\"" | verified | d73b48a |

## Decisions

- 2026-11-25 (owner, mock-up): notices appear as a toast centred above the status bar, also on phones.
- 2026-11-26 (spec): one toast for every notice, without a severity; the status bar keeps the last notice on windows that have one.
- 2026-11-26 (build): `StatusInfo.notice` tells the frame what is a notice: Image mode's notice (or load error), Pixel mode's status text unless it is "initializing…" or "ready". The frame shows a toast when that value changes; an editor whose tab is in the background shows none.
- 2026-11-26 (build): the toast lies in the canvas area, 12 px above its lower edge; on a phone it is lifted above the floating zoom control, which already clears the panels sheet and the docked effect panel.
- 2026-11-26 (build): red run — the 13 tests of `ui/e2e/notifications.spec.ts` all failed against the bundle built before the change. Pixel mode's status bar never shows the tool's name while idle (it reads "ready"); the test for AC-06 expects that.
- 2026-11-26 (build): New public API, for review: `StatusInfo.notice`.
- 2026-11-26 (build, AC-01 revised): the first version put the toast over the lower edge of the canvas area on every width. With the view fitted it covered the bottom of the artwork for five seconds after every notice — rotating, a refused stroke, placing text — and e2e specs that compare canvas screenshots saw it. On windows with a status bar the toast now sits on the bar, centred and no higher than it; phones keep it over the canvas area because there is nowhere else.
- 2026-11-26 (acceptance, AC-09 reworded): it read "a notice still shows as a toast", but while an effect is open or an answer runs the canvas and every menu that reports a notice take no input, so no user action can report one; what matters is that a toast already shown is not hidden behind the inert regions and can be dismissed. The tests always checked that.
- 2026-11-26 (acceptance, round 1: NOT READY): the "answer runs" half of AC-09 had no test; added (with the AC reworded, see above).
