<script lang="ts">
  import type { Document } from '../core';
  import TileThumbnail from './TileThumbnail.svelte';

  // The wasm `Document` is the source of truth for tileset state. The
  // panel reads through the M8.6 / M8.7b surface (`tilesetCount`,
  // `tilesetIdAt`, `tilesetName`, `tilesetTileWidth`,
  // `tilesetTileHeight`, `tilesetTileCount`, `tilePixels`) and emits
  // `addTileset`, `addTile`, `addTilemapLayer` on the various buttons.
  // `rev` is a parent-managed change counter that bumps whenever the
  // wasm side may have mutated tilesets (new / open document, undo,
  // redo, or any edit). The `$derived.by` block reads it to mark the
  // list reactive against opaque wasm mutations.
  //
  // `selectedTile` highlights the active stamp target — the parent
  // owns the state so the Tilemap Stamp tool can read it without the
  // panel re-broadcasting. `onSelectStampTile` / `onEditTile` fire on
  // single click / double click respectively, letting the parent
  // route the gesture to the Stamp tool or the Tile Editor sub-mode.
  let {
    doc,
    rev = 0,
    selectedTile = null,
    onChange,
    onSelectStampTile,
    onEditTile,
  }: {
    doc: Document | null;
    rev?: number;
    selectedTile?: { tilesetId: number; tileId: number } | null;
    onChange?: () => void;
    onSelectStampTile?: (tilesetId: number, tileId: number) => void;
    onEditTile?: (tilesetId: number, tileId: number) => void;
  } = $props();

  type TilesetRow = {
    id: number;
    name: string;
    tileW: number;
    tileH: number;
    tileCount: number;
  };

  const tilesets = $derived.by<TilesetRow[]>(() => {
    void rev;
    if (!doc) return [];
    const list: TilesetRow[] = [];
    const count = doc.tilesetCount;
    for (let i = 0; i < count; i += 1) {
      let id: number;
      try {
        id = doc.tilesetIdAt(i);
      } catch {
        continue;
      }
      list.push({
        id,
        name: doc.tilesetName(id),
        tileW: doc.tilesetTileWidth(id),
        tileH: doc.tilesetTileHeight(id),
        tileCount: doc.tilesetTileCount(id),
      });
    }
    return list;
  });

  let formOpen = $state(false);
  let formName = $state('Tileset');
  let formTileW = $state(16);
  let formTileH = $state(16);
  let formError = $state<string | null>(null);
  // Per-tileset transient error surface, keyed by tileset id. Cleared
  // on the next mutation that targets the same tileset.
  let rowError = $state<Record<number, string | null>>({});

  function openForm() {
    formOpen = true;
    formError = null;
  }

  function closeForm() {
    formOpen = false;
    formError = null;
  }

  function submitForm(e: SubmitEvent) {
    e.preventDefault();
    if (!doc) return;
    const name = formName.trim();
    if (name.length === 0) {
      formError = 'name is required';
      return;
    }
    if (!Number.isInteger(formTileW) || formTileW <= 0) {
      formError = 'tile width must be a positive integer';
      return;
    }
    if (!Number.isInteger(formTileH) || formTileH <= 0) {
      formError = 'tile height must be a positive integer';
      return;
    }
    try {
      doc.addTileset(name, formTileW, formTileH);
    } catch (err) {
      formError = err instanceof Error ? err.message : String(err);
      return;
    }
    formOpen = false;
    formError = null;
    formName = 'Tileset';
    onChange?.();
  }

  function addTile(tilesetId: number) {
    if (!doc) return;
    rowError[tilesetId] = null;
    try {
      const newTileId = doc.addTile(tilesetId);
      // Auto-select the freshly added tile as the stamp target so the
      // common workflow "create tile, paint, place" needs no extra
      // click.
      onSelectStampTile?.(tilesetId, newTileId);
    } catch (err) {
      rowError[tilesetId] = err instanceof Error ? err.message : String(err);
      return;
    }
    onChange?.();
  }

  function addTilemapLayer(tilesetId: number, tilesetName: string) {
    if (!doc) return;
    rowError[tilesetId] = null;
    try {
      doc.addTilemapLayer(`${tilesetName} Layer`, tilesetId);
    } catch (err) {
      rowError[tilesetId] = err instanceof Error ? err.message : String(err);
      return;
    }
    onChange?.();
  }

  function selectStamp(tilesetId: number, tileId: number) {
    onSelectStampTile?.(tilesetId, tileId);
  }

  function editTile(tilesetId: number, tileId: number) {
    onEditTile?.(tilesetId, tileId);
  }
</script>

<aside
  class="flex w-64 shrink-0 flex-col gap-2 border-l border-neutral-800 bg-neutral-950 p-3 text-sm"
  aria-label="Tilesets"
