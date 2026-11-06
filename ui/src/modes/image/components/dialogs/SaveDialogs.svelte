<script lang="ts">
  // The prompts of the Image-mode Save flow (core/save-flow.ts): each one
  // answers the open `savePrompt`, which closes it. A Save shows at most one
  // of them before the save picker (save-original-format AC-10, AC-17).
  import { onDestroy } from 'svelte';
  import { t } from '../../../../lib/i18n/index.svelte';
  import { savePrompt, type FlattenChoice } from '../../stores/editor.svelte';
  import ExportDialog from './ExportDialog.svelte';
  import Modal from './Modal.svelte';
  import SaveAsDialog from './SaveAsDialog.svelte';

  const DEFAULT_QUALITY = 90;

  // "Don't ask again" starts unchecked and the quality at its default on
  // every flatten prompt.
  let dontAsk = $state(false);
  let quality = $state(DEFAULT_QUALITY);

  // `savePrompt` is module state and outlives this editor: when a native
  // New / Open replaces the editor mid-prompt, cancel the abandoned Save so
  // the next editor neither shows the prompt nor writes through it.
  onDestroy(() => savePrompt.value?.resolve(null));

  function answerFlatten(
    resolve: (choice: FlattenChoice | null) => void,
    choice: FlattenChoice | null,
  ): void {
    dontAsk = false;
    quality = DEFAULT_QUALITY;
    resolve(choice);
  }
</script>

{#if savePrompt.value?.kind === 'jpeg'}
  {@const prompt = savePrompt.value}
  <ExportDialog
    title={t('save.jpegTitle')}
    applyLabel={t('menu.save')}
    onApply={(q) => prompt.resolve(q)}
    onClose={() => prompt.resolve(null)}
  />
{:else if savePrompt.value?.kind === 'flatten'}
  {@const prompt = savePrompt.value}
  <Modal
    title={t('save.flattenTitle')}
    applyLabel={t('save.flatten')}
    onApply={() =>
      answerFlatten(prompt.resolve, {
        action: 'flatten',
        dontAsk,
        ...(prompt.askQuality ? { quality } : {}),
      })}
    secondary={{
      label: t('save.asOra'),
      onClick: () => answerFlatten(prompt.resolve, { action: 'ora', dontAsk: false }),
    }}
    onClose={() => answerFlatten(prompt.resolve, null)}
  >
    <p class="mb-3 text-pc-muted">{t('save.flattenBody')}</p>
    {#if prompt.askQuality}
      <label class="mb-3 flex items-center justify-between gap-2">
        <span class="text-pc-muted">{t('export.quality')}</span>
        <input type="range" min="1" max="100" bind:value={quality} class="flex-1" />
        <span class="w-8 text-right tabular-nums">{quality}</span>
      </label>
    {/if}
    <label class="mb-4 flex items-center gap-2">
      <input type="checkbox" bind:checked={dontAsk} />
      <span>{t('save.dontAskAgain')}</span>
    </label>
  </Modal>
{:else if savePrompt.value?.kind === 'format'}
  {@const prompt = savePrompt.value}
  <SaveAsDialog
    preselect={prompt.preselect}
    unwritable={prompt.unwritable}
    quality={prompt.quality}
    confirmMerge={prompt.confirmMerge}
    onSave={(choice) => prompt.resolve(choice)}
    onClose={() => prompt.resolve(null)}
  />
{/if}
