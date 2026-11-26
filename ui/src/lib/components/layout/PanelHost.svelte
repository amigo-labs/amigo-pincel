<script lang="ts">
  // Renders one frame panel and provides its `PanelSlot` to the `Panel`
  // chrome inside (context needs a component boundary; a snippet cannot
  // set it).
  import type { Snippet } from 'svelte';
  import { providePanelSlot } from './panels.svelte';

  interface Props {
    title: string;
    host: 'side' | 'bottom' | 'sheet';
    collapsed: boolean;
    /** A tab of a group (see `PanelSlot.grouped`). */
    grouped?: boolean;
    /** The user removed the panel (see `PanelSlot.removed`). */
    removed?: boolean;
    onToggle: () => void;
    body: Snippet;
  }
  const props: Props = $props();

  providePanelSlot({
    get title() {
      return props.title;
    },
    get host() {
      return props.host;
    },
    get collapsed() {
      return props.collapsed;
    },
    get grouped() {
      return props.grouped === true;
    },
    get removed() {
      return props.removed === true;
    },
    toggle: () => props.onToggle(),
  });
</script>

{@render props.body()}
