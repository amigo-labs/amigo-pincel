<script lang="ts">
  // Foreground / background colors (spec §16.6, §4.2), on the shared
  // ColorPair control.
  import ColorPair from '../../../../lib/components/ColorPair.svelte';
  import Panel from '../../../../lib/components/layout/Panel.svelte';
  import { tool, swapColors, resetColors } from '../../stores/editor.svelte';
</script>

<Panel>
  <ColorPair
    fg={tool.foreground}
    bg={tool.background}
    onFg={(hex) => (tool.foreground = hex)}
    onBg={(hex) => (tool.background = hex)}
    onSwap={swapColors}
    onReset={resetColors}
  />
</Panel>
