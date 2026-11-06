<script lang="ts">
  // The size input Scale Image and Resize Canvas share
  // (docs/specs/image-resize-autocrop.md AC-01 – AC-08): a percentage, or an
  // absolute size in a unit at a resolution, with an aspect lock and a
  // readout of the resulting pixel size. The arithmetic is size-input.ts.
  import { locale, t } from '../../../../lib/i18n/index.svelte';
  import type { MessageKey } from '../../../../lib/i18n/index.svelte';
  import {
    changeUnit,
    editField,
    initialSize,
    invalidFields,
    resultSize,
    usesResolution,
    type SizeDoc,
    type SizeField,
    type SizeState,
    type SizeUnit,
  } from '../../size-input';

  interface Props {
    doc: SizeDoc;
    /** Called with the size Apply produces, or `null` while a used field
     *  is invalid, on every change. */
    onSize: (size: [number, number] | null) => void;
  }
  // `doc` is fixed for the dialog's lifetime: it opens fresh every time.
  const { doc, onSize }: Props = $props();

  const UNITS: readonly { id: SizeUnit; label: MessageKey }[] = [
    { id: 'px', label: 'resize.unitPixels' },
    { id: 'percent', label: 'resize.unitPercent' },
    { id: 'in', label: 'resize.unitInches' },
    { id: 'cm', label: 'resize.unitCentimeters' },
  ];

  function fresh(): SizeState {
    return initialSize(doc, locale());
  }
  let s = $state<SizeState>(fresh());
  const invalid = $derived(new Set(invalidFields(s)));
  const result = $derived(resultSize(s, doc));
  $effect(() => {
    onSize(result);
  });

  function input(field: SizeField, e: Event): void {
    const text = (e.currentTarget as HTMLInputElement).value;
    s = editField(s, field, text, doc, locale());
  }

  function hint(field: SizeField): string | undefined {
    if (!invalid.has(field)) return undefined;
    if (field === 'resolution') return t('resize.invalidResolution');
    if (field !== 'percent' && s.unit === 'px') return t('resize.invalidPixels');
    return t('resize.invalidValue');
  }

  const fieldClass =
    'w-24 rounded border bg-pc-bg px-2 py-1 aria-[invalid=true]:border-red-500 border-pc-border';
</script>

<fieldset class="mb-2">
  <label class="mb-1 flex items-center gap-2">
    <input
      type="radio"
      name="size-mode"
      checked={s.mode === 'percent'}
      onchange={() => (s = { ...s, mode: 'percent' })}
    />
    <span>{t('resize.byPercentage')}</span>
  </label>
  <label class="mb-2 ml-6 flex items-center justify-between gap-2">
    <span class="text-pc-muted">{t('resize.percentage')}</span>
    <span class="flex items-center gap-1">
      <input
        type="text"
        inputmode="decimal"
        value={s.percent.text}
        aria-label={t('resize.percentage')}
        oninput={(e) => input('percent', e)}
        aria-invalid={s.mode === 'percent' && invalid.has('percent')}
        title={s.mode === 'percent' ? hint('percent') : undefined}
        class={fieldClass}
      />
      <span class="w-4 text-pc-muted" aria-hidden="true">%</span>
    </span>
  </label>

  <label class="mb-1 flex items-center gap-2">
    <input
      type="radio"
      name="size-mode"
      checked={s.mode === 'absolute'}
      onchange={() => (s = { ...s, mode: 'absolute' })}
    />
    <span>{t('resize.byAbsolute')}</span>
  </label>
  <div class="ml-6">
    <label class="mb-2 flex items-center gap-2">
      <input
        type="checkbox"
        checked={s.keepAspect}
        onchange={(e) => (s = { ...s, keepAspect: e.currentTarget.checked })}
        disabled={s.mode === 'percent'}
      />
      <span class="text-pc-muted">{t('resize.maintainAspect')}</span>
    </label>
    {#each ['width', 'height'] as const as edge (edge)}
      <label class="mb-2 flex items-center justify-between gap-2">
        <span class="text-pc-muted">{t(edge === 'width' ? 'common.width' : 'common.height')}</span>
        <input
          type="text"
          inputmode="decimal"
          value={s[edge].text}
          oninput={(e) => input(edge, e)}
          aria-invalid={s.mode === 'absolute' && invalid.has(edge)}
          title={s.mode === 'absolute' ? hint(edge) : undefined}
          class={fieldClass}
        />
      </label>
    {/each}
    <label class="mb-2 flex items-center justify-between gap-2">
      <span class="text-pc-muted">{t('resize.unit')}</span>
      <select
        value={s.unit}
        onchange={(e) =>
          (s = changeUnit(s, (e.currentTarget as HTMLSelectElement).value as SizeUnit, doc, locale()))}
        class="rounded border border-pc-border bg-pc-raised px-2 py-1"
      >
        {#each UNITS as u (u.id)}
          <option value={u.id}>{t(u.label)}</option>
        {/each}
      </select>
    </label>
    <label class="mb-2 flex items-center justify-between gap-2">
      <span class="text-pc-muted">{t('resize.resolution')}</span>
      <span class="flex items-center gap-1">
        <input
          type="text"
          inputmode="decimal"
          value={s.resolution.text}
          oninput={(e) => input('resolution', e)}
          aria-invalid={s.mode === 'absolute' && usesResolution(s.unit) && invalid.has('resolution')}
          title={hint('resolution')}
          class={fieldClass}
        />
        <span class="text-xs text-pc-muted">{t('resize.pixelsPerInch')}</span>
      </span>
    </label>
  </div>
</fieldset>

<p class="mb-3 flex justify-between gap-2" aria-live="polite">
  <span class="text-pc-muted">{t('resize.newSize')}</span>
  <span data-testid="size-result"
    >{result ? t('resize.sizePx', { width: result[0], height: result[1] }) : t('resize.noSize')}</span
  >
</p>
