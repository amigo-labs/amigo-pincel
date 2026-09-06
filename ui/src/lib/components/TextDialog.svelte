<script lang="ts">
  // Text tool dialog. The parent owns the wasm `Document`, runs
  // `previewText` on every `onPreview(params)` and commits via
  // `onApply(params)`; `onCancel` restores the un-previewed composite.
  export type TextParams = {
    text: string;
    size: number;
    bold: boolean;
    italic: boolean;
    antiAlias: boolean;
    align: 'left' | 'center' | 'right';
  };

  let {
    x,
    y,
    onPreview,
    onApply,
    onCancel,
  }: {
    x: number;
    y: number;
    onPreview: (params: TextParams) => void;
    onApply: (params: TextParams) => void;
    onCancel: () => void;
  } = $props();

  let text = $state('');
  let size = $state(12);
  let bold = $state(false);
  let italic = $state(false);
  let antiAlias = $state(false);
  let align = $state<'left' | 'center' | 'right'>('left');

  const params = $derived<TextParams>({
    text,
    size: Math.max(1, Math.min(2000, Math.round(Number.isFinite(size) ? size : 12))),
    bold,
    italic,
    antiAlias,
    align,
  });

  let rafId = 0;
  $effect(() => {
    const p = params;
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      rafId = 0;
      onPreview(p);
    });
    return () => {
      if (rafId) cancelAnimationFrame(rafId);
    };
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (params.text.trim().length > 0) onApply(params);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="pointer-events-none fixed inset-0 z-20 flex items-start justify-end p-4"
  role="dialog"
  aria-modal="false"
  aria-labelledby="text-title"
>
  <form
    class="pointer-events-auto flex w-80 flex-col gap-3 rounded border border-neutral-700 bg-neutral-900 p-4 text-sm text-neutral-100 shadow-2xl"
    onsubmit={(e) => {
      e.preventDefault();
      if (params.text.trim().length > 0) onApply(params);
    }}
  >
    <header class="flex items-baseline justify-between">
      <h2 id="text-title" class="text-base font-semibold">Text</h2>
      <span class="text-xs text-neutral-400">at {x}, {y}</span>
    </header>
    <!-- svelte-ignore a11y_autofocus -->
    <textarea
      bind:value={text}
      rows="3"
      autofocus
      placeholder="Type here… (Ctrl+Enter to place)"
      class="rounded border border-neutral-700 bg-neutral-950 px-2 py-1 font-mono text-sm"
      aria-label="Text content"
    ></textarea>
    <div class="flex items-center gap-3 text-xs text-neutral-300">
      <label class="flex items-center gap-1">
        <span>Size</span>
        <input
          type="number"
          min="1"
          max="2000"
          bind:value={size}
          class="w-16 rounded border border-neutral-700 bg-neutral-950 px-1 py-0.5 text-right tabular-nums"
          aria-label="Font size"
        />
      </label>
      <select
        bind:value={align}
        class="rounded border border-neutral-700 bg-neutral-950 px-1 py-0.5"
        aria-label="Text alignment"
      >
        <option value="left">Left</option>
        <option value="center">Center</option>
        <option value="right">Right</option>
      </select>
    </div>
    <div class="flex items-center gap-3 text-xs text-neutral-300">
      <label class="flex items-center gap-1">
        <input type="checkbox" bind:checked={bold} />
        <span class="font-bold">Bold</span>
      </label>
      <label class="flex items-center gap-1">
        <input type="checkbox" bind:checked={italic} />
        <span class="italic">Italic</span>
      </label>
      <label class="flex items-center gap-1">
        <input type="checkbox" bind:checked={antiAlias} />
        <span>Anti-alias</span>
      </label>
    </div>
    <footer class="mt-1 flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>Cancel</button>
      <button type="submit" class="panel-btn panel-btn-primary" disabled={params.text.trim().length === 0}>
        Place
      </button>
    </footer>
  </form>
</div>
