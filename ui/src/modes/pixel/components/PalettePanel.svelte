<script lang="ts">
  import { t } from '../../../lib/i18n/index.svelte';
  import Panel from '../../../lib/components/layout/Panel.svelte';
  import IconButton from '../../../lib/components/layout/IconButton.svelte';
  import { packColor, unpackColor } from '../../../lib/color';
  import type { Document } from '../../../lib/core';

  // Palette panel (spec §6 panel layout). The wasm `Document` is the
  // source of truth; the panel reads the palette through the read
  // surface (`paletteCount`, `paletteColor`, `paletteName`) behind the
  // parent-managed `rev` change counter (bumped on new / open / undo /
  // redo / any edit) so the `$derived.by` block re-evaluates against the
  // opaque wasm getters.
  //
  // Palettes are recovered from the source file on open (T3); a fresh
  // document starts with DB32 (spec §3.7). Clicking a swatch calls
  // `onPick` with the packed `0xRRGGBBAA` color (the parent sets the
  // foreground from it) and selects the entry for editing: recolour it,
  // move it left / right (or drag it), or delete it; `+` appends the
  // current foreground (`addColor`). Edits are undoable wasm commands;
  // `onChange` bumps the parent's `rev`. `activeColor` (the current
  // foreground as `#RRGGBB`) highlights the matching swatch, if any.
  // Right-clicking a swatch calls `onPickBg` (background color).
  let {
    doc,
    rev = 0,
    activeColor = null,
    addColor = 0x000000ff,
    onPick,
    onPickBg,
    onChange,
    onError,
  }: {
    doc: Document | null;
    rev?: number;
    activeColor?: string | null;
    /** Packed `0xRRGGBBAA` appended by `+`. */
    addColor?: number;
    onPick?: (packed: number) => void;
    onPickBg?: (packed: number) => void;
    onChange?: () => void;
    onError?: (message: string) => void;
  } = $props();

  let selected = $state<number | null>(null);
  let dragFrom: number | null = null;

  function edit(label: string, fn: (d: Document) => void) {
    if (!doc) return;
    try {
      fn(doc);
      onChange?.();
    } catch (err) {
      onError?.(t('status.failed', { action: label, error: err instanceof Error ? err.message : String(err) }));
    }
  }

  function move(from: number, to: number) {
    if (from === to || to < 0 || to >= swatches.length) return;
    edit(t('palette.move'), (d) => d.movePaletteEntry(from, to));
    selected = to;
  }

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
  const current = $derived(swatches.find((s) => s.index === selected) ?? null);
</script>

<Panel count={swatches.length > 0 ? swatches.length : undefined}>
  {#snippet actions()}
    <IconButton
      icon="plus"
      label={t('palette.add')}
      size="sm"
      disabled={!doc || swatches.length >= 256}
      onclick={() =>
        edit(t('palette.addAction'), (d) => {
          selected = d.addPaletteEntry(addColor);
        })}
    />
  {/snippet}

  {#if swatches.length === 0}
    <p class="text-xs text-pc-faint">{t('palette.empty')}</p>
  {:else}
    <div class="palette-grid" role="group" aria-label={t('palette.colors')}>
      {#each swatches as s (s.index)}
        {@const isActive = activeCss !== null && s.css.toLowerCase() === activeCss}
        <button
          type="button"
          class="palette-swatch"
          class:swatch-active={isActive}
          class:swatch-selected={selected === s.index}
          style:background-color={s.css}
          draggable="true"
          ondragstart={() => (dragFrom = s.index)}
          ondragover={(e) => e.preventDefault()}
          ondrop={(e) => {
            e.preventDefault();
            if (dragFrom !== null) move(dragFrom, s.index);
            dragFrom = null;
          }}
          onclick={() => {
            selected = s.index;
            onPick?.(s.packed);
          }}
          oncontextmenu={(e) => {
            e.preventDefault();
            onPickBg?.(s.packed);
          }}
          aria-pressed={isActive}
          aria-label={t('palette.pick', { name: s.name || s.css, index: s.index })}
          title={s.name
            ? t('palette.pickTitleNamed', { name: s.name, css: s.css })
            : t('palette.pickTitle', { css: s.css, index: s.index })}
        ></button>
      {/each}
    </div>
    {#if current}
      <div
        class="flex items-center gap-2 text-xs text-pc-muted"
        aria-label={t('palette.editor')}
      >
        <span class="tabular-nums">#{current.index}</span>
        <input
          type="color"
          value={current.css}
          oninput={(e) => {
            const packed = packColor(e.currentTarget.value, current.packed & 0xff);
            edit(t('palette.recolor'), (d) => d.setPaletteColor(current.index, packed));
          }}
          onchange={() => doc?.endStroke()}
          aria-label={t('palette.entryColor')}
        />
        <button
          type="button"
          class="pal-btn"
          disabled={current.index === 0}
          onclick={() => move(current.index, current.index - 1)}
          aria-label={t('palette.left')}>◀</button
        >
        <button
          type="button"
          class="pal-btn"
          disabled={current.index >= swatches.length - 1}
          onclick={() => move(current.index, current.index + 1)}
          aria-label={t('palette.right')}>▶</button
        >
        <button
          type="button"
          class="pal-btn"
          onclick={() => {
            const index = current.index;
            edit(t('palette.removeAction'), (d) => d.removePaletteEntry(index));
            selected = null;
          }}
          aria-label={t('palette.delete')}>{t('tl.delete')}</button
        >
      </div>
    {/if}
  {/if}
</Panel>

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
  .swatch-selected {
    border-color: rgb(229 229 229);
    border-width: 2px;
  }
  .pal-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0 0.4rem;
    font-size: 0.75rem;
  }
  .pal-btn:hover:not(:disabled) {
    background-color: rgb(38 38 38);
  }
  .pal-btn:disabled {
    opacity: 0.4;
  }
  .swatch-active {
    outline: 2px solid rgb(59 130 246);
    outline-offset: 1px;
    border-color: rgb(59 130 246);
  }
</style>
