<script lang="ts">
  // The prompts of the Image-mode Save flow (core/save-flow.ts): each one
  // answers the open `savePrompt`, which closes it.
  import { onDestroy } from 'svelte';
  import { t } from '../../../../lib/i18n/index.svelte';
  import { savePrompt, type FlattenChoice } from '../../stores/editor.svelte';
  import ExportDialog from './ExportDialog.svelte';
  import Modal from './Modal.svelte';
  import SaveAsDialog from './SaveAsDialog.svelte';

  // "Don't ask again" starts unchecked on every flatten prompt.
  let dontAsk = $state(false);

  // `savePrompt` is module state and outlives this editor: when a native
  // New / Open replaces the editor mid-prompt, cancel the abandoned Save so
  // the next editor neither shows the prompt nor writes through it.
  onDestroy(() => savePrompt.value?.resolve(null));

  function answerFlatten(
    resolve: (choice: FlattenChoice | null) => void,
    choice: FlattenChoice | null,
  ): void {
    dontAsk = false;
    resolve(choice);
  }
</script>

{#if savePrompt.value?.kind === 'jpeg'}
  {@const prompt = savePrompt.value}
  <ExportDialog
    title={t('save.jpegTitle')}
    applyLabel={t('menu.save')}
    onApply={(quality) => prompt.resolve(quality)}
    onClose={() => prompt.resolve(null)}
  />
{:else if savePrompt.value?.kind === 'flatten'}
  {@const prompt = savePrompt.value}
  <Modal
    title={t('save.flattenTitle')}
    applyLabel={t('save.flatten')}
    onApply={() => answerFlatten(prompt.resolve, { action: 'flatten', dontAsk })}
    secondary={{
      label: t('save.asOra'),
      onClick: () => answerFlatten(prompt.resolve, { action: 'ora', dontAsk: false }),
    }}
    onClose={() => answerFlatten(prompt.resolve, null)}
  >
    <p class="mb-3 text-pc-muted">{t('save.flattenBody')}</p>
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
    onSave={(format) => prompt.resolve(format)}
    onClose={() => prompt.resolve(null)}
  />
{/if}
