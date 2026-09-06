<script lang="ts">
  import { defaultValues, toParams, type EffectDef } from '../effects/catalog';

  // Parameter dialog for one effect / adjustment. Purely presentational:
  // the parent owns the wasm `Document`, runs `previewEffect` on every
  // `onPreview(params)` and commits via `onApply(params)`. `onCancel`
  // restores the un-previewed composite. Parameters are edited as a flat
  // value map keyed by `ParamDef.key`; `toParams` orders them for the wire.
  let {
    def,
    hasSelection = false,
    onPreview,
    onApply,
    onCancel,
  }: {
    def: EffectDef;
    hasSelection?: boolean;
    onPreview: (params: number[]) => void;
    onApply: (params: number[]) => void;
    onCancel: () => void;
  } = $props();

  // The parent remounts this dialog per effect (`{#key def.id}`), so the
  // initial snapshot of `def` is the only one this instance ever sees.
  // svelte-ignore state_referenced_locally
  let values = $state<Record<string, number>>(defaultValues(def));
  let preview = $state(true);

  const params = $derived(toParams(def, values));

  // Live preview: re-run whenever a value changes while preview is on.
  // Coalesced through rAF so a slider drag renders at most once a frame.
  let rafId = 0;
  $effect(() => {
    const p = params;
    const on = preview;
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      rafId = 0;
      if (on) onPreview(p);
      else onCancel_previewOnly();
    });
    return () => {
      if (rafId) cancelAnimationFrame(rafId);
    };
  });

  // Turning preview off restores the composite without closing.
  let onCancel_previewOnly = () => onPreview([]);

  function reset() {
    values = defaultValues(def);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'Enter' && !(e.target instanceof HTMLTextAreaElement)) {
      e.preventDefault();
      onApply(params);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="pointer-events-none fixed inset-0 z-20 flex items-start justify-end p-4"
  role="dialog"
  aria-modal="false"
  aria-labelledby="effect-title"
>
  <form
    class="pointer-events-auto flex w-80 max-h-full flex-col gap-3 overflow-y-auto rounded border border-neutral-700 bg-neutral-900 p-4 text-sm text-neutral-100 shadow-2xl"
    onsubmit={(e) => {
      e.preventDefault();
      onApply(params);
    }}
  >
    <header class="flex items-baseline justify-between">
      <h2 id="effect-title" class="text-base font-semibold">{def.label}</h2>
      <span class="text-xs text-neutral-400">
        {hasSelection ? 'selection' : 'whole layer'}
      </span>
    </header>

    {#if def.params.length === 0}
      <p class="text-xs text-neutral-400">This effect has no parameters.</p>
    {/if}

    {#each def.params as p (p.key)}
      {#if p.kind === 'number'}
        <label class="flex flex-col gap-1 text-xs text-neutral-300">
          <span class="flex justify-between">
            <span>{p.label}</span>
            <span class="tabular-nums text-neutral-500">{values[p.key]}</span>
          </span>
          <span class="flex items-center gap-2">
            <input
              type="range"
              min={p.min}
              max={p.max}
              step={p.step}
              bind:value={values[p.key]}
              class="flex-1 cursor-pointer"
              aria-label={p.label}
            />
            <input
              type="number"
              min={p.min}
              max={p.max}
              step={p.step}
              bind:value={values[p.key]}
              class="w-16 rounded border border-neutral-700 bg-neutral-950 px-1 py-0.5 text-right tabular-nums"
              aria-label="{p.label} value"
            />
          </span>
        </label>
      {:else if p.kind === 'bool'}
        <label class="flex items-center gap-2 text-xs text-neutral-300">
          <input
            type="checkbox"
            checked={values[p.key] !== 0}
            onchange={(e) => (values[p.key] = e.currentTarget.checked ? 1 : 0)}
          />
          <span>{p.label}</span>
        </label>
      {:else}
        <label class="flex items-center justify-between gap-2 text-xs text-neutral-300">
          <span>{p.label}</span>
          <select
            bind:value={values[p.key]}
            class="rounded border border-neutral-700 bg-neutral-950 px-1 py-0.5"
          >
            {#each p.options as opt, i (opt)}
              <option value={i}>{opt}</option>
            {/each}
          </select>
        </label>
      {/if}
    {/each}

    <footer class="mt-1 flex items-center justify-between gap-2">
      <label class="flex items-center gap-1 text-xs text-neutral-400">
        <input type="checkbox" bind:checked={preview} />
        <span>Preview</span>
      </label>
      <span class="flex gap-2">
        <button type="button" class="panel-btn" onclick={reset} disabled={def.params.length === 0}>
          Reset
        </button>
        <button type="button" class="panel-btn" onclick={onCancel}>Cancel</button>
        <button type="submit" class="panel-btn panel-btn-primary">OK</button>
      </span>
    </footer>
  </form>
</div>
