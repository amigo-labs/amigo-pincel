<script lang="ts">
  // The layers panel both modes share (spec §9.2): per row a drag grip,
  // visibility, lock, thumbnail, name and rename button; the active layer's
  // opacity and blend mode below; add / duplicate / merge / delete in the
  // header and the whole-stack actions in the footer. Reordering uses
  // pointer events on the grip (HTML drag-and-drop does not work with
  // touch) and the arrow keys on a focused grip. With `onMove` the drop
  // also picks above / below / into-a-group from where on the row the
  // pointer is, and Alt+←/→ move a layer out of / into a group.
  import Icon from '../icons/Icon.svelte';
  import IconButton from '../layout/IconButton.svelte';
  import Panel from '../layout/Panel.svelte';
  import { t } from '../../i18n/index.svelte';
  import LayerThumbnail from './LayerThumbnail.svelte';
  import type { BlendOption, DropPlace, LayerRow, Thumb } from './layers';

  interface Props {
    /** Top-most first. */
    rows: LayerRow[];
    activeKey: string | null;
    blendModes: BlendOption[];
    /** Bumped on every document change; redraws the thumbnails. */
    revision: number;
    thumbnail: (row: LayerRow) => Thumb | null;
    canDuplicate: boolean;
    canMergeDown: boolean;
    canDelete: boolean;
    onSelect: (row: LayerRow) => void;
    onToggleVisible: (row: LayerRow) => void;
    onToggleLocked: (row: LayerRow) => void;
    onRename: (row: LayerRow, name: string) => void;
    /** Move `row` to `target`'s slot. */
    onReorder: (row: LayerRow, target: LayerRow) => void;
    /** Move `row` above / below `target` or into it (a group), across
     *  groups; replaces `onReorder` for pointer drags when given. */
    onMove?: (row: LayerRow, target: LayerRow, place: DropPlace) => void;
    /** Add an empty group (header button shown when given). */
    onAddGroup?: () => void;
    /** `opacity` is `0..=1`; `commit` marks the end of a slider drag. */
    onOpacity: (row: LayerRow, opacity: number, commit: boolean) => void;
    onBlend: (row: LayerRow, mode: string) => void;
    onAdd: () => void;
    onDuplicate: () => void;
    onMergeDown: () => void;
    onDelete: () => void;
    onMergeVisible?: () => void;
    onFlatten: () => void;
  }
  const {
    rows,
    activeKey,
    blendModes,
    revision,
    thumbnail,
    canDuplicate,
    canMergeDown,
    canDelete,
    onSelect,
    onToggleVisible,
    onToggleLocked,
    onRename,
    onReorder,
    onMove,
    onAddGroup,
    onOpacity,
    onBlend,
    onAdd,
    onDuplicate,
    onMergeDown,
    onDelete,
    onMergeVisible,
    onFlatten,
  }: Props = $props();

  const active = $derived(rows.find((r) => r.key === activeKey) ?? null);

  // Inline rename: double-click the name or press the rename button.
  let editingKey = $state<string | null>(null);
  let draft = $state('');

  function startRename(row: LayerRow): void {
    editingKey = row.key;
    draft = row.name;
  }

  function commitRename(row: LayerRow): void {
    if (editingKey !== row.key) return;
    editingKey = null;
    const name = draft.trim();
    if (name && name !== row.name) onRename(row, name);
  }

  function onRenameKey(e: KeyboardEvent, row: LayerRow): void {
    e.stopPropagation();
    if (e.key === 'Enter') {
      e.preventDefault();
      commitRename(row);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      editingKey = null;
    }
  }

  // Pointer drag on a row's grip; `over` is the row under the pointer and
  // `place` where on it the drop lands (with `onMove`).
  let drag = $state<{ key: string; over: string | null; place: DropPlace } | null>(null);
  const dragIndex = $derived(drag ? rows.findIndex((r) => r.key === drag?.key) : -1);

  // Above / below / into from the pointer's height on the row: the outer
  // quarters are the gaps above and below it; a group's middle takes the
  // layer in (as does its lower edge, which is the slot of its top child),
  // anything else's middle swaps places in the drag's direction.
  function placeOn(el: HTMLElement, row: LayerRow, clientY: number): DropPlace {
    const box = el.getBoundingClientRect();
    const f = box.height > 0 ? (clientY - box.top) / box.height : 0.5;
    if (f < 0.25) return 'above';
    if (row.isGroup) return 'into';
    if (f > 0.75) return 'below';
    return rows.indexOf(row) < dragIndex ? 'above' : 'below';
  }

  function onGripDown(e: PointerEvent, row: LayerRow): void {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { key: row.key, over: null, place: 'above' };
  }

  function onGripMove(e: PointerEvent): void {
    if (!drag) return;
    const el = document
      .elementFromPoint(e.clientX, e.clientY)
      ?.closest<HTMLElement>('[data-layer-key]');
    const row = rows.find((r) => r.key === el?.dataset.layerKey);
    drag.over = row?.key ?? null;
    if (el && row) drag.place = placeOn(el, row, e.clientY);
  }

  function onGripUp(): void {
    if (!drag) return;
    const { key, over, place } = drag;
    drag = null;
    const from = rows.find((r) => r.key === key);
    const to = rows.find((r) => r.key === over);
    if (!from || !to || from === to) return;
    if (onMove) onMove(from, to, place);
    else onReorder(from, to);
  }

  function onGripKey(e: KeyboardEvent, row: LayerRow, i: number): void {
    if (onMove && e.altKey && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      e.stopPropagation();
      const depth = row.depth ?? 0;
      if (e.key === 'ArrowLeft') {
        // Out of the enclosing group: just above it.
        const parent = rows.slice(0, i).findLast((r) => (r.depth ?? 0) < depth);
        if (parent) onMove(row, parent, 'above');
      } else {
        // Into the nearest group above at the same level.
        const group = rows.slice(0, i).findLast((r) => r.isGroup && (r.depth ?? 0) === depth);
        if (group) onMove(row, group, 'into');
      }
      return;
    }
    const step = e.key === 'ArrowUp' ? -1 : e.key === 'ArrowDown' ? 1 : 0;
    if (step === 0) return;
    e.preventDefault();
    e.stopPropagation();
    const target = rows[i + step];
    if (target) onReorder(row, target);
  }

  const pct = (v: number): number => Math.round(v * 100);
