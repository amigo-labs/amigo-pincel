<script lang="ts">
  // Layers panel (spec §16.5): per-layer visibility, lock, thumbnail, name,
  // blend mode and opacity, plus add/delete/duplicate/merge actions and
  // drag-to-reorder. All mutations go through the controller (CLAUDE.md §5.4).
  import { editor } from '../../stores/editor.svelte';
  import type { BlendMode } from '../../core/wasm';
  import {
    addLayer,
    deleteLayer,
    duplicateLayer,
    reorderLayer,
    renameLayer,
    setLayerOpacity,
    setLayerBlendMode,
    setLayerVisible,
    setLayerLocked,
    mergeDown,
    mergeVisible,
    flattenImage,
    selectLayer,
  } from '../../core/controller';
  import LayerThumbnail from './LayerThumbnail.svelte';

  const BLEND_MODES: Array<{ value: BlendMode; label: string }> = [
    { value: 'normal', label: 'Normal' },
    { value: 'multiply', label: 'Multiply' },
    { value: 'screen', label: 'Screen' },
    { value: 'overlay', label: 'Overlay' },
    { value: 'darken', label: 'Darken' },
    { value: 'lighten', label: 'Lighten' },
    { value: 'color_dodge', label: 'Color Dodge' },
    { value: 'color_burn', label: 'Color Burn' },
    { value: 'hard_light', label: 'Hard Light' },
    { value: 'soft_light', label: 'Soft Light' },
    { value: 'difference', label: 'Difference' },
    { value: 'exclusion', label: 'Exclusion' },
  ];

  // Display order is top-to-bottom, i.e. the reverse of core storage order.
  const displayIndices = $derived(editor.layers.map((_, i) => editor.layers.length - 1 - i));

  let editingIndex = $state<number | null>(null);
  let editingName = $state('');
  let draggingIndex: number | null = null;

  function startRename(index: number, current: string): void {
    editingIndex = index;
    editingName = current;
  }

  function commitRename(): void {
    if (editingIndex !== null) {
      const name = editingName.trim();
      if (name) {
        renameLayer(editingIndex, name);
      }
      editingIndex = null;
    }
  }

  function onDrop(target: number): void {
    if (draggingIndex !== null) {
      reorderLayer(draggingIndex, target);
      draggingIndex = null;
    }
  }
</script>

<div class="flex min-h-0 flex-1 flex-col border-b border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)]">
  <div class="flex items-center justify-between px-3 pt-3">
    <h2 class="text-xs font-semibold uppercase tracking-wide text-neutral-500">Layers</h2>
    <span class="text-xs text-neutral-500">{editor.layers.length}</span>
  </div>

  <!-- Layer list, top-to-bottom display order. -->
  <ul class="min-h-0 flex-1 overflow-auto px-2 py-2">
    {#each displayIndices as i (editor.layers[i].id)}
      {@const layer = editor.layers[i]}
      <li
        class="mb-1 rounded border p-1.5"
        class:border-[var(--fl-accent)]={i === editor.activeLayer}
        class:bg-neutral-700={i === editor.activeLayer}
        class:border-transparent={i !== editor.activeLayer}
        draggable="true"
        ondragstart={(e) => {
          draggingIndex = i;
          // Firefox only starts a drag when dataTransfer carries data.
          e.dataTransfer?.setData('text/plain', String(i));
        }}
        ondragover={(e) => e.preventDefault()}
        ondrop={() => onDrop(i)}
        ondragend={() => (draggingIndex = null)}
      >
        <div class="flex items-center gap-1.5">
          <button
            class="text-sm leading-none"
            title={layer.visible ? 'Hide layer' : 'Show layer'}
            aria-label={layer.visible ? 'Hide layer' : 'Show layer'}
            aria-pressed={layer.visible}
            onclick={() => setLayerVisible(i, !layer.visible)}
          >
            {layer.visible ? '👁' : '🙈'}
          </button>
          <button
            class="text-sm leading-none"
            title={layer.locked ? 'Unlock layer' : 'Lock layer'}
            aria-label={layer.locked ? 'Unlock layer' : 'Lock layer'}
            aria-pressed={layer.locked}
            onclick={() => setLayerLocked(i, !layer.locked)}
          >
            {layer.locked ? '🔒' : '🔓'}
          </button>
          <LayerThumbnail layerId={layer.id} />
          <div class="min-w-0 flex-1">
            {#if editingIndex === i}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="w-full rounded border border-[var(--fl-panel-border)] bg-[var(--fl-app-bg)] px-1 text-xs text-neutral-100"
                bind:value={editingName}
                autofocus
                onblur={commitRename}
                onkeydown={(e) => {
                  if (e.key === 'Enter') {
                    commitRename();
                  } else if (e.key === 'Escape') {
                    editingIndex = null;
                  }
                }}
              />
            {:else}
              <button
                class="block w-full truncate text-left text-xs text-neutral-200"
                title={layer.name}
                onclick={() => selectLayer(i)}
                ondblclick={() => startRename(i, layer.name)}
              >
                {layer.name}
              </button>
            {/if}
          </div>
        </div>

        <div class="mt-1.5 flex items-center gap-1.5">
          <select
            class="min-w-0 flex-1 rounded border border-[var(--fl-panel-border)] bg-[var(--fl-app-bg)] px-1 py-0.5 text-xs text-neutral-200"
            value={layer.blend_mode}
            aria-label="Blend mode"
            onchange={(e) => setLayerBlendMode(i, e.currentTarget.value as BlendMode)}
          >
            {#each BLEND_MODES as m (m.value)}
              <option value={m.value}>{m.label}</option>
            {/each}
          </select>
          <input
            type="range"
            min="0"
            max="100"
            value={Math.round(layer.opacity * 100)}
            class="w-16"
            aria-label="Layer opacity"
            title={`Opacity ${Math.round(layer.opacity * 100)}%`}
            oninput={(e) => setLayerOpacity(i, Number(e.currentTarget.value))}
          />
        </div>
      </li>
    {/each}
  </ul>

  <!-- Action buttons (spec §16.5). -->
  <div class="grid grid-cols-4 gap-1 border-t border-[var(--fl-panel-border)] p-2 text-xs">
    <button class="rounded py-1 hover:bg-neutral-700" title="Add layer" onclick={addLayer}>＋</button>
    <button
      class="rounded py-1 hover:bg-neutral-700"
      title="Duplicate layer"
      onclick={() => duplicateLayer(editor.activeLayer)}
    >
      ⧉
    </button>
    <button
      class="rounded py-1 hover:bg-neutral-700 disabled:opacity-40"
      title="Merge down"
      disabled={editor.activeLayer === 0}
      onclick={() => mergeDown(editor.activeLayer)}
    >
      ⤓
    </button>
    <button
      class="rounded py-1 hover:bg-neutral-700 disabled:opacity-40"
      title="Delete layer"
      disabled={editor.layers.length <= 1}
      onclick={() => deleteLayer(editor.activeLayer)}
    >
      🗑
    </button>
    <button class="col-span-2 rounded py-1 hover:bg-neutral-700" title="Merge visible" onclick={mergeVisible}>
      Merge visible
    </button>
    <button class="col-span-2 rounded py-1 hover:bg-neutral-700" title="Flatten image" onclick={flattenImage}>
      Flatten
    </button>
  </div>
</div>
