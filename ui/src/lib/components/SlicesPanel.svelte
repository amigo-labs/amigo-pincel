<script lang="ts">
  import type { Document } from '../core';

  // The wasm `Document` is the source of truth for slice state. The
  // panel reads through the M9.4 surface (`sliceCount`, `sliceIdAt`,
  // `sliceName`, `sliceColor`, `sliceKey*`) and emits `addSlice`,
  // `removeSlice`, `setSliceKey`. `rev` is a parent-managed change
  // counter that bumps whenever wasm may have mutated slices (new /
  // open document, undo, redo, or any edit) so the `$derived.by`
  // block re-evaluates against opaque wasm getters.
  //
  // `activeSliceId` highlights the currently selected slice — the
  // parent owns the state so the canvas-side marching-ants overlay
  // can read it without the panel re-broadcasting. `onActivate`
  // fires when a slice row is clicked. The panel itself only edits
  // frame-0 keys (multi-frame slice editing is deferred — STATUS.md
  // open question — and the codec round-trip already covers multi-
  // frame slices through `setSliceKey`).
  let {
    doc,
    rev = 0,
    activeSliceId = null,
    onChange,
    onActivate,
  }: {
    doc: Document | null;
    rev?: number;
    activeSliceId?: number | null;
    onChange?: () => void;
    onActivate?: (sliceId: number | null) => void;
  } = $props();

  type SliceRow = {
    id: number;
    name: string;
    color: number;
    keyCount: number;
    // Frame-0 key payload, surfaced flat for the form. `keyIndex` is
    // the position of the frame-0 key inside `Slice::keys` — when no
    // such key exists yet the row falls back to key 0 (the codec
    // guarantees at least one key per slice).
    keyIndex: number;
    x: number;
    y: number;
    w: number;
    h: number;
    hasCenter: boolean;
    cx: number;
    cy: number;
    cw: number;
    ch: number;
    hasPivot: boolean;
    px: number;
    py: number;
  };

  const slices = $derived.by<SliceRow[]>(() => {
    void rev;
    if (!doc) return [];
    const list: SliceRow[] = [];
    const count = doc.sliceCount;
    for (let i = 0; i < count; i += 1) {
      let id: number;
      try {
        id = doc.sliceIdAt(i);
      } catch {
        continue;
      }
      const keyCount = doc.sliceKeyCount(id);
      let keyIndex = 0;
      for (let k = 0; k < keyCount; k += 1) {
        try {
          if (doc.sliceKeyFrame(id, k) === 0) {
            keyIndex = k;
            break;
          }
        } catch {
          // Ignore — the loop falls back to key 0 below.
        }
      }
      try {
        list.push({
          id,
          name: doc.sliceName(id),
          color: doc.sliceColor(id),
          keyCount,
          keyIndex,
          x: doc.sliceKeyX(id, keyIndex),
          y: doc.sliceKeyY(id, keyIndex),
          w: doc.sliceKeyWidth(id, keyIndex),
          h: doc.sliceKeyHeight(id, keyIndex),
          hasCenter: doc.sliceKeyHasCenter(id, keyIndex),
          cx: doc.sliceKeyCenterX(id, keyIndex),
          cy: doc.sliceKeyCenterY(id, keyIndex),
          cw: doc.sliceKeyCenterWidth(id, keyIndex),
          ch: doc.sliceKeyCenterHeight(id, keyIndex),
          hasPivot: doc.sliceKeyHasPivot(id, keyIndex),
          px: doc.sliceKeyPivotX(id, keyIndex),
          py: doc.sliceKeyPivotY(id, keyIndex),
        });
      } catch {
        continue;
      }
    }
    return list;
  });

  let formOpen = $state(false);
  let formName = $state('Slice');
  let formColor = $state('#3b82f6');
  let formError = $state<string | null>(null);
  // Per-slice transient error surface, keyed by slice id. Cleared on
  // the next mutation that targets the same slice.
  let rowError = $state<Record<number, string | null>>({});

  function openForm() {
    formOpen = true;
    formError = null;
  }

  function closeForm() {
    formOpen = false;
    formError = null;
  }

  // `<input type="color">` returns `#RRGGBB`; the wasm surface wants
  // a packed `0xRRGGBBAA`. Mirrors `App.svelte::packColor`.
  function packHex(hex: string, alpha = 0xff): number {
    const rgb = Number.parseInt(hex.slice(1), 16);
    return ((rgb << 8) | (alpha & 0xff)) >>> 0;
  }

  // Inverse of `packHex` — drops the alpha channel to render in a
  // color input. The overlay alpha is preserved on the wasm side.
  function unpackHex(rgba: number): string {
    const rgb = (rgba >>> 8) & 0xffffff;
    return '#' + rgb.toString(16).padStart(6, '0');
  }

  function submitForm(e: SubmitEvent) {
    e.preventDefault();
    if (!doc) return;
    const name = formName.trim();
    if (name.length === 0) {
      formError = 'name is required';
      return;
    }
    const w = doc.width || 16;
    const h = doc.height || 16;
    try {
      const id = doc.addSlice(name, 0, 0, w, h, packHex(formColor));
      onActivate?.(id);
    } catch (err) {
      formError = err instanceof Error ? err.message : String(err);
      return;
    }
    formOpen = false;
    formError = null;
    formName = 'Slice';
    onChange?.();
  }

  function removeSlice(id: number) {
    if (!doc) return;
    rowError[id] = null;
    try {
      doc.removeSlice(id);
    } catch (err) {
      rowError[id] = err instanceof Error ? err.message : String(err);
      return;
    }
    if (activeSliceId === id) onActivate?.(null);
    onChange?.();
  }

  function activate(id: number) {
    onActivate?.(activeSliceId === id ? null : id);
  }

  // Apply the row's current local state as a `setSliceKey` call. The
  // form binds directly to the derived `SliceRow`, so this collapses
  // the per-field "commit on blur" pattern into a single call — the
  // user adjusts the inputs, the row recomputes from wasm on rev
  // bump, and any partial value rejects from the wasm side rather
  // than from local state.
  function commit(row: SliceRow) {
    if (!doc) return;
    rowError[row.id] = null;
    try {
      doc.setSliceKey(
        row.id,
        0,
        row.x,
        row.y,
        row.w,
        row.h,
        row.hasCenter ? row.cx : undefined,
        row.hasCenter ? row.cy : undefined,
        row.hasCenter ? row.cw : undefined,
        row.hasCenter ? row.ch : undefined,
        row.hasPivot ? row.px : undefined,
        row.hasPivot ? row.py : undefined,
      );
    } catch (err) {
      rowError[row.id] = err instanceof Error ? err.message : String(err);
      return;
    }
    onChange?.();
  }

  // Fields the wasm surface stores as `u32`. JS numbers cross into
  // wasm-bindgen as raw bit patterns, so a negative input would wrap
  // to a huge unsigned and produce an invalid (often massive) rect.
  // Gate the unsigned fields here and surface the rejection as a row
  // error rather than calling wasm with a wrapped value.
  const UNSIGNED_FIELDS = new Set<keyof SliceRow>(['w', 'h', 'cw', 'ch']);

  function onNumberCommit(row: SliceRow, field: keyof SliceRow, value: number) {
    if (!Number.isFinite(value)) return;
    const truncated = Math.trunc(value);
    if (UNSIGNED_FIELDS.has(field) && truncated < 1) {
      rowError[row.id] = `${String(field)} must be a positive integer`;
      return;
    }
    rowError[row.id] = null;
    (row as Record<string, unknown>)[field as string] = truncated;
    commit(row);
  }

  function toggleCenter(row: SliceRow, on: boolean) {
    row.hasCenter = on;
    if (on) {
      // Seed a reasonable inset (1-pixel inset on each side, clamped
      // to a 1x1 minimum) so the user has a starting 9-patch they can
      // refine — typing into an empty quartet otherwise rejects from
      // the wasm "partial center" guard and the row sits in an error
      // state until all four fields are filled.
      if (row.cw === 0 || row.ch === 0) {
        row.cx = row.x + 1;
        row.cy = row.y + 1;
        row.cw = Math.max(1, row.w - 2);
        row.ch = Math.max(1, row.h - 2);
      }
    }
    commit(row);
  }

  function togglePivot(row: SliceRow, on: boolean) {
    row.hasPivot = on;
    if (on) {
      // Default the pivot to the slice's top-left corner; users
      // recentering to the bbox center is the common follow-up edit.
      // Aseprite signs the pivot, so negatives are valid.
      row.px = row.x;
      row.py = row.y;
    }
    commit(row);
  }