</script>

<Panel count={rows.length} bodyClass="flex min-h-0 flex-col gap-2 px-2 pb-2">
  {#snippet actions()}
    <IconButton icon="plus" label={t('layers.add')} size="sm" onclick={onAdd} />
    {#if onAddGroup}
      <IconButton icon="folder-plus" label={t('layers.addGroup')} size="sm" onclick={onAddGroup} />
    {/if}
    <IconButton
      icon="duplicate"
      label={t('layers.duplicate')}
      size="sm"
      disabled={!canDuplicate}
      onclick={onDuplicate}
    />
    <IconButton
      icon="merge-down"
      label={t('layers.mergeDown')}
      size="sm"
      disabled={!canMergeDown}
      onclick={onMergeDown}
    />
    <IconButton
      icon="trash"
      label={t('layers.delete')}
      size="sm"
      disabled={!canDelete}
      onclick={onDelete}
    />
  {/snippet}

  {#if rows.length === 0}
    <p class="px-1 text-xs text-pc-faint">{t('layers.none')}</p>
  {:else}
    <ul class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto" aria-label={t('layers.list')}>
      {#each rows as row, i (row.key)}
        {@const isActive = row.key === activeKey}
        {@const isOver = drag !== null && drag.over === row.key && drag.key !== row.key}
        <li
          data-layer-key={row.key}
          class="layer-row flex items-center gap-1 rounded border py-0.5 pr-1"
          class:layer-active={isActive}
          class:border-transparent={!isActive}
          class:drop-above={isOver && (onMove ? drag?.place === 'above' : i < dragIndex)}
          class:drop-below={isOver && (onMove ? drag?.place === 'below' : i > dragIndex)}
          class:drop-into={isOver && onMove !== undefined && drag?.place === 'into'}
          data-depth={row.depth ?? 0}
          class:opacity-50={drag?.key === row.key}
          style:padding-left="{(row.depth ?? 0) * 0.75}rem"
        >
          <button
            type="button"
            class="grip flex h-8 w-5 shrink-0 cursor-grab items-center justify-center text-pc-faint hover:text-pc-text"
            aria-label={t('layers.move', { name: row.name })}
            title={t('layers.moveHint')}
            onpointerdown={(e) => onGripDown(e, row)}
            onpointermove={onGripMove}
            onpointerup={onGripUp}
            onpointercancel={() => (drag = null)}
            onkeydown={(e) => onGripKey(e, row, i)}
          >
            <Icon name="grip" size={14} />
          </button>
          <button
            type="button"
            class="row-icon"
            aria-pressed={row.visible}
            aria-label={t(row.visible ? 'layers.hide' : 'layers.show', { name: row.name })}
            title={t(row.visible ? 'layers.hide' : 'layers.show', { name: row.name })}
            onclick={() => onToggleVisible(row)}
          >
            <Icon name={row.visible ? 'eye' : 'eye-off'} size={15} />
          </button>
          <button
            type="button"
            class="row-icon"
            class:row-locked={row.locked}
            aria-pressed={row.locked}
            aria-label={t(row.locked ? 'layers.unlock' : 'layers.lock', { name: row.name })}
            title={row.lockedByParent && !row.locked
              ? t('layers.lockedByGroup')
              : t(row.locked ? 'layers.unlock' : 'layers.lock', { name: row.name })}
            onclick={() => onToggleLocked(row)}
          >
            <Icon name={row.locked || row.lockedByParent ? 'lock' : 'unlock'} size={15} />
          </button>
          <LayerThumbnail source={() => thumbnail(row)} {revision} />
          <div class="ml-1 min-w-0 flex-1">
            {#if editingKey === row.key}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="opt-field w-full"
                bind:value={draft}
                autofocus
                aria-label={t('layers.renameLabel', { name: row.name })}
                onkeydown={(e) => onRenameKey(e, row)}
                onblur={() => commitRename(row)}
              />
            {:else}
              <button
                type="button"
                class="flex w-full min-w-0 items-center gap-1.5 text-left"
                aria-pressed={isActive}
                title={row.name}
                onclick={() => onSelect(row)}
                ondblclick={() => startRename(row)}
              >
                <span class="truncate text-xs" class:text-pc-faint={!row.visible}>{row.name}</span>
                {#if row.badge}
                  <span class="shrink-0 text-[10px] text-pc-faint uppercase">{row.badge}</span>
                {/if}
              </button>
            {/if}
          </div>
          <button
            type="button"
            class="row-icon"
            aria-label={t('layers.renameLabel', { name: row.name })}
            title={t('layers.rename')}
            onclick={() => startRename(row)}
          >
            <Icon name="rename" size={14} />
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if active}
    <div
      class="flex shrink-0 flex-col gap-1.5 rounded border border-pc-border p-2 text-xs text-pc-muted"
      role="group"
      aria-label={t('layers.properties')}
    >
      <label class="flex items-center gap-2">
        <span class="w-16 shrink-0">{t('layers.opacity')}</span>
        <input
          type="range"
          min="0"
          max="100"
          value={pct(active.opacity)}
          class="min-w-0 flex-1"
          aria-label={t('layers.opacityLabel')}
          oninput={(e) => onOpacity(active, Number(e.currentTarget.value) / 100, false)}
          onchange={(e) => onOpacity(active, Number(e.currentTarget.value) / 100, true)}
        />
        <span class="w-9 text-right tabular-nums">{pct(active.opacity)}%</span>
      </label>
      <label class="flex items-center gap-2">
        <span class="w-16 shrink-0">{t('layers.blend')}</span>
        <select
          class="opt-field min-w-0 flex-1"
          value={active.blend}
          aria-label={t('layers.blendLabel')}
          onchange={(e) => onBlend(active, e.currentTarget.value)}
        >
          {#each blendModes as m (m.value)}
            <option value={m.value}>{m.label}</option>
          {/each}
        </select>
      </label>
    </div>
  {/if}

  <div class="flex shrink-0 gap-1 text-xs">
    {#if onMergeVisible}
      <button
        type="button"
        class="panel-btn flex-1"
        disabled={rows.length < 2}
        onclick={onMergeVisible}>{t('layers.mergeVisible')}</button
      >
    {/if}
    <button type="button" class="panel-btn flex-1" disabled={rows.length < 2} onclick={onFlatten}
      >{t('layers.flatten')}</button
    >
  </div>
</Panel>

<style>
  .layer-active {
    border-color: var(--color-pc-accent);
    background-color: var(--color-pc-active);
  }
  .drop-above {
    box-shadow: inset 0 2px 0 var(--color-pc-accent);
  }
  .drop-below {
    box-shadow: inset 0 -2px 0 var(--color-pc-accent);
  }
  .drop-into {
    box-shadow: inset 0 0 0 2px var(--color-pc-accent);
  }
  .grip {
    touch-action: none;
  }
  .row-icon {
    display: flex;
    height: 1.75rem;
    width: 1.5rem;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    border-radius: 0.25rem;
    color: var(--color-pc-muted);
  }
  .row-icon:hover {
    color: var(--color-pc-text);
  }
  .row-locked,
  .row-locked:hover {
    color: var(--color-pc-accent);
  }
</style>
