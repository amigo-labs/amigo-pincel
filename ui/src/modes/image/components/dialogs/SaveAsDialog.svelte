<script lang="ts">
  // Save As (save-original-format AC-09, AC-10, AC-14 – AC-16): every format
  // Image mode writes, with the JPEG quality and the merge confirmation in
  // the same dialog, and a notice when the document's own format cannot be
  // written back.
  import { t } from '../../../../lib/i18n/index.svelte';
  import type { DocSaveFormat } from '../../core/document-file';
  import type { SaveAsChoice } from '../../stores/editor.svelte';
  import Modal from './Modal.svelte';

  interface Props {
    /** Format selected when the dialog opens. */
    preselect: DocSaveFormat;
    /** Label of the document's unwritable format (`GIF`), or `null`. */
    unwritable: string | null;
    /** JPEG quality the slider starts at. */
    quality: number;
    /** A format other than `.ora` must be confirmed with "Merge layers". */
    confirmMerge: boolean;
    onSave: (choice: SaveAsChoice) => void;
    onClose: () => void;
  }
  const { preselect, unwritable, quality: startQuality, confirmMerge, onSave, onClose }: Props =
    $props();

  const FORMATS: { id: DocSaveFormat; label: () => string }[] = [
    { id: 'ora', label: () => t('save.fmtOra') },
    { id: 'png', label: () => 'PNG' },
    { id: 'jpeg', label: () => 'JPEG' },
    { id: 'webp', label: () => t('menu.exportWebp') },
    { id: 'bmp', label: () => 'BMP' },
    { id: 'tiff', label: () => 'TIFF' },
  ];

  // The user's picks; until then the preselected format and quality.
  let picked = $state<DocSaveFormat | null>(null);
  const format = $derived(picked ?? preselect);
  let pickedQuality = $state<number | null>(null);
  const quality = $derived(pickedQuality ?? startQuality);
  // Kept while the dialog is open, whatever format is picked in between.
  let merge = $state(false);
  const needsMerge = $derived(confirmMerge && format !== 'ora');

  function onChange(e: Event & { currentTarget: HTMLSelectElement }): void {
    picked = FORMATS.find((f) => f.id === e.currentTarget.value)?.id ?? null;
  }
</script>

<Modal
  title={t('save.asTitle')}
  applyLabel={t('menu.save')}
  applyDisabled={needsMerge && !merge}
  onApply={() => onSave({ format, quality })}
  {onClose}
>
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
  {#if format === 'jpeg'}
    <label class="mb-4 flex items-center justify-between gap-2">
      <span class="text-pc-muted">{t('export.quality')}</span>
      <input
        type="range"
        min="1"
        max="100"
        class="flex-1"
        value={quality}
        oninput={(e) => (pickedQuality = e.currentTarget.valueAsNumber)}
      />
      <span class="w-8 text-right tabular-nums">{quality}</span>
    </label>
  {/if}
  {#if needsMerge}
    <label class="mb-4 flex items-start gap-2">
      <input type="checkbox" class="mt-0.5" bind:checked={merge} />
      <span>
        {t('save.mergeLayers')}
        <span class="block text-pc-muted">{t('save.mergeLayersHint')}</span>
      </span>
    </label>
  {/if}
</Modal>
