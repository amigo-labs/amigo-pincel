// Mounts the shared tool rail with made-up groups, for flyout states no
// shipping mode has (a disabled variant inside a group). Served by the Vite
// dev server that `tool-flyout-harness.spec.ts` starts; not part of the app.
import { mount, unmount } from 'svelte';
import ToolRailHarness from './ToolRailHarness.svelte';
import '../../src/app.css';

const target = document.getElementById('harness');
if (!target) throw new Error('missing #harness root element');

const harness = mount(ToolRailHarness, { target });

/** Hooks the spec drives from `page.evaluate`. */
export interface HarnessHooks {
  setDisabled: (id: string, disabled: boolean) => void;
  replaceDocument: () => void;
  unmount: () => void;
}

const hooks: HarnessHooks = {
  setDisabled: (id, disabled) => harness.setDisabled(id, disabled),
  replaceDocument: () => harness.replaceDocument(),
  unmount: () => void unmount(harness),
};
(window as unknown as { harness: HarnessHooks }).harness = hooks;
