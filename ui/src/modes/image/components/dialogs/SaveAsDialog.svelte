<script lang="ts">
  // Save As (save-original-format AC-09, AC-10, AC-14): every format Image
  // mode writes, with a notice when the document's own format cannot be
  // written back.
  import { t } from '../../../../lib/i18n/index.svelte';
  import type { DocSaveFormat } from '../../core/document-file';
  import Modal from './Modal.svelte';

  interface Props {
    /** Format selected when the dialog opens. */
    preselect: DocSaveFormat;
    /** Label of the document's unwritable format (`GIF`), or `null`. */
    unwritable: string | null;
    onSave: (format: DocSaveFormat) => void;
    onClose: () => void;
  }
  const { preselect, unwritable, onSave, onClose }: Props = $props();

  const FORMATS: { id: DocSaveFormat; label: () => string }[] = [
    { id: 'ora', label: () => t('save.fmtOra') },
    { id: 'png', label: () => 'PNG' },
    { id: 'jpeg', label: () => 'JPEG' },
    { id: 'webp', label: () => t('menu.exportWebp') },
    { id: 'bmp', label: () => 'BMP' },
    { id: 'tiff', label: () => 'TIFF' },
  ];

  // The user's pick; until then the preselected format.
  let picked = $state<DocSaveFormat | null>(null);
  const format = $derived(picked ?? preselect);

  function onChange(e: Event & { currentTarget: HTMLSelectElement }): void {
    picked = FORMATS.find((f) => f.id === e.currentTarget.value)?.id ?? null;
  }
</script>

<Modal title={t('save.asTitle')} applyLabel={t('menu.save')} onApply={() => onSave(format)} {onClose}>
  {#if unwritable !== null}
    <p class="mb-3 text-pc-muted">{t('save.noEncoder', { format: unwritable })}</p>
  {/if}
  <label class="mb-4 flex items-center justify-between gap-2">
    <span class="text-pc-muted">{t('save.format')}</span>
    <select
      aria-label={t('save.format')}
      class="flex-1 rounded border border-pc-border bg-pc-raised px-2 py-1"
      value={format}
      onchange={onChange}
    >
      {#each FORMATS as f (f.id)}
        <option value={f.id}>{f.label()}</option>
      {/each}
    </select>
  </label>
</Modal>
