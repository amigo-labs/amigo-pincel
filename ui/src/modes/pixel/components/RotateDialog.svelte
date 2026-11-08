<script lang="ts">
  // Layer ▸ Rotate… (docs/specs/rotsprite.md): angle as number plus
  // slider, method choice, live preview. Presentational: the parent draws
  // the preview and runs the rotation. No backdrop tint, so the canvas
  // preview stays visible; the transparent backdrop keeps tools off it.
  import { t } from '../../../lib/i18n/index.svelte';
  import {
    MAX_ANGLE,
    MIN_ANGLE,
    clampAngle,
    rotspriteTooLarge,
    type RotateDialogState,
    type RotateMethod,
  } from '../rotate-ops';

  let {
    input,
    onPreview,
    onApply,
    onCancel,
  }: {
    input: RotateDialogState;
    onPreview: (angle: number, method: RotateMethod) => void;
    onApply: (angle: number, method: RotateMethod) => void;
    onCancel: () => void;
  } = $props();

  let angle = $state(0);
  let method = $state<RotateMethod>('rotsprite');
  const tooLarge = $derived(method === 'rotsprite' && rotspriteTooLarge(input));

  $effect(() => onPreview(angle, method));

  // Typed values outside the range snap to the nearest bound (AC-11);
  // a lone "-" or an empty field waits for more input.
  function onAngleInput(field: HTMLInputElement) {
    const raw = field.value.trim();
    if (raw === '' || raw === '-') return;
    const v = Number(raw);
    if (!Number.isFinite(v)) return;
    angle = clampAngle(v);
    if (String(angle) !== raw) field.value = String(angle);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="fixed inset-0 z-20 flex items-start justify-end p-4 pt-16"
  role="dialog"
  aria-modal="true"
  aria-label={t('pxRotate.title')}
>
  <form
    class="flex w-72 flex-col gap-3 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      if (!tooLarge) onApply(angle, method);
    }}
  >
    <h2 class="text-base font-semibold">{t('pxRotate.title')}</h2>
    <label class="flex items-center justify-between gap-2 text-xs">
      <span>{t('pxRotate.angle')}</span>
      <input
        type="number"
        min={MIN_ANGLE}
        max={MAX_ANGLE}
        step="1"
        value={angle}
        oninput={(e) => onAngleInput(e.currentTarget)}
        onchange={(e) => (e.currentTarget.value = String(angle))}
        class="w-20 rounded border border-pc-border bg-pc-bg px-2 py-1 tabular-nums"
        aria-label={t('pxRotate.angle')}
      />
    </label>
    <input
      type="range"
      min={MIN_ANGLE}
      max={MAX_ANGLE}
      step="1"
      bind:value={angle}
      aria-label={t('pxRotate.angleSlider')}
    />
    <label class="flex items-center justify-between gap-2 text-xs">
      <span>{t('pxRotate.method')}</span>
      <select
        bind:value={method}
        class="rounded border border-pc-border bg-pc-bg px-1 py-0.5"
        aria-label={t('pxRotate.method')}
      >
        <option value="nearest">{t('pxRotate.nearest')}</option>
        <option value="rotsprite">{t('pxRotate.rotsprite')}</option>
      </select>
    </label>
    {#if tooLarge}
      <p role="alert" class="text-xs text-pc-accent-text">
        {t('pxRotate.tooLarge', { max: input.max, w: input.width, h: input.height })}
      </p>
    {/if}
    <footer class="mt-1 flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary" disabled={tooLarge}>
        {t('common.apply')}
      </button>
    </footer>
  </form>
</div>
