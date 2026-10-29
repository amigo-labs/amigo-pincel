<script lang="ts">
  // The prompts of the Image-mode Save flow (core/save-flow.ts): each one
  // answers the open `savePrompt`, which closes it.
  import { t } from '../../../../lib/i18n/index.svelte';
  import { savePrompt } from '../../stores/editor.svelte';
  import ExportDialog from './ExportDialog.svelte';
</script>

{#if savePrompt.value?.kind === 'jpeg'}
  {@const prompt = savePrompt.value}
  <ExportDialog
    title={t('save.jpegTitle')}
    applyLabel={t('menu.save')}
    onApply={(quality) => prompt.resolve(quality)}
    onClose={() => prompt.resolve(null)}
  />
{/if}
