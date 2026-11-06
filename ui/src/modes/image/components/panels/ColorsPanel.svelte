<script lang="ts">
  // Foreground / background colors (spec §16.6, §4.2), on the shared
  // ColorPair control, with the recent colors and the default palette
  // always in view below them (color-picker AC-25 – AC-29).
  import ColorPair from '../../../../lib/components/ColorPair.svelte';
  import SwatchGrid from '../../../../lib/components/color/SwatchGrid.svelte';
  import Panel from '../../../../lib/components/layout/Panel.svelte';
  import { DEFAULT_PALETTE, hexToRgb, toHex, type Rgba8 } from '../../../../lib/color';
  import { colorDraft, recentColors } from '../../../../lib/colors/recent.svelte';
  import { t } from '../../../../lib/i18n/index.svelte';
  import { tool, swapColors, resetColors } from '../../stores/editor.svelte';

  function setFg(c: Rgba8): void {
    tool.foreground = toHex(c);
    tool.foregroundAlpha = c.a;
  }

  function setBg(c: Rgba8): void {
    tool.background = toHex(c);
    tool.backgroundAlpha = c.a;
  }

  const pick = (c: Rgba8, target: 'fg' | 'bg'): void => (target === 'fg' ? setFg(c) : setBg(c));
</script>

<Panel>
  <div class="flex flex-col gap-3">
    <ColorPair
      fg={{ ...hexToRgb(tool.foreground), a: tool.foregroundAlpha }}
      bg={{ ...hexToRgb(tool.background), a: tool.backgroundAlpha }}
      onFg={setFg}
      onBg={setBg}
      onSwap={swapColors}
      onReset={resetColors}
      recent={recentColors.image}
      onDraft={(d) => (colorDraft.image = d)}
    />
    <SwatchGrid label={t('color.recent')} colors={recentColors.image} onPick={pick} />
    <SwatchGrid label={t('color.palette')} colors={DEFAULT_PALETTE} onPick={pick} />
  </div>
</Panel>