>
  <header class="flex items-center justify-between">
    <h2 class="text-xs font-semibold tracking-wide text-neutral-300 uppercase">
      Tilesets
    </h2>
    {#if !formOpen}
      <button
        type="button"
        class="panel-btn"
        onclick={openForm}
        disabled={!doc}
        aria-label="Add tileset"
      >
        + Add
      </button>
    {/if}
  </header>

  {#if formOpen}
    <form class="flex flex-col gap-2 rounded border border-neutral-800 p-2" onsubmit={submitForm}>
      <label class="flex flex-col gap-1 text-xs text-neutral-400">
        <span>Name</span>
        <input
          type="text"
          bind:value={formName}
          class="rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-sm text-neutral-100"
        />
      </label>
      <div class="flex gap-2">
        <label class="flex flex-1 flex-col gap-1 text-xs text-neutral-400">
          <span>Tile W</span>
          <input
            type="number"
            min="1"
            step="1"
            inputmode="numeric"
            bind:value={formTileW}
            class="rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-sm text-neutral-100"
          />
        </label>
        <label class="flex flex-1 flex-col gap-1 text-xs text-neutral-400">
          <span>Tile H</span>
          <input
            type="number"
            min="1"
            step="1"
            inputmode="numeric"
            bind:value={formTileH}
            class="rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-sm text-neutral-100"
          />
        </label>
      </div>
      {#if formError}
        <p class="text-xs text-red-400" role="alert">{formError}</p>
      {/if}
      <div class="flex justify-end gap-1">
        <button type="button" class="panel-btn" onclick={closeForm}>Cancel</button>
        <button type="submit" class="panel-btn panel-btn-primary">Add</button>
      </div>
    </form>
  {/if}

  {#if tilesets.length === 0}
    <p class="text-xs text-neutral-500">
      {doc ? 'no tilesets yet' : 'open or create a document'}
    </p>
  {:else}
    <ul class="flex flex-col gap-1">
      {#each tilesets as ts (ts.id)}
        <li
          class="flex flex-col gap-1 rounded border border-neutral-800 px-2 py-1"
        >
          <span class="truncate text-sm text-neutral-100" title={ts.name}>{ts.name}</span>
          <span class="text-xs text-neutral-500">
            id {ts.id} · {ts.tileW}×{ts.tileH} · {ts.tileCount} tile{ts.tileCount === 1 ? '' : 's'}
          </span>
          <div class="flex gap-1">
            <button
              type="button"
              class="panel-btn"
              onclick={() => addTile(ts.id)}
              aria-label={`Add tile to ${ts.name}`}
            >
              + Tile
            </button>
            <button
              type="button"
              class="panel-btn"
              onclick={() => addTilemapLayer(ts.id, ts.name)}
              aria-label={`Add tilemap layer using ${ts.name}`}
            >
              + Layer
            </button>
          </div>
          {#if rowError[ts.id]}
            <p class="text-xs text-red-400" role="alert">{rowError[ts.id]}</p>
          {/if}
          {#if doc && ts.tileCount > 0}
            <div class="flex flex-wrap gap-1" aria-label={`${ts.name} tiles`}>
              {#each Array.from({ length: ts.tileCount }, (_, i) => i) as tileId (tileId)}
                {@const isSelected =
                  selectedTile?.tilesetId === ts.id && selectedTile?.tileId === tileId}
                <button
                  type="button"
                  class="tile-btn"
                  class:tile-btn-selected={isSelected}
                  onclick={() => selectStamp(ts.id, tileId)}
                  ondblclick={() => editTile(ts.id, tileId)}
                  aria-pressed={isSelected}
                  aria-label={`Select tile ${tileId} of ${ts.name} (double-click to edit)`}
                  title={`tile ${tileId} — click to stamp, double-click to edit`}
                >
                  <TileThumbnail
                    {doc}
                    tilesetId={ts.id}
                    {tileId}
                    tileW={ts.tileW}
                    tileH={ts.tileH}
                    {rev}
                  />
                </button>
              {/each}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</aside>

<style>
  .panel-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.125rem 0.5rem;
    font-size: 0.75rem;
    color: rgb(229 229 229);
  }
  .panel-btn:hover:not(:disabled) {
    background-color: rgb(38 38 38);
  }
  .panel-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .panel-btn-primary {
    background-color: rgb(55 65 81);
    border-color: rgb(115 115 115);
  }
  .panel-btn-primary:hover:not(:disabled) {
    background-color: rgb(75 85 99);
  }
  .tile-btn {
    padding: 0;
    border: 1px solid transparent;
    background: transparent;
    border-radius: 0.25rem;
    cursor: pointer;
    line-height: 0;
  }
  .tile-btn:hover {
    border-color: rgb(82 82 82);
  }
  .tile-btn-selected {
    border-color: rgb(59 130 246);
    box-shadow: 0 0 0 1px rgb(59 130 246);
  }
</style>
