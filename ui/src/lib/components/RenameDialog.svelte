<script lang="ts">
  // "Rename layer" dialog behind Layer ▸ Rename… in both modes (spec §9.2);
  // the layers panel also renames inline. Keys typed here never reach the
  // editor's window-level shortcuts.
  import { onMount } from 'svelte';
  import Dialog from './Dialog.svelte';
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

  // Keys typed here never reach the editor's window-level shortcuts; the
  // shell inside has already handled Escape / Enter / Tab by then.
</script>

<Dialog
  title={t('layers.renameTitle')}
  applyLabel={t('layers.rename')}
  onApply={submit}
  onClose={onCancel}
>
  <div class="flex flex-col gap-3">
    <label class="flex flex-col gap-1 text-xs text-pc-muted">
      {t('layers.name')}
      <input bind:this={input} bind:value={draft} class="am-input w-full" spellcheck="false" />
    </label>
  </div>
</Dialog>
