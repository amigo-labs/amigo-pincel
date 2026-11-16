<script lang="ts">
  // "By how many pixels?" for Select ▸ Expand / Contract / Feather, in both
  // modes (spec §9.2). The cores take a radius; before this the menus were
  // wired to a fixed 1 px.
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';

  interface Props {
    title: string;
    value: number;
    max?: number;
    onApply: (radius: number) => void;
    onCancel: () => void;
  }
  const { title, value, max = 200, onApply, onCancel }: Props = $props();

  // svelte-ignore state_referenced_locally
  let draft = $state(String(value));
  let input: HTMLInputElement | undefined = $state();

  onMount(() => {
    input?.focus();
    input?.select();
  });

  function submit(): void {
    const radius = Math.round(Number(draft));
    if (Number.isFinite(radius) && radius >= 1 && radius <= max) onApply(radius);
    else onCancel();
  }

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={title}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <form
    class="am-dialog"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <header>
      <h2>{title}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <label class="flex flex-col gap-1 text-xs text-pc-muted">
        {t('select.radius')}
        <input
          bind:this={input}
          bind:value={draft}
          type="number"
          min="1"
          {max}
          step="1"
          class="am-input w-full"
        />
      </label>
    </div>
    <footer>
      <button type="button" class="am-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="am-btn am-btn--primary">{t('common.apply')}</button>
    </footer>
  </form>
</div>
