<script lang="ts">
  import type { Document } from '../core';

  // Layers panel (M13.3). The wasm `Document` is the source of truth;
  // the panel reads the layer stack through the M8.7 / M13.2 surface
  // (`layerCount`, `layerIdAt`, `layerName`, `layerKind`, `layerVisible`)
  // and reorders via `moveLayerUp` / `moveLayerDown`. `rev` is the
  // parent-managed change counter (bumped on new / open / undo / redo /
  // any edit) that the `$derived.by` block reads so the list reflects
  // opaque wasm mutations.
  //
  // `activeLayerId` is parent-owned: clicking a row calls `onActivate`,
  // and the highlight reads back the parent's choice. Painting still
  // targets the auto-picked image layer until the paint surface learns
  // the active layer (M13.3b) — the selection is wired here first.
  let {
    doc,
    rev = 0,
    activeLayerId = null,
    onChange,
    onActivate,
    onToggleVisible,
  }: {
    doc: Document | null;
    rev?: number;
    activeLayerId?: number | null;
    onChange?: () => void;
    onActivate?: (layerId: number) => void;
    onToggleVisible?: (layerId: number, visible: boolean) => void;
  } = $props();

  type LayerRow = {
    id: number;
    name: string;
    kind: string;
    visible: boolean;
  };

  // Bottom-to-top z-order from the document, then reversed so the panel
  // lists the top-most layer first (matching every raster editor).
  const layers = $derived.by<LayerRow[]>(() => {
    void rev;
    if (!doc) return [];
    const list: LayerRow[] = [];
    const count = doc.layerCount;
    for (let i = 0; i < count; i += 1) {
      let id: number;
      try {
        id = doc.layerIdAt(i);
      } catch {
        continue;
      }
      list.push({
        id,
        name: doc.layerName(id),
        kind: doc.layerKind(id),
        visible: doc.layerVisible(id),
      });
    }
    list.reverse();
    return list;
  });

  let error = $state<string | null>(null);

  function move(id: number, dir: 'up' | 'down') {
    if (!doc) return;
    error = null;
    try {
      if (dir === 'up') doc.moveLayerUp(id);
      else doc.moveLayerDown(id);
      onChange?.();
    } catch (err) {
      // `LayerAtEdge` is an expected no-op when the row is already
      // top-/bottom-most among its siblings (the buttons are also
      // disabled at the list ends); surface anything else.
      const msg = err instanceof Error ? err.message : String(err);
      if (!msg.includes('edge')) error = msg;
    }
  }
</script>

<aside
  class="flex w-64 shrink-0 flex-col gap-2 border-l border-neutral-800 bg-neutral-950 p-3 text-sm"
  aria-label="Layers"
>
  <header class="flex items-center justify-between">
    <h2 class="text-xs font-semibold tracking-wide text-neutral-300 uppercase">Layers</h2>
  </header>

  {#if layers.length === 0}
    <p class="text-xs text-neutral-500">No layers.</p>
  {:else}
    <ul class="flex flex-col gap-1">
      {#each layers as layer, i (layer.id)}
        {@const isActive = layer.id === activeLayerId}
        <li
          class="flex items-center gap-2 rounded border px-2 py-1"
          class:layer-active={isActive}
          class:border-neutral-800={!isActive}
        >
          <button
            type="button"
            class="layer-eye shrink-0"
            onclick={() => onToggleVisible?.(layer.id, !layer.visible)}
            aria-pressed={layer.visible}
            aria-label={`${layer.visible ? 'Hide' : 'Show'} ${layer.name}`}
            title={layer.visible ? 'Hide layer' : 'Show layer'}
          >
            {layer.visible ? '●' : '○'}
          </button>
          <button
            type="button"
            class="flex min-w-0 flex-1 items-center gap-2 text-left"
            onclick={() => onActivate?.(layer.id)}
            aria-pressed={isActive}
            title={`Select ${layer.name}`}
          >
            <span
              class="truncate text-sm"
              class:text-neutral-100={layer.visible}
              class:text-neutral-500={!layer.visible}
              title={layer.name}
            >
              {layer.name}
            </span>
            <span class="ml-auto shrink-0 text-[0.65rem] text-neutral-600 uppercase">
              {layer.kind}
            </span>
          </button>
          <div class="flex shrink-0 flex-col">
            <button
              type="button"
              class="layer-move"
              onclick={() => move(layer.id, 'up')}
              disabled={i === 0}
              aria-label={`Move ${layer.name} up`}
              title="Move up"
            >
              ▲
            </button>
            <button
              type="button"
              class="layer-move"
              onclick={() => move(layer.id, 'down')}
              disabled={i === layers.length - 1}
              aria-label={`Move ${layer.name} down`}
              title="Move down"
            >
              ▼
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#if error}
    <p class="text-xs text-red-400" role="alert">{error}</p>
  {/if}
</aside>

<style>
  .layer-active {
    border-color: rgb(59 130 246);
    background-color: rgb(30 41 59);
  }
  .layer-eye {
    font-size: 0.6rem;
    line-height: 1;
    color: rgb(115 115 115);
  }
  .layer-eye[aria-pressed='true'] {
    color: rgb(163 163 163);
  }
  .layer-eye:hover {
    color: rgb(229 229 229);
  }
  .layer-move {
    line-height: 1;
    padding: 0 0.25rem;
    font-size: 0.55rem;
    color: rgb(163 163 163);
  }
  .layer-move:hover:not(:disabled) {
    color: rgb(229 229 229);
  }
  .layer-move:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }
</style>
