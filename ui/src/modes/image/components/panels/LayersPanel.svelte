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
  import Icon from '../../../../lib/components/icons/Icon.svelte';
  import IconButton from '../../../../lib/components/layout/IconButton.svelte';
  import Panel from '../../../../lib/components/layout/Panel.svelte';

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
  const displayLayers = $derived(editor.layers.map((layer, i) => ({ layer, i })).reverse());

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

<Panel count={editor.layers.length} bodyClass="flex flex-col">
  {#snippet actions()}
    <IconButton icon="plus" label="Add layer" size="sm" onclick={addLayer} />
    <IconButton
      icon="duplicate"
      label="Duplicate layer"
      size="sm"
      onclick={() => duplicateLayer(editor.activeLayer)}
    />
    <IconButton
      icon="merge-down"
      label="Merge down"
      size="sm"
      disabled={editor.activeLayer === 0}
      onclick={() => mergeDown(editor.activeLayer)}
    />
    <IconButton
      icon="trash"
      label="Delete layer"
      size="sm"
      disabled={editor.layers.length <= 1}
      onclick={() => deleteLayer(editor.activeLayer)}
    />
  {/snippet}

  <!-- Layer list, top-to-bottom display order. -->
  <ul class="min-h-0 flex-1 overflow-auto px-2 pb-2">
    {#each displayLayers as { layer, i } (layer.id)}
      <li
        class="mb-1 rounded border p-1.5"
        class:border-pc-accent={i === editor.activeLayer}
        class:bg-pc-active={i === editor.activeLayer}
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
            class="text-pc-muted hover:text-pc-text"
            title={layer.visible ? 'Hide layer' : 'Show layer'}
            aria-label={layer.visible ? 'Hide layer' : 'Show layer'}
            aria-pressed={layer.visible}
            onclick={() => setLayerVisible(i, !layer.visible)}
          >
            <Icon name={layer.visible ? 'eye' : 'eye-off'} size={15} />
          </button>
          <button
            class="text-pc-muted hover:text-pc-text"
            title={layer.locked ? 'Unlock layer' : 'Lock layer'}
            aria-label={layer.locked ? 'Unlock layer' : 'Lock layer'}
            aria-pressed={layer.locked}
            onclick={() => setLayerLocked(i, !layer.locked)}
          >
            <Icon name={layer.locked ? 'lock' : 'unlock'} size={15} />
          </button>
          <LayerThumbnail layerId={layer.id} />
          <div class="min-w-0 flex-1">
            {#if editingIndex === i}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="w-full rounded border border-pc-border bg-pc-bg px-1 text-xs text-pc-text"
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
                class="block w-full truncate text-left text-xs text-pc-text"
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
            class="min-w-0 flex-1 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-xs text-pc-text"
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

  <!-- Whole-stack actions (spec §16.5); per-layer ones sit in the header. -->
  <div class="flex gap-1 px-2 pb-2 text-xs">
    <button class="panel-btn flex-1" onclick={mergeVisible}>Merge visible</button>
    <button class="panel-btn flex-1" onclick={flattenImage}>Flatten</button>
  </div>
</Panel>
