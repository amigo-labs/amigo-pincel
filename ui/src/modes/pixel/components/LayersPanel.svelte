<script lang="ts">
  // Pixel mode's layers panel: the shared panel (spec §9.2) over the wasm
  // `Document`, which stays the source of truth. `rev` is the editor's
  // change counter; the rows re-read the document whenever it moves.
  // Rows address layers by id; groups indent and carry a kind badge.
  import LayersPanel from '../../../lib/components/layers/LayersPanel.svelte';
  import { blendOptions, type LayerRow } from '../../../lib/components/layers/layers';
  import { blendModeNames, type Document } from '../../../lib/core';
  import { t } from '../../../lib/i18n/index.svelte';

  interface Props {
    doc: Document | null;
    rev: number;
    frame: number;
    activeLayerId: number | null;
    onActivate: (layerId: number) => void;
    onToggleVisible: (layerId: number, visible: boolean) => void;
    onToggleLocked: (layerId: number, editable: boolean) => void;
    onRename: (layerId: number, name: string) => void;
    onReorder: (layerId: number, targetId: number) => void;
    /** `opacity` is `0..=255`; `commit` ends a slider drag. */
    onSetOpacity: (layerId: number, opacity: number, commit: boolean) => void;
    onSetBlendMode: (layerId: number, mode: string) => void;
    onAddLayer: () => void;
    onDuplicate: (layerId: number) => void;
    onMergeDown: (layerId: number) => void;
    onRemoveLayer: (layerId: number) => void;
    onFlatten: () => void;
  }
  const p: Props = $props();

  type Row = LayerRow & { kind: string };

  const rows = $derived.by<Row[]>(() => {
    void p.rev;
    const doc = p.doc;
    if (!doc) return [];
    const list: Row[] = [];
    for (let i = 0; i < doc.layerCount; i += 1) {
      let id: number;
      try {
        id = doc.layerIdAt(i);
      } catch {
        continue;
      }
      const kind = doc.layerKind(id);
      const editable = doc.layerEditable(id);
      list.push({
        key: String(id),
        ref: id,
        kind,
        name: doc.layerName(id),
        visible: doc.layerVisible(id),
        locked: !editable,
        lockedByParent: editable && doc.layerLocked(id),
        opacity: doc.layerOpacity(id) / 255,
        blend: doc.layerBlendMode(id),
        depth: doc.layerDepth(id),
        ...(kind === 'image' ? {} : { badge: t(kind === 'group' ? 'layers.kind.group' : 'layers.kind.tilemap') }),
      });
    }
    return list.reverse();
  });

  const blendModes = $derived(p.doc ? blendOptions(blendModeNames()) : []);
  const activeIndex = $derived(rows.findIndex((r) => r.ref === p.activeLayerId));
  const active = $derived(rows[activeIndex] ?? null);
  const canMergeDown = $derived.by(() => {
    if (!active || active.kind !== 'image') return false;
    const below = rows[activeIndex + 1];
    return below !== undefined && below.kind === 'image' && (below.depth ?? 0) === (active.depth ?? 0);
  });
</script>

<LayersPanel
  {rows}
  activeKey={p.activeLayerId === null ? null : String(p.activeLayerId)}
  {blendModes}
  revision={p.rev}
  thumbnail={(row) => {
    if (!p.doc) return null;
    try {
      const f = p.doc.layerThumbnail(row.ref, p.frame, 32);
      const thumb = { rgba: f.pixels, width: f.width, height: f.height };
      f.free();
      return thumb;
    } catch {
      return null;
    }
  }}
  canDuplicate={active !== null && active.kind !== 'group'}
  {canMergeDown}
  canDelete={active !== null && rows.length > 1}
  onSelect={(row) => p.onActivate(row.ref)}
  onToggleVisible={(row) => p.onToggleVisible(row.ref, !row.visible)}
  onToggleLocked={(row) => p.onToggleLocked(row.ref, row.locked)}
  onRename={(row, name) => p.onRename(row.ref, name)}
  onReorder={(row, target) => p.onReorder(row.ref, target.ref)}
  onOpacity={(row, opacity, commit) => p.onSetOpacity(row.ref, Math.round(opacity * 255), commit)}
  onBlend={(row, mode) => p.onSetBlendMode(row.ref, mode)}
  onAdd={p.onAddLayer}
  onDuplicate={() => active && p.onDuplicate(active.ref)}
  onMergeDown={() => active && p.onMergeDown(active.ref)}
  onDelete={() => active && p.onRemoveLayer(active.ref)}
  onFlatten={p.onFlatten}
/>
