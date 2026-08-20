<script lang="ts">
  import { unpackColor } from '../color';
  import type { Document } from '../core';

  // Palette panel (spec §6 panel layout). The wasm `Document` is the
  // source of truth; the panel reads the palette through the read
  // surface (`paletteCount`, `paletteColor`, `paletteName`) behind the
  // parent-managed `rev` change counter (bumped on new / open / undo /
  // redo / any edit) so the `$derived.by` block re-evaluates against the
  // opaque wasm getters.
  //
  // Palettes are recovered from the source file on open (T3); a fresh
  // `Document::new` seeds none, so a new document shows the empty state
  // until a file with a palette is opened. Clicking a swatch calls
  // `onPick` with the packed `0xRRGGBBAA` color; the parent sets the
  // foreground color from it. `activeColor` (the current foreground as
  // `#RRGGBB`) highlights the matching swatch, if any.
  let {
    doc,
    rev = 0,
    activeColor = null,
    onPick,
  }: {
    doc: Document | null;
    rev?: number;
    activeColor?: string | null;
    onPick?: (packed: number) => void;
  } = $props();

  type Swatch = {
    index: number;
    packed: number;
    name: string;
    css: string;
  };

  const swatches = $derived.by<Swatch[]>(() => {
    void rev;
    if (!doc) return [];
    const list: Swatch[] = [];
    const count = doc.paletteCount;
    for (let i = 0; i < count; i += 1) {
      let packed: number;
      try {
        packed = doc.paletteColor(i);
      } catch {
        continue;
      }
      let name = '';
      try {
        name = doc.paletteName(i);
      } catch {
        // Out of range can't happen for `i < paletteCount`; ignore.
      }
      list.push({ index: i, packed, name, css: unpackColor(packed) });
    }
    return list;
  });

  // Alpha is dropped by `unpackColor` (the color picker has no alpha
  // control), so highlight-matching compares `#RRGGBB` only. Both sides
  // are lowercased to stay case-insensitive.
  const activeCss = $derived(activeColor?.toLowerCase() ?? null);
</script>

<aside
  class="flex w-64 shrink-0 flex-col gap-2 border-l border-neutral-800 bg-neutral-950 p-3 text-sm"
  aria-label="Palette"
>
  <header class="flex items-center justify-between">
    <h2 class="text-xs font-semibold tracking-wide text-neutral-300 uppercase">Palette</h2>
    {#if swatches.length > 0}
      <span class="text-xs text-neutral-500">{swatches.length}</span>
    {/if}
  </header>

  {#if swatches.length === 0}
    <p class="text-xs text-neutral-500">No palette. Open a file with a palette to see its colors.</p>
  {:else}
    <div class="palette-grid" role="group" aria-label="Palette colors">
      {#each swatches as s (s.index)}
        {@const isActive = activeCss !== null && s.css.toLowerCase() === activeCss}
        <button
          type="button"
          class="palette-swatch"
          class:swatch-active={isActive}
          style:background-color={s.css}
          onclick={() => onPick?.(s.packed)}
          aria-pressed={isActive}
          aria-label={`Pick color ${s.name || s.css} (entry ${s.index})`}
          title={s.name ? `${s.name} — ${s.css}` : `${s.css} (entry ${s.index})`}
        ></button>
      {/each}
    </div>
  {/if}
</aside>

<style>
  .palette-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(1.25rem, 1fr));
    gap: 0.25rem;
  }
  .palette-swatch {
    aspect-ratio: 1;
    width: 100%;
    border-radius: 0.2rem;
    border: 1px solid rgb(64 64 64);
    cursor: pointer;
  }
  .palette-swatch:hover {
    border-color: rgb(163 163 163);
  }
  .swatch-active {
    outline: 2px solid rgb(59 130 246);
    outline-offset: 1px;
    border-color: rgb(59 130 246);
  }
</style>
