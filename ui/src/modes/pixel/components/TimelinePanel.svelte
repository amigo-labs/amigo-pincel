<script lang="ts">
  import IconButton from '../../../lib/components/layout/IconButton.svelte';
  import Panel from '../../../lib/components/layout/Panel.svelte';
  import { hint } from '../../../lib/shell/shortcuts';
  import type { Document } from '../../../lib/core';
  import { isIdentifierLike, type OnionSettings, type TagDirection } from '../playback';
  import { t } from '../../../lib/i18n/index.svelte';

  // Timeline strip (M14.6 / M14.7, spec §9.2): frames with their
  // durations, tag lanes, playback and onion-skin controls. The wasm
  // `Document` is the source of truth; `rev` is the parent's change
  // counter so the derived rows re-read after opaque wasm mutations.
  // Frame selection, add / remove, playback and onion settings go
  // through the parent (they drive recompose and the rAF loop); duration
  // and tag edits are plain undoable document edits made here, followed
  // by `onChange`.

  let {
    doc,
    rev = 0,
    currentFrame,
    playing,
    loopTag,
    onion,
    onSelectFrame,
    onAddFrame,
    onRemoveFrame,
    onTogglePlay,
    onLoopTagChange,
    onOnionChange,
    onChange,
    onError,
  }: {
    doc: Document | null;
    rev?: number;
    currentFrame: number;
    playing: boolean;
    /** Tag index playback loops over, or `null` for the whole sequence. */
    loopTag: number | null;
    onion: OnionSettings;
    onSelectFrame: (frame: number) => void;
    onAddFrame: () => void;
    onRemoveFrame: (frame: number) => void;
    onTogglePlay: () => void;
    onLoopTagChange: (tag: number | null) => void;
    onOnionChange: (settings: OnionSettings) => void;
    onChange: () => void;
    onError: (message: string) => void;
  } = $props();

  type TagRow = {
    index: number;
    name: string;
    from: number;
    to: number;
    direction: TagDirection;
    color: string;
  };

  const DIRECTIONS: { value: TagDirection; label: string }[] = [
    { value: 'forward', label: t('tl.forward') },
    { value: 'reverse', label: t('tl.reverse') },
    { value: 'pingpong', label: t('tl.pingpong') },
    { value: 'pingpong_reverse', label: t('tl.pingpongReverse') },
  ];

  // Distinct, readable lane colours for new tags (Aseprite-like).
  const TAG_COLORS = [0x5b8def_ff, 0xe0a83c_ff, 0x58b368_ff, 0xd0607a_ff, 0x9a6ad8_ff, 0x3cb7c4_ff];

  const durations = $derived.by<number[]>(() => {
    void rev;
    if (!doc) return [];
    const out: number[] = [];
    for (let i = 0; i < doc.frameCount; i++) out.push(doc.frameDuration(i));
    return out;
  });

  const tags = $derived.by<TagRow[]>(() => {
    void rev;
    if (!doc) return [];
    const out: TagRow[] = [];
    for (let i = 0; i < doc.tagCount; i++) {
      const rgba = doc.tagColor(i) >>> 0;
      out.push({
        index: i,
        name: doc.tagName(i),
        from: doc.tagFrom(i),
        to: doc.tagTo(i),
        direction: doc.tagDirection(i) as TagDirection,
        color: `#${(rgba >>> 8).toString(16).padStart(6, '0')}`,
      });
    }
    return out;
  });

  let editingTag = $state<number | null>(null);
  const editing = $derived(tags.find((tag) => tag.index === editingTag) ?? null);

  function run(label: string, fn: () => void) {
    try {
      fn();
      onChange();
    } catch (err) {
      onError(t('status.failed', { action: label, error: err instanceof Error ? err.message : String(err) }));
    }
  }

  function setDuration(frame: number, value: string) {
    const ms = Math.round(Number(value));
    if (!doc || !Number.isFinite(ms) || ms < 1 || ms > 65535) return;
    const d = doc;
    run(t('tl.setDuration'), () => {
      d.setFrameDuration(frame, ms);
      d.endStroke();
    });
  }

  function addTag() {
    if (!doc) return;
    const d = doc;
    run(t('tl.addTagAction'), () => {
      const color = TAG_COLORS[d.tagCount % TAG_COLORS.length] ?? TAG_COLORS[0]!;
      const index = d.addTag(`tag${d.tagCount + 1}`, currentFrame, currentFrame, color);
      editingTag = index;
    });
  }

  function editTag(label: string, fn: (d: Document, index: number) => void) {
    if (!doc || editingTag === null) return;
    const d = doc;
    const index = editingTag;
    run(label, () => {
      fn(d, index);
      d.endStroke();
    });
  }

  function removeTag(index: number) {
    if (!doc) return;
    const d = doc;
    run(t('tl.deleteTag'), () => {
      d.removeTag(index);
      if (loopTag === index) onLoopTagChange(null);
      else if (loopTag !== null && loopTag > index) onLoopTagChange(loopTag - 1);
      editingTag = null;
    });
  }

  function clampFrame(value: string): number {
    const n = Math.round(Number(value)) - 1;
    return Math.max(0, Math.min(durations.length - 1, Number.isFinite(n) ? n : 0));
  }