</script>

<aside
  class="flex w-64 shrink-0 flex-col gap-2 border-l border-neutral-800 bg-neutral-950 p-3 text-sm"
  aria-label="Slices"
>
  <header class="flex items-center justify-between">
    <h2 class="text-xs font-semibold tracking-wide text-neutral-300 uppercase">
      Slices
    </h2>
    {#if !formOpen}
      <button
        type="button"
        class="panel-btn"
        onclick={openForm}
        disabled={!doc}
        aria-label="Add slice"
      >
        + Add
      </button>
    {/if}
  </header>

  {#if formOpen}
    <form class="flex flex-col gap-2 rounded border border-neutral-800 p-2" onsubmit={submitForm}>
      <label class="flex flex-col gap-1 text-xs text-neutral-400">
        <span>Name</span>
        <input
          type="text"
          bind:value={formName}
          class="rounded border border-neutral-700 bg-neutral-900 px-2 py-1 text-sm text-neutral-100"
        />
      </label>
      <label class="flex items-center gap-2 text-xs text-neutral-400">
        <span>Color</span>
        <input
          type="color"
          bind:value={formColor}
          class="h-6 w-8 cursor-pointer rounded border border-neutral-700 bg-transparent"
        />
      </label>
      {#if formError}
        <p class="text-xs text-red-400" role="alert">{formError}</p>
      {/if}
      <div class="flex justify-end gap-1">
        <button type="button" class="panel-btn" onclick={closeForm}>Cancel</button>
        <button type="submit" class="panel-btn panel-btn-primary">Add</button>
      </div>
    </form>
  {/if}

  {#if slices.length === 0}
    <p class="text-xs text-neutral-500">
      {doc ? 'no slices yet' : 'open or create a document'}
    </p>
  {:else}
    <ul class="flex flex-col gap-1">
      {#each slices as s (s.id)}
        {@const active = activeSliceId === s.id}
        <li
          class="flex flex-col gap-1 rounded border px-2 py-1"
          class:border-neutral-800={!active}
          class:border-blue-500={active}
        >
          <div class="flex items-center gap-2">
            <button
              type="button"
              class="color-swatch shrink-0"
              style:background-color={unpackHex(s.color)}
              onclick={() => activate(s.id)}
              aria-label={`Activate slice ${s.name}`}
              aria-pressed={active}
            ></button>
            <button
              type="button"
              class="flex-1 truncate text-left text-sm text-neutral-100"
              onclick={() => activate(s.id)}
              title={s.name}
            >
              {s.name}
            </button>
            <button
              type="button"
              class="panel-btn"
              onclick={() => removeSlice(s.id)}
              aria-label={`Remove ${s.name}`}
              title="Remove slice"
            >
              ×
            </button>
          </div>
          <span class="text-xs text-neutral-500">
            id {s.id} · {s.keyCount} key{s.keyCount === 1 ? '' : 's'} ·
            frame 0
          </span>

          <fieldset class="flex flex-col gap-1">
            <legend class="sr-only">Bounds for slice {s.name}</legend>
            <div class="flex gap-1">
              <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                <span>X</span>
                <input
                  type="number"
                  step="1"
                  inputmode="numeric"
                  value={s.x}
                  onchange={(e) =>
                    onNumberCommit(s, 'x', Number((e.target as HTMLInputElement).value))}
                  class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                />
              </label>
              <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                <span>Y</span>
                <input
                  type="number"
                  step="1"
                  inputmode="numeric"
                  value={s.y}
                  onchange={(e) =>
                    onNumberCommit(s, 'y', Number((e.target as HTMLInputElement).value))}
                  class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                />
              </label>
            </div>
            <div class="flex gap-1">
              <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                <span>W</span>
                <input
                  type="number"
                  min="1"
                  step="1"
                  inputmode="numeric"
                  value={s.w}
                  onchange={(e) =>
                    onNumberCommit(s, 'w', Number((e.target as HTMLInputElement).value))}
                  class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                />
              </label>
              <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                <span>H</span>
                <input
                  type="number"
                  min="1"
                  step="1"
                  inputmode="numeric"
                  value={s.h}
                  onchange={(e) =>
                    onNumberCommit(s, 'h', Number((e.target as HTMLInputElement).value))}
                  class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                />
              </label>
            </div>
          </fieldset>

          <label class="flex items-center gap-2 text-xs text-neutral-400">
            <input
              type="checkbox"
              checked={s.hasCenter}
              onchange={(e) =>
                toggleCenter(s, (e.target as HTMLInputElement).checked)}
            />
            <span>9-patch</span>
          </label>
          {#if s.hasCenter}
            <fieldset class="flex flex-col gap-1 pl-4">
              <legend class="sr-only">9-patch center rectangle</legend>
              <div class="flex gap-1">
                <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                  <span>cX</span>
                  <input
                    type="number"
                    step="1"
                    inputmode="numeric"
                    value={s.cx}
                    onchange={(e) =>
                      onNumberCommit(s, 'cx', Number((e.target as HTMLInputElement).value))}
                    class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                  />
                </label>
                <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                  <span>cY</span>
                  <input
                    type="number"
                    step="1"
                    inputmode="numeric"
                    value={s.cy}
                    onchange={(e) =>
                      onNumberCommit(s, 'cy', Number((e.target as HTMLInputElement).value))}
                    class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                  />
                </label>
              </div>
              <div class="flex gap-1">
                <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                  <span>cW</span>
                  <input
                    type="number"
                    min="1"
                    step="1"
                    inputmode="numeric"
                    value={s.cw}
                    onchange={(e) =>
                      onNumberCommit(s, 'cw', Number((e.target as HTMLInputElement).value))}
                    class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                  />
                </label>
                <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                  <span>cH</span>
                  <input
                    type="number"
                    min="1"
                    step="1"
                    inputmode="numeric"
                    value={s.ch}
                    onchange={(e) =>
                      onNumberCommit(s, 'ch', Number((e.target as HTMLInputElement).value))}
                    class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                  />
                </label>
              </div>
            </fieldset>
          {/if}

          <label class="flex items-center gap-2 text-xs text-neutral-400">
            <input
              type="checkbox"
              checked={s.hasPivot}
              onchange={(e) =>
                togglePivot(s, (e.target as HTMLInputElement).checked)}
            />
            <span>Pivot</span>
          </label>
          {#if s.hasPivot}
            <div class="flex gap-1 pl-4">
              <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                <span>pX</span>
                <input
                  type="number"
                  step="1"
                  inputmode="numeric"
                  value={s.px}
                  onchange={(e) =>
                    onNumberCommit(s, 'px', Number((e.target as HTMLInputElement).value))}
                  class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                />
              </label>
              <label class="flex flex-1 flex-col gap-0.5 text-[10px] text-neutral-500">
                <span>pY</span>
                <input
                  type="number"
                  step="1"
                  inputmode="numeric"
                  value={s.py}
                  onchange={(e) =>
                    onNumberCommit(s, 'py', Number((e.target as HTMLInputElement).value))}
                  class="rounded border border-neutral-700 bg-neutral-900 px-1 py-0.5 text-sm text-neutral-100"
                />
              </label>
            </div>
          {/if}

          {#if rowError[s.id]}
            <p class="text-xs text-red-400" role="alert">{rowError[s.id]}</p>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</aside>

<style>
  .panel-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.125rem 0.5rem;
    font-size: 0.75rem;
    color: rgb(229 229 229);
  }
  .panel-btn:hover:not(:disabled) {
    background-color: rgb(38 38 38);
  }
  .panel-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .panel-btn-primary {
    background-color: rgb(55 65 81);
    border-color: rgb(115 115 115);
  }
  .panel-btn-primary:hover:not(:disabled) {
    background-color: rgb(75 85 99);
  }
  .color-swatch {
    width: 1rem;
    height: 1rem;
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    cursor: pointer;
  }
</style>
