<script lang="ts">
  // The New size form's presets (docs/specs/new-document-form.md AC-02 –
  // AC-04): one button per common size; the one the fields hold is pressed.
  import { t } from '../i18n/index.svelte';
  import { SIZE_PRESETS } from './doc-size';
  import type { EditorMode } from './types';

  interface Props {
    mode: EditorMode;
    width: number | null;
    height: number | null;
    onPick: (width: number, height: number) => void;
  }
  const { mode, width, height, onPick }: Props = $props();
</script>

<div class="flex flex-wrap gap-1.5" role="group" aria-label={t('newDoc.presets')} data-size-presets="">
  {#each SIZE_PRESETS[mode] as [w, h] (`${w}x${h}`)}
    <button
      type="button"
      class="am-btn am-btn--sm am-btn--ghost tabular-nums"
      aria-pressed={width === w && height === h}
      onclick={() => onPick(w, h)}>{w} × {h}</button
    >
  {/each}
</div>
