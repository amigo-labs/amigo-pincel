<script lang="ts">
  // "By how many pixels?" for Select ▸ Expand / Contract / Feather, in both
  // modes (spec §9.2). The cores take a radius; before this the menus were
  // wired to a fixed 1 px.
  import { onMount } from 'svelte';
  import Dialog from './Dialog.svelte';
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

  // Keys typed here never reach the editor's window-level shortcuts; the
  // shell inside has already handled Escape / Enter / Tab by then.
</script>

<Dialog {title} applyLabel={t('common.apply')} onApply={submit} onClose={onCancel}>
  <div class="flex flex-col gap-3">
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
</Dialog>
