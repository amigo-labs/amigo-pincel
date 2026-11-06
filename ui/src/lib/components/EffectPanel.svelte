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
  import { effectHost } from './layout/effect-host';

  // Modal dialog for one open effect / adjustment (effect-panel spec); the
  // editor frame floats it over the canvas area, or docks it to the bottom
  // edge on phones, and moves it by its title bar (`effectHost`).
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

  const host = effectHost();
  const floating = $derived(host?.floating ?? true);
  // Without a hosting frame the title bar is a plain header.
  const titleBar = host?.titleBar ?? (() => {});

  let root: HTMLElement;
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

  type NumberDef = Extract<ParamDef, { kind: 'number' }>;
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

  // A typed number (image-transform-handles-rotate-zoom AC-09): within the
  // range it applies while typing; on commit (Enter, leaving the field) a
  // value outside the range becomes the nearest bound and a non-numeric
  // entry gives way to the previous value.
  function typeNumber(p: NumberDef, field: HTMLInputElement, commit: boolean): void {
    const v = field.valueAsNumber;
    if (!Number.isFinite(v)) {
      if (commit) field.value = String(values[p.key] ?? p.default);
      return;
    }
    const bounded = Math.min(p.max, Math.max(p.min, v));
    if (!commit && bounded !== v) return;
    values[p.key] = bounded;
    if (commit) field.value = String(bounded);
  }

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

  const TABBABLE =
    'input:not([disabled]), select:not([disabled]), textarea:not([disabled]), button:not([disabled]), [tabindex="0"]';

  // A modal dialog (AC-21): Tab and Shift+Tab move among the panel's own
  // controls, title bar included, and wrap at the ends — from anywhere,
  // since the rest of the UI is inert.
  function onTab(e: KeyboardEvent) {
    if (e.key !== 'Tab' || e.ctrlKey || e.metaKey || e.altKey || e.defaultPrevented) return;
    const stops = Array.from(root.querySelectorAll<HTMLElement>(TABBABLE)).filter(
      (el) => el.getClientRects().length > 0,
    );
    if (stops.length === 0) return;
    e.preventDefault();
    const at = stops.indexOf(document.activeElement as HTMLElement);
    const step = e.shiftKey ? -1 : 1;
    const next =
      at === -1 ? (e.shiftKey ? stops.length - 1 : 0) : (at + step + stops.length) % stops.length;
    stops[next]?.focus();
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

<svelte:window onkeydown={onKeydown} onkeydowncapture={onTab} />

<div
  bind:this={root}
  class={[
    'flex min-h-0 flex-col border-pc-border bg-pc-panel text-sm text-pc-text',
    floating
      ? 'rounded-lg border shadow-[0_8px_30px_rgb(0_0_0/0.5)]'
      : 'rounded-t-lg border-t shadow-[0_-8px_30px_rgb(0_0_0/0.5)]',
  ]}
  role="dialog"
  aria-modal="true"
  aria-labelledby="{uid}-title"
>
  <form
    class="flex min-h-0 flex-col"
    onsubmit={(e) => {
      e.preventDefault();
      onApply(params);
    }}
  >
    <!-- Title bar: drags the panel, arrow keys move it (AC-18, AC-20). -->
    <div
      class="flex h-9 shrink-0 touch-none items-center gap-2 rounded-t-lg px-3 select-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-pc-accent"
      class:cursor-move={floating}
      role="group"
      aria-label={t('effectDlg.move')}
      {@attach titleBar}
    >
      <h2 id="{uid}-title" class="min-w-0 flex-1 truncate text-sm font-semibold">
        {effectLabel(def)}
      </h2>
      <span class="text-xs text-pc-muted">
        {hasSelection ? t('effectDlg.selection') : t('effectDlg.wholeLayer')}
      </span>
      <IconButton icon="close" label={t('common.close')} size="sm" onclick={onCancel} />
    </div>

    <!-- The controls scroll when the panel does not fit; title bar and
         buttons stay in view. -->
    <div class="flex min-h-0 flex-col gap-3 overflow-y-auto px-3 pt-1 pb-1" bind:this={body}>
      {#if def.params.length === 0}
        <p class="text-xs text-pc-muted">{t('effectDlg.noParams')}</p>
      {/if}

      {#each def.params as p (p.key)}
        {#if p.kind === 'number'}
          <!-- Slider first in the DOM (focus order, effect-panel AC-06), shown
               under the label / value row by the grid. -->
          <label class="grid grid-cols-[1fr_auto] items-center gap-x-2 gap-y-1.5 text-xs text-pc-text">
            <span class="col-start-1 row-start-1">{fxText(p.label)}</span>
            <input
              type="range"
              min={p.min}
              max={p.max}
              step={p.step}
              bind:value={values[p.key]}
              class="col-span-2 row-start-2 w-full cursor-pointer"
              aria-label={fxText(p.label)}
            />
            <input
              type="number"
              min={p.min}
              max={p.max}
              step={p.step}
              value={values[p.key]}
              oninput={(e) => typeNumber(p, e.currentTarget, false)}
              onchange={(e) => typeNumber(p, e.currentTarget, true)}
              class="opt-field col-start-2 row-start-1 w-16 text-right tabular-nums"
              aria-label={t('effectDlg.value', { label: fxText(p.label) })}
            />
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
                    class="opt-field w-16 text-right tabular-nums"
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
                class="opt-field w-16 text-right tabular-nums"
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
                  class="opt-field w-16 text-right tabular-nums"
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

    <footer class="flex shrink-0 items-center justify-between gap-2 px-3 pt-2 pb-3">
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
