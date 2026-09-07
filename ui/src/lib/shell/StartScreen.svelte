<script lang="ts">
  // Landing view of the two-mode shell: pick an editor (new pixel sprite /
  // new image), open a file, or re-open a recent one. Dropping a file
  // anywhere on the screen opens it. See docs/specs/pincel.md §9.2.
  import type { RecentFile } from '../idb/recent-files';
  import type { EditorMode } from './types';

  let {
    recents,
    busy = false,
    onNew,
    onOpen,
    onOpenRecent,
    onDropFile,
  }: {
    recents: RecentFile[];
    busy?: boolean;
    onNew: (mode: EditorMode) => void;
    onOpen: () => void;
    onOpenRecent: (recent: RecentFile) => void;
    onDropFile: (file: File) => void;
  } = $props();

  let dragOver = $state(false);

  function onDragOver(e: DragEvent) {
    if (!e.dataTransfer?.types.includes('Files')) return;
    e.preventDefault();
    dragOver = true;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
    const file = e.dataTransfer?.files[0];
    if (file) onDropFile(file);
  }

  const modeLabel: Record<EditorMode, string> = { pixel: 'Pixel', image: 'Image' };
</script>

<main
  class="flex h-full flex-col items-center justify-center bg-neutral-950 p-6 text-neutral-100"
  class:bg-neutral-900={dragOver}
  ondragover={onDragOver}
  ondragleave={() => (dragOver = false)}
  ondrop={onDrop}
>
  <section
    class="w-full max-w-xl rounded border border-neutral-800 bg-neutral-900 p-6 shadow-xl"
    aria-labelledby="start-title"
  >
    <h1 id="start-title" class="text-2xl font-semibold tracking-wide">Pincel</h1>
    <p class="mt-1 text-sm text-neutral-400">
      Pixel-art sprites and everyday image editing, one app. Drop a file here to open it.
    </p>

    <div class="mt-6 grid grid-cols-1 gap-3 sm:grid-cols-2">
      <button
        class="start-card"
        onclick={() => onNew('pixel')}
        disabled={busy}
        aria-label="New pixel sprite"
      >
        <span class="text-base font-semibold">New pixel sprite</span>
        <span class="text-xs text-neutral-400">
          Frames, palettes, tilemaps, slices. Saves as .aseprite.
        </span>
      </button>
      <button
        class="start-card"
        onclick={() => onNew('image')}
        disabled={busy}
        aria-label="New image"
      >
        <span class="text-base font-semibold">New image</span>
        <span class="text-xs text-neutral-400">
          Layers, soft brushes, selections, effects. Exports PNG / JPEG / WebP.
        </span>
      </button>
    </div>

    <div class="mt-4 flex items-center gap-2">
      <button class="toolbar-btn" onclick={onOpen} disabled={busy}>Open…</button>
      <span class="text-xs text-neutral-500">
        .aseprite opens in Pixel mode; PNG, JPEG, WebP, BMP, GIF and TIFF open in Image mode.
      </span>
    </div>

    {#if recents.length > 0}
      <h2 class="mt-6 text-xs font-semibold uppercase tracking-wide text-neutral-500">Recent</h2>
      <ul class="mt-2 flex flex-col divide-y divide-neutral-800" aria-label="Recent files">
        {#each recents as r (r.id)}
          <li>
            <button
              class="flex w-full items-center justify-between gap-3 px-2 py-1.5 text-left text-sm hover:bg-neutral-800 disabled:opacity-50"
              onclick={() => onOpenRecent(r)}
              disabled={busy}
              title={r.path ?? r.name}
            >
              <span class="truncate">{r.name}</span>
              <span
                class="shrink-0 rounded border border-neutral-700 px-1.5 text-[10px] uppercase text-neutral-400"
              >
                {modeLabel[r.mode]}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</main>

<style>
  .start-card {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    border-radius: 0.375rem;
    border: 1px solid rgb(64 64 64);
    padding: 1rem;
    text-align: left;
  }
  .start-card:hover:not(:disabled) {
    background-color: rgb(38 38 38);
    border-color: rgb(115 115 115);
  }
  .start-card:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .toolbar-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.125rem 0.5rem;
    font-size: 0.75rem;
  }
  .toolbar-btn:hover:not(:disabled) {
    background-color: rgb(38 38 38);
  }
  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
