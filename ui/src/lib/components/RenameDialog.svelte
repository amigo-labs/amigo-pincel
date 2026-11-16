<script lang="ts">
  // "Rename layer" dialog behind Layer ▸ Rename… in both modes (spec §9.2);
  // the layers panel also renames inline. Keys typed here never reach the
  // editor's window-level shortcuts.
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';

  interface Props {
    name: string;
    onRename: (name: string) => void;
    onCancel: () => void;
  }
  const { name, onRename, onCancel }: Props = $props();

  // svelte-ignore state_referenced_locally
  let draft = $state(name);
  let input: HTMLInputElement | undefined = $state();

  onMount(() => {
    input?.focus();
    input?.select();
  });

  function submit(): void {
    const next = draft.trim();
    if (next && next !== name) onRename(next);
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
  aria-label={t('layers.renameTitle')}
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
      <h2>{t('layers.renameTitle')}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <label class="flex flex-col gap-1 text-xs text-pc-muted">
        {t('layers.name')}
        <input bind:this={input} bind:value={draft} class="am-input w-full" spellcheck="false" />
      </label>
    </div>
    <footer>
      <button type="button" class="am-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="am-btn am-btn--primary">{t('layers.rename')}</button>
    </footer>
  </form>
</div>
