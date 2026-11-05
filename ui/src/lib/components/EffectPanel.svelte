<script lang="ts">
  import { onMount } from 'svelte';
  import { toParams, type ParamDef } from '../effects/catalog';
  import { canvasPick } from '../effects/canvas-pick.svelte';
  import { effectLabel, fxText } from '../effects/labels';
  import type { EffectSession } from '../effects/session.svelte';
  import { t } from '../i18n/index.svelte';
  import AngleDial from './AngleDial.svelte';
  import EffectColorField from './EffectColorField.svelte';
  import IconButton from './layout/IconButton.svelte';

  // Docked panel for one open effect / adjustment (effect-panel spec); the
  // editor frame shows it on top of the sidebar, drawer or bottom sheet.
  // Purely presentational: the parent owns the wasm `Document`, runs
  // `previewEffect` on every `onPreview(params)`, restores the
  // un-previewed composite on `onPreview(null)` (Preview off) and commits
  // via `onApply(params)`. `onCancel` restores the un-previewed composite too.
  // The values live in the editor's `EffectSession`, so they survive the
  // frame re-mounting this panel in another layout (AC-14); `toParams`
  // orders them for the wire.
  let {
    session,
    hasSelection = false,
    onPreview,
    onApply,
    onCancel,
  }: {
    session: EffectSession;
    hasSelection?: boolean;
    /** `null` turns the preview off: show the image without the effect. */
    onPreview: (params: number[] | null) => void;
    onApply: (params: number[]) => void;
    onCancel: () => void;
  } = $props();

  // The parent remounts this panel per session (`{#key session.token}`),
  // so the first session is the only one this instance ever sees.
  // svelte-ignore state_referenced_locally
  const def = session.def;
  const values = $derived(session.values);
  const params = $derived(toParams(def, values));
  const uid = $props.id();

  let body: HTMLElement;
  let okButton: HTMLButtonElement;

  // Focus the first control, or OK when there is none (AC-06).
  onMount(() => {
    const first = body.querySelector<HTMLElement>(
      'input, select, textarea, button, [tabindex="0"]',
    );
    (first ?? okButton).focus();
  });

  // Live preview: re-run whenever a value changes while preview is on.
  // Coalesced through rAF so a slider drag renders at most once a frame.
  let rafId = 0;
  $effect(() => {
    const p = params;
    const on = session.preview;
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      rafId = 0;
      // `null`, not `[]`: a parameterless effect previews with `[]`.
      onPreview(on ? p : null);
    });
    return () => {
      if (rafId) cancelAnimationFrame(rafId);
    };
  });

  type PointDef = Extract<ParamDef, { kind: 'point' }>;
  type PairDef = Extract<ParamDef, { kind: 'pair' }>;
  type SeedDef = Extract<ParamDef, { kind: 'seed' }>;

  // Pick on canvas (AC-11): the point parameter being picked, if any. The
  // canvas routes its presses here through `canvasPick` while it is set.
  let picking = $state<string | null>(null);

  const pct = (f: number): number => Math.round(Math.min(100, Math.max(0, f * 100)) * 10) / 10;

  function startPicking(p: PointDef): void {
    picking = p.key;
    canvasPick.handler = (fx, fy, phase) => {
      values[p.xKey] = pct(fx);
      values[p.yKey] = pct(fy);
      if (phase === 'up') stopPicking();
    };
  }

  function stopPicking(): void {
    picking = null;
    canvasPick.handler = null;
  }

  // Closing the dialog (OK, Cancel, Escape) ends picking with it.
  $effect(() => stopPicking);

  // Linked pairs (effect-catalog-v2 AC-13): `session.lastEdited` keeps the
  // side edited last, so re-linking takes its value.
  function setPair(p: PairDef, side: 'first' | 'second', v: number): void {
    if (!Number.isFinite(v)) return;
    session.lastEdited[p.key] = side;
    values[p[side].key] = v;
    if (values[p.linkKey] !== 0) values[p[side === 'first' ? 'second' : 'first'].key] = v;
  }

  function toggleLink(p: PairDef): void {
    const linked = values[p.linkKey] !== 0;
    values[p.linkKey] = linked ? 0 : 1;
    if (!linked) {
      const v = values[p[session.lastEdited[p.key] ?? 'first'].key] ?? p.default;
      values[p.first.key] = v;
      values[p.second.key] = v;
    }
  }

  /** A new seed in range, different from the current one (AC-12). */
  function reseed(p: SeedDef): void {
    const current = values[p.key];
    let next = current;
    while (next === current) next = p.min + Math.floor(Math.random() * (p.max - p.min + 1));
    values[p.key] = next ?? p.default;
  }

  function reset() {
    stopPicking();
    session.reset();
  }

  // Controls Enter activates on its own: a menu entry, Cancel, ×, Reset
  // keep their meaning instead of turning into OK (effect-panel AC-04,
  // AC-05). Multi-line fields take Enter as a line break.
  const OWNS_ENTER =
    'button, a[href], summary, textarea, [role="button"], [role="link"], [role^="menuitem"], [role="tab"], [role="option"]';

  function ownsEnter(target: EventTarget | null): boolean {
    return target instanceof Element && target.closest(OWNS_ENTER) !== null;
  }

  function onKeydown(e: KeyboardEvent) {
    // Taken by another handler already (an open menu closes on Escape).
    if (e.defaultPrevented) return;
    if (e.key === 'Escape' && picking !== null) {
      // Escape ends picking only; the dialog stays open.
      e.preventDefault();
      stopPicking();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'Enter' && (picking !== null || !ownsEnter(e.target))) {
      // Enter while picking applies even from the armed Pick button
      // (effect-catalog-v2 AC-11).
      e.preventDefault();
      onApply(params);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="flex flex-col text-sm text-pc-text"
  role="dialog"
  aria-modal="false"
  aria-labelledby="{uid}-title"
>
  <form
    class="flex flex-col gap-3 px-3 pt-1 pb-3"
    onsubmit={(e) => {
      e.preventDefault();
      onApply(params);
    }}
  >
    <header class="flex h-8 items-center gap-2">
      <h2 id="{uid}-title" class="min-w-0 flex-1 truncate text-sm font-semibold">
        {effectLabel(def)}
      </h2>
      <span class="text-xs text-pc-muted">
        {hasSelection ? t('effectDlg.selection') : t('effectDlg.wholeLayer')}
      </span>
      <IconButton icon="close" label={t('common.close')} size="sm" onclick={onCancel} />
    </header>

    <div class="flex flex-col gap-3" bind:this={body}>
      {#if def.params.length === 0}
        <p class="text-xs text-pc-muted">{t('effectDlg.noParams')}</p>
      {/if}

      {#each def.params as p (p.key)}
        {#if p.kind === 'number'}
          <label class="flex flex-col gap-1 text-xs text-pc-text">
            <span class="flex justify-between">
              <span>{fxText(p.label)}</span>
              <span class="tabular-nums text-pc-faint">{values[p.key]}</span>
            </span>
            <span class="flex items-center gap-2">
              <input
                type="range"
                min={p.min}
                max={p.max}
                step={p.step}
                bind:value={values[p.key]}
                class="flex-1 cursor-pointer"
                aria-label={fxText(p.label)}
              />
              <input
                type="number"
                min={p.min}
                max={p.max}
                step={p.step}
                bind:value={values[p.key]}
                class="w-16 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-right tabular-nums"
                aria-label={t('effectDlg.value', { label: fxText(p.label) })}
              />
            </span>
          </label>
        {:else if p.kind === 'bool'}
          <label class="flex items-center gap-2 text-xs text-pc-text">
            <input
              type="checkbox"
              checked={values[p.key] !== 0}
              onchange={(e) => (values[p.key] = e.currentTarget.checked ? 1 : 0)}
            />
            <span>{fxText(p.label)}</span>
          </label>
        {:else if p.kind === 'angle'}
          <div class="flex flex-col gap-1 text-xs text-pc-text">
            <span>{fxText(p.label)}</span>
            <AngleDial
              value={values[p.key] ?? p.default}
              label={fxText(p.label)}
              onchange={(v) => (values[p.key] = v)}
            />
          </div>
        {:else if p.kind === 'point'}
          <fieldset class="flex flex-col gap-1 text-xs text-pc-text">
            <legend>{fxText(p.label)}</legend>
            <span class="flex items-center gap-2">
              {#each [{ key: p.xKey, label: p.xLabel }, { key: p.yKey, label: p.yLabel }] as axis (axis.key)}
                <label class="flex items-center gap-1">
                  <span>{fxText(axis.label)}</span>
                  <input
                    type="number"
                    min="0"
                    max="100"
                    step="0.1"
                    bind:value={values[axis.key]}
                    class="w-16 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-right tabular-nums"
                    aria-label={t('effectDlg.percent', { label: fxText(axis.label) })}
                  />
                  <span class="text-pc-faint">%</span>
                </label>
              {/each}
            </span>
            <button
              type="button"
              class="panel-btn self-start"
              aria-pressed={picking === p.key}
              onclick={() => (picking === p.key ? stopPicking() : startPicking(p))}
            >
              {t('effectDlg.pick')}
            </button>
            {#if picking === p.key}
              <span class="text-pc-muted" role="status">
                {t('effectDlg.picking', { label: fxText(p.label) })}
              </span>
            {/if}
          </fieldset>
        {:else if p.kind === 'color'}
          <EffectColorField
            value={values[p.key] ?? 0}
            label={fxText(p.label)}
            onchange={(v) => (values[p.key] = v)}
          />
        {:else if p.kind === 'seed'}
          <div class="flex items-center justify-between gap-2 text-xs text-pc-text">
            <span>{fxText(p.label)}</span>
            <span class="flex items-center gap-2">
              <input
                type="number"
                min={p.min}
                max={p.max}
                step="1"
                bind:value={values[p.key]}
                class="w-16 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-right tabular-nums"
                aria-label={t('effectDlg.value', { label: fxText(p.label) })}
              />
              <button type="button" class="panel-btn" onclick={() => reseed(p)}>
                {t('effectDlg.reseed')}
              </button>
            </span>
          </div>
        {:else if p.kind === 'pair'}
          <fieldset class="flex flex-col gap-1 text-xs text-pc-text">
            <legend>{fxText(p.label)}</legend>
            {#each ['first', 'second'] as const as side (side)}
              {@const part = p[side]}
              <span class="flex items-center gap-2">
                <span class="w-12">{fxText(part.label)}</span>
                <input
                  type="range"
                  min={p.min}
                  max={p.max}
                  step={p.step}
                  value={values[part.key]}
                  oninput={(e) => setPair(p, side, e.currentTarget.valueAsNumber)}
                  class="flex-1 cursor-pointer"
                  aria-label={fxText(part.label)}
                />
                <input
                  type="number"
                  min={p.min}
                  max={p.max}
                  step={p.step}
                  value={values[part.key]}
                  oninput={(e) => setPair(p, side, e.currentTarget.valueAsNumber)}
                  class="w-16 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-right tabular-nums"
                  aria-label={t('effectDlg.value', { label: fxText(part.label) })}
                />
              </span>
            {/each}
            <label class="flex items-center gap-2">
              <input
                type="checkbox"
                checked={values[p.linkKey] !== 0}
                onchange={() => toggleLink(p)}
              />
              <span>
                {t('effectDlg.link', { a: fxText(p.first.label), b: fxText(p.second.label) })}
              </span>
            </label>
          </fieldset>
        {:else}
          <label class="flex items-center justify-between gap-2 text-xs text-pc-text">
            <span>{fxText(p.label)}</span>
            <select
              value={values[p.key]}
              onchange={(e) => (values[p.key] = Number(e.currentTarget.value))}
              class="rounded border border-pc-border bg-pc-bg px-1 py-0.5"
              aria-label={fxText(p.label)}
            >
              {#each p.options as opt, i (opt)}
                <option value={i}>{fxText(opt)}</option>
              {/each}
            </select>
          </label>
        {/if}
      {/each}
    </div>

    <footer class="mt-1 flex items-center justify-between gap-2">
      <label class="flex items-center gap-1 text-xs text-pc-muted">
        <input type="checkbox" bind:checked={session.preview} />
        <span>{t('effectDlg.preview')}</span>
      </label>
      <span class="flex gap-2">
        <button type="button" class="panel-btn" onclick={reset} disabled={def.params.length === 0}>
          {t('effectDlg.reset')}
        </button>
        <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
        <button type="submit" class="panel-btn panel-btn-primary" bind:this={okButton}>
          {t('common.ok')}
        </button>
      </span>
    </footer>
  </form>
</div>
