<script lang="ts">
  // Image mode's layers panel: the shared panel (spec §9.2) over the image
  // core's layer list. Rows address layers by stack index; every mutation
  // goes through the controller (CLAUDE.md §5.4).
  import { editor } from '../../stores/editor.svelte';
  import type { BlendMode } from '../../core/wasm';
  import {
    addLayer,
    deleteLayer,
    duplicateLayer,
    flattenImage,
    layerThumbnail,
    mergeDown,
    mergeVisible,
    renameLayer,
    reorderLayer,
    selectLayer,
    setLayerBlendMode,
    setLayerLocked,
    setLayerOpacity,
    setLayerVisible,
  } from '../../core/controller';
  import LayersPanel from '../../../../lib/components/layers/LayersPanel.svelte';
  import { blendOptions, type LayerRow } from '../../../../lib/components/layers/layers';

  const BLEND_MODES: BlendMode[] = [
    'normal',
    'multiply',
    'screen',
    'overlay',
    'darken',
    'lighten',
    'color_dodge',
    'color_burn',
    'hard_light',
    'soft_light',
    'difference',
    'exclusion',
  ];
  const blendModes = $derived(blendOptions(BLEND_MODES));

  // Top-most first: the reverse of the core's bottom-up storage order.
  const rows = $derived<LayerRow[]>(
    editor.layers
      .map((l, i) => ({
        key: l.id,
        ref: i,
        name: l.name,
        visible: l.visible,
        locked: l.locked,
        opacity: l.opacity,
        blend: l.blend_mode,
      }))
      .reverse(),
  );
  const activeKey = $derived(editor.layers[editor.activeLayer]?.id ?? null);

  function isBlendMode(v: string): v is BlendMode {
    return (BLEND_MODES as string[]).includes(v);
  }
</script>

<LayersPanel
  {rows}
  {activeKey}
  {blendModes}
  revision={editor.revision}
  thumbnail={(row) => {
    const rgba = layerThumbnail(row.key);
    return rgba ? { rgba, width: 32, height: 32 } : null;
  }}
  canDuplicate={editor.layers.length > 0}
  canMergeDown={editor.activeLayer > 0}
  canDelete={editor.layers.length > 1}
  onSelect={(row) => selectLayer(row.ref)}
  onToggleVisible={(row) => setLayerVisible(row.ref, !row.visible)}
  onToggleLocked={(row) => setLayerLocked(row.ref, !row.locked)}
  onRename={(row, name) => renameLayer(row.ref, name)}
  onReorder={(row, target) => reorderLayer(row.ref, target.ref)}
  onOpacity={(row, opacity, commit) => {
    if (!commit) setLayerOpacity(row.ref, opacity * 100);
  }}
  onBlend={(row, mode) => {
    if (isBlendMode(mode)) setLayerBlendMode(row.ref, mode);
  }}
  onAdd={addLayer}
  onDuplicate={() => duplicateLayer(editor.activeLayer)}
  onMergeDown={() => mergeDown(editor.activeLayer)}
  onDelete={() => deleteLayer(editor.activeLayer)}
  onMergeVisible={mergeVisible}
  onFlatten={flattenImage}
/>
