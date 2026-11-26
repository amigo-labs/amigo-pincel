<script lang="ts">
  // The tool rail's colour well in Image mode (docs/specs/tool-rail-colors.md):
  // the Colors panel's two swatches with swap and reset, on the same colours.
  import ColorPair from '../../../lib/components/ColorPair.svelte';
  import { hexToRgb, toHex, type Rgba8 } from '../../../lib/color';
  import { colorDraft, recentColors } from '../../../lib/colors/recent.svelte';
  import { resetColors, swapColors, tool } from '../stores/editor.svelte';

  function setFg(c: Rgba8): void {
    tool.foreground = toHex(c);
    tool.foregroundAlpha = c.a;
  }

  function setBg(c: Rgba8): void {
    tool.background = toHex(c);
    tool.backgroundAlpha = c.a;
  }
</script>

<ColorPair
  compact
  fg={{ ...hexToRgb(tool.foreground), a: tool.foregroundAlpha }}
  bg={{ ...hexToRgb(tool.background), a: tool.backgroundAlpha }}
  onFg={setFg}
  onBg={setBg}
  onSwap={swapColors}
  onReset={resetColors}
  recent={recentColors.image}
  onDraft={(d) => (colorDraft.image = d)}
/>
