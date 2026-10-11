<script lang="ts">
  // Export Sprite Sheet dialog (spec §7.3). Presentational: the parent
  // runs `exportAtlas` from `onSubmit`. Frames pack left-to-right,
  // top-to-bottom into a grid of `columns` cells; `tag` restricts the
  // sheet to one animation; `data` also writes the Aseprite `--data`
  // JSON sidecar (durations, tags, slices).
  export interface SpriteSheetOptions {
    columns: number;
    /** Tag name, or `null` for every frame. */
    tag: string | null;
    data: boolean;
  }

  let {
    frameCount,
    tags,
    frameWidth,
    frameHeight,
    onSubmit,
    onCancel,
  }: {
    frameCount: number;
    tags: { name: string; from: number; to: number }[];
    frameWidth: number;
    frameHeight: number;
    onSubmit: (options: SpriteSheetOptions) => void;
    onCancel: () => void;
  } = $props();

  // Remounted per open, so the initial props are the only ones seen.
  // svelte-ignore state_referenced_locally
  let columns = $state(Math.max(1, Math.ceil(Math.sqrt(frameCount))));
  let tag = $state<string | null>(null);
  let data = $state(true);

  const count = $derived.by(() => {
    const t = tags.find((x) => x.name === tag);
    return t ? t.to - t.from + 1 : frameCount;
  });
  const cols = $derived(Math.max(1, Math.min(count, Math.floor(columns) || 1)));
  const rows = $derived(Math.ceil(count / cols));

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label="Export sprite sheet"
>
  <form
    class="flex w-80 flex-col gap-3 rounded border border-neutral-700 bg-neutral-900 p-4 text-sm text-neutral-100 shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      onSubmit({ columns: cols, tag, data });
    }}
  >
    <h2 class="text-base font-semibold">Export sprite sheet</h2>
    <label class="flex flex-col gap-1 text-xs text-neutral-300">
      <span>Frames</span>
      <select
        class="rounded border border-neutral-700 bg-neutral-950 px-2 py-1"
        value={tag ?? ''}
        onchange={(e) => (tag = e.currentTarget.value === '' ? null : e.currentTarget.value)}
        aria-label="Frames"
      >
        <option value="">All frames ({frameCount})</option>
        {#each tags as t (t.name)}
          <option value={t.name}>{t.name} ({t.to - t.from + 1})</option>
        {/each}
      </select>
    </label>
    <label class="flex flex-col gap-1 text-xs text-neutral-300">
      <span>Columns</span>
      <input
        type="number"
        min="1"
        max={count}
        bind:value={columns}
        class="rounded border border-neutral-700 bg-neutral-950 px-2 py-1 tabular-nums"
        aria-label="Columns"
      />
    </label>
    <p class="text-xs text-neutral-400" aria-live="polite">
      {cols} × {rows} cells · {cols * frameWidth}×{rows * frameHeight} px
    </p>
    <label class="flex items-center gap-2 text-xs text-neutral-300">
      <input type="checkbox" bind:checked={data} aria-label="Write JSON data" />
      Also write JSON data (Aseprite format: durations, tags, slices)
    </label>
    <div class="flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>Cancel</button>
      <button type="submit" class="panel-btn panel-btn-primary">Export</button>
    </div>
  </form>
</div>

<style>
  .panel-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.25rem 0.75rem;
  }
  .panel-btn:hover {
    background-color: rgb(38 38 38);
  }
  .panel-btn-primary {
    border-color: rgb(59 130 246);
    background-color: rgb(29 78 216);
  }
</style>
