<script lang="ts">
  // Latching Shift / Alt toggles for touch and pen (spec §9.4): Shift
  // constrains shapes (square, 45° lines) and adds to a selection, Alt
  // subtracts, both together intersect. A tap latches, another releases.
  import Icon from '../icons/Icon.svelte';
  import { t } from '../../i18n/index.svelte';
  import { modifiers } from '../../shell/modifiers.svelte';
</script>

<div
  class="absolute bottom-3 left-3 z-20 flex items-center gap-0.5 rounded-md border border-pc-border bg-pc-panel/90 p-0.5 shadow-lg backdrop-blur"
  role="group"
  aria-label={t('mods.label')}
>
  <button
    type="button"
    class="mod-btn btn-press"
    class:btn-on={modifiers.shift}
    aria-pressed={modifiers.shift}
    aria-label={t('mods.shift')}
    title={t('mods.shiftHint')}
    onclick={() => (modifiers.shift = !modifiers.shift)}
  >
    <Icon name="shift" size={18} />
  </button>
  <button
    type="button"
    class="mod-btn btn-press"
    class:btn-on={modifiers.alt}
    aria-pressed={modifiers.alt}
    aria-label={t('mods.alt')}
    title={t('mods.altHint')}
    onclick={() => (modifiers.alt = !modifiers.alt)}
  >
    <Icon name="option" size={18} />
  </button>
</div>

<style>
  .mod-btn {
    display: flex;
    height: 2.5rem;
    width: 2.5rem;
    align-items: center;
    justify-content: center;
    border-radius: 0.375rem;
  }
  .mod-btn:not(.btn-on) {
    color: var(--color-pc-muted);
  }
  /* An on toggle keeps the accent glyph of `.btn-on` (app.css). */
  .mod-btn:not(.btn-on):hover {
    color: var(--color-pc-text);
  }
</style>