</script>

<Panel keepActions bodyClass="flex flex-col gap-1.5 px-3 pb-2 text-xs text-pc-text">
  {#snippet actions()}
    <IconButton
      icon={playing ? 'pause' : 'play'}
      label={playing ? t('menu.pause') : t('menu.play')}
      hint={hint('togglePlay')}
      size="sm"
      onclick={onTogglePlay}
      disabled={!doc || durations.length < 2}
    />
    <IconButton
      icon="chevron-left"
      label={t('tl.prevFrame')}
      hint={hint('prevFrame')}
      size="sm"
      onclick={() => onSelectFrame(currentFrame - 1)}
      disabled={!doc || currentFrame === 0}
    />
    <span class="min-w-16 text-center text-xs tabular-nums text-pc-muted" aria-live="polite"
      >{t('tl.frameOf', { n: currentFrame + 1, total: durations.length })}</span
    >
    <IconButton
      icon="chevron-right"
      label={t('tl.nextFrame')}
      hint={hint('nextFrame')}
      size="sm"
      onclick={() => onSelectFrame(currentFrame + 1)}
      disabled={!doc || currentFrame >= durations.length - 1}
    />
    <span class="mx-1 h-4 w-px bg-pc-border"></span>
    <IconButton icon="plus" label={t('tl.addFrame')} size="sm" onclick={onAddFrame} disabled={!doc} />
    <IconButton
      icon="minus"
      label={t('tl.removeFrame')}
      size="sm"
      onclick={() => onRemoveFrame(currentFrame)}
      disabled={!doc || durations.length < 2}
    />
  {/snippet}

  <div class="flex flex-wrap items-center gap-2">
    <label class="flex items-center gap-1" title={t('tl.loopHint')}>
      {t('tl.loop')}
      <select
        class="rounded border border-pc-border bg-pc-panel px-1"
        value={loopTag === null ? '' : String(loopTag)}
        onchange={(e) => {
          const v = e.currentTarget.value;
          onLoopTagChange(v === '' ? null : Number(v));
        }}
        aria-label={t('tl.loopRange')}
      >
        <option value="">{t('tl.allFrames')}</option>
        {#each tags as tag (tag.index)}
          <option value={String(tag.index)}>{tag.name}</option>
        {/each}
      </select>
    </label>
    <button
      class="panel-btn"
      onclick={addTag}
      disabled={!doc}
      aria-label={t('tl.addTag')}
      title={t('tl.addTagHint')}>{t('tl.plusTag')}</button
    >
    <span class="mx-1 h-4 w-px bg-pc-border"></span>
    <label class="flex items-center gap-1">
      <input
        type="checkbox"
        checked={onion.enabled}
        onchange={(e) => onOnionChange({ ...onion, enabled: e.currentTarget.checked })}
        aria-label={t('tl.onionSkin')}
      />
      {t('tl.onion')}
    </label>
    <label class="flex items-center gap-1" title={t('tl.onionBackHint')}>
      ◀
      <input
        type="number"
        min="0"
        max="8"
        class="w-10 rounded border border-pc-border bg-pc-panel px-1"
        value={onion.back}
        disabled={!onion.enabled}
        onchange={(e) =>
          onOnionChange({
            ...onion,
            back: Math.max(0, Math.min(8, Number(e.currentTarget.value) || 0)),
          })}
        aria-label={t('tl.onionBack')}
      />
    </label>
    <label class="flex items-center gap-1" title={t('tl.onionFwdHint')}>
      <input
        type="number"
        min="0"
        max="8"
        class="w-10 rounded border border-pc-border bg-pc-panel px-1"
        value={onion.forward}
        disabled={!onion.enabled}
        onchange={(e) =>
          onOnionChange({
            ...onion,
            forward: Math.max(0, Math.min(8, Number(e.currentTarget.value) || 0)),
          })}
        aria-label={t('tl.onionFwd')}
      />
      ▶
    </label>
    <label class="flex items-center gap-1" title={t('tl.onionOpacityHint')}>
      <input
        type="range"
        min="0"
        max="255"
        value={onion.alpha}
        disabled={!onion.enabled}
        oninput={(e) => onOnionChange({ ...onion, alpha: Number(e.currentTarget.value) })}
        aria-label={t('tl.onionOpacity')}
      />
    </label>
  </div>

  <div class="overflow-x-auto">
    <div class="grid gap-y-1" style:grid-template-columns="repeat({durations.length}, 3.5rem)">
      {#each durations as ms, i (i)}
        <div
          class="frame-cell"
          class:frame-current={i === currentFrame}
          style:grid-row="1"
          style:grid-column={i + 1}
        >
          <button
            class="w-full text-center font-semibold tabular-nums"
            onclick={() => onSelectFrame(i)}
            aria-label={t('tl.frame', { n: i + 1 })}
            aria-current={i === currentFrame ? 'true' : undefined}
          >
            {i + 1}
          </button>
          <input
            type="number"
            min="1"
            max="65535"
            class="w-full rounded border border-pc-border bg-pc-panel px-0.5 text-center tabular-nums"
            value={ms}
            onchange={(e) => setDuration(i, e.currentTarget.value)}
            aria-label={t('tl.frameDuration', { n: i + 1 })}
            title={t('tl.duration')}
          />
        </div>
      {/each}
      {#each tags as tag, lane (tag.index)}
        <button
          class="tag-bar"
          class:tag-editing={editingTag === tag.index}
          style:grid-row={lane + 2}
          style:grid-column="{tag.from + 1} / {tag.to + 2}"
          style:border-color={tag.color}
          style:background-color="{tag.color}33"
          onclick={() => (editingTag = editingTag === tag.index ? null : tag.index)}
          aria-label={t('tl.tag', { name: tag.name })}
          title={t('tl.tagTitle', { name: tag.name, from: tag.from + 1, to: tag.to + 1, direction: tag.direction })}
        >
          {tag.name}{#if !isIdentifierLike(tag.name)}<span
              class="text-amber-400"
              title={t('tl.notIdent')}
            >
              ⚠</span
            >{/if}
        </button>
      {/each}
    </div>
  </div>

  {#if editing}
    <div
      class="flex flex-wrap items-center gap-2 rounded border border-pc-border p-1.5"
      aria-label={t('tl.tagEditor')}
    >
      <label class="flex items-center gap-1">
        {t('layers.name')}
        <input
          class="w-28 rounded border border-pc-border bg-pc-panel px-1"
          value={editing.name}
          onchange={(e) => {
            const name = e.currentTarget.value.trim();
            if (name) editTag(t('tl.renameTag'), (d, i) => d.setTagName(i, name));
          }}
          aria-label={t('tl.tagName')}
        />
      </label>
      {#if !isIdentifierLike(editing.name)}
        <span class="text-amber-400">
          {t('tl.identHint')}
        </span>
      {/if}
      <label class="flex items-center gap-1">
        {t('tl.from')}
        <input
          type="number"
          min="1"
          max={durations.length}
          class="w-12 rounded border border-pc-border bg-pc-panel px-1"
          value={editing.from + 1}
          onchange={(e) => {
            const from = clampFrame(e.currentTarget.value);
            const to = Math.max(from, editing.to);
            editTag(t('tl.setTagRange'), (d, i) => d.setTagRange(i, from, to));
          }}
          aria-label={t('tl.tagFrom')}
        />
      </label>
      <label class="flex items-center gap-1">
        {t('tl.to')}
        <input
          type="number"
          min="1"
          max={durations.length}
          class="w-12 rounded border border-pc-border bg-pc-panel px-1"
          value={editing.to + 1}
          onchange={(e) => {
            const to = clampFrame(e.currentTarget.value);
            const from = Math.min(editing.from, to);
            editTag(t('tl.setTagRange'), (d, i) => d.setTagRange(i, from, to));
          }}
          aria-label={t('tl.tagTo')}
        />
      </label>
      <label class="flex items-center gap-1">
        {t('tl.direction')}
        <select
          class="rounded border border-pc-border bg-pc-panel px-1"
          value={editing.direction}
          onchange={(e) => {
            const dir = e.currentTarget.value;
            editTag(t('tl.setTagDirection'), (d, i) => d.setTagDirection(i, dir));
          }}
          aria-label={t('tl.tagDirection')}
        >
          {#each DIRECTIONS as dir (dir.value)}
            <option value={dir.value}>{dir.label}</option>
          {/each}
        </select>
      </label>
      <button class="panel-btn" onclick={() => removeTag(editing.index)} aria-label={t('tl.deleteTag')}
        >{t('tl.delete')}</button
      >
    </div>
  {/if}
</Panel>

<style>
  .frame-cell {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    margin-right: 0.25rem;
    border-radius: 0.25rem;
    border: 1px solid var(--color-pc-border);
    padding: 0.125rem;
  }
  .frame-current {
    border-color: var(--color-pc-accent);
    background-color: var(--color-pc-active);
  }
  .tag-bar {
    margin-right: 0.25rem;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    border-radius: 0.25rem;
    border-width: 1px;
    padding: 0 0.25rem;
    text-align: left;
  }
  .tag-editing {
    outline: 1px solid rgb(229 229 229);
  }
</style>
