<script lang="ts">
  // Foreground / background colors (spec §16.6, §4.2), on the shared
  // ColorPair control.
  import ColorPair from '../../../../lib/components/ColorPair.svelte';
  import Panel from '../../../../lib/components/layout/Panel.svelte';
  import { hexToRgb, toHex } from '../../../../lib/color';
  import { recentColors } from '../../../../lib/colors/recent.svelte';
  import { tool, swapColors, resetColors } from '../../stores/editor.svelte';
</script>

<Panel>
  <ColorPair
    fg={{ ...hexToRgb(tool.foreground), a: tool.foregroundAlpha }}
    bg={{ ...hexToRgb(tool.background), a: tool.backgroundAlpha }}
    onFg={(c) => {
      tool.foreground = toHex(c);
      tool.foregroundAlpha = c.a;
    }}
    onBg={(c) => {
      tool.background = toHex(c);
      tool.backgroundAlpha = c.a;
    }}
    onSwap={swapColors}
    onReset={resetColors}
    recent={recentColors.image}
  />
</Panel>
