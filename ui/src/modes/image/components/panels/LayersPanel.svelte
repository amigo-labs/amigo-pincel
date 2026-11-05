<script lang="ts">
  // Image mode's layers panel: the shared panel (spec §9.2) over the image
  // core's layer list. Rows address layers by stack index; every mutation
  // goes through the controller (CLAUDE.md §5.4). Groups are listed as a
  // tree, each header directly above its children
  // (docs/specs/image-layer-groups.md).
  import { editor } from '../../stores/editor.svelte';
  import type { BlendMode } from '../../core/wasm';
  import { canDeleteLayer, canMergeDown, treeOrder } from '../../core/layer-tree';
  import {
    addGroup,
    addLayer,
    deleteLayer,
    duplicateLayer,
    flattenImage,
    layerThumbnail,
    mergeDown,
    mergeVisible,
    moveLayerTo,
    renameLayer,
    selectLayer,
    setLayerBlendMode,
    setLayerLocked,
    setLayerOpacity,
    setLayerVisible,
  } from '../../core/controller';
  import LayersPanel from '../../../../lib/components/layers/LayersPanel.svelte';
  import { blendOptions, type LayerRow } from '../../../../lib/components/layers/layers';
  import { t } from '../../../../lib/i18n/index.svelte';

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

  // Top-most first, every group directly above its children.
  const rows = $derived<LayerRow[]>(
    treeOrder(editor.layers).flatMap((i) => {
      const l = editor.layers[i];
      if (!l) return [];
      const isGroup = l.kind === 'group';
      return [
        {
          key: l.id,
          ref: i,
          name: l.name,
          visible: l.visible,
          locked: l.locked,
          lockedByParent: l.locked_by_parent,
          opacity: l.opacity,
          blend: l.blend_mode,
          depth: l.depth,
          isGroup,
          ...(isGroup ? { badge: t('layers.kind.group') } : {}),
        },
      ];
    }),
  );
  const activeKey = $derived(editor.layers[editor.activeLayer]?.id ?? null);

  function isBlendMode(v: string): v is BlendMode {
    return (BLEND_MODES as string[]).includes(v);
  }

  /** Arrow-key reorder: swap with the neighbouring sibling, never leaving
   *  the group (Alt+←/→ does that). */
  function reorder(row: LayerRow, target: LayerRow): void {
    const i = rows.indexOf(row);
    const up = rows.indexOf(target) < i;
    const depth = row.depth ?? 0;
    if (up) {
      for (let j = i - 1; j >= 0; j--) {
        const d = rows[j]?.depth ?? 0;
        if (d < depth) return;
        const sibling = rows[j];
        if (d === depth && sibling) return moveLayerTo(row.ref, sibling.ref, 'above');
      }
    } else {
      for (let j = i + 1; j < rows.length; j++) {
        const d = rows[j]?.depth ?? 0;
        if (d < depth) return;
        const sibling = rows[j];
        if (d === depth && sibling) return moveLayerTo(row.ref, sibling.ref, 'below');
      }
    }
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
  canMergeDown={canMergeDown(editor.layers, editor.activeLayer)}
  canDelete={canDeleteLayer(editor.layers, editor.activeLayer)}
  onSelect={(row) => selectLayer(row.ref)}
  onToggleVisible={(row) => setLayerVisible(row.ref, !row.visible)}
  onToggleLocked={(row) => setLayerLocked(row.ref, !row.locked)}
  onRename={(row, name) => renameLayer(row.ref, name)}
  onReorder={reorder}
  onMove={(row, target, place) => moveLayerTo(row.ref, target.ref, place)}
  onAddGroup={addGroup}
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
