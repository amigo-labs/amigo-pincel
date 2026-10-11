<script lang="ts">
  // Foreground/background swatches with swap and hex entry (spec §16.6, §4.2).
  import { tool, swapColors } from '../../stores/editor.svelte';

  // Hex text entry mirrors the foreground; only a full #RRGGBB commits.
  let hexText = $state(tool.foreground);
  let hexFocused = false;

  $effect(() => {
    // Follow external changes (picker, eyedropper, swap) unless mid-edit.
    if (!hexFocused) {
      hexText = tool.foreground;
    }
  });

  function onHexInput(): void {
    const hex = /^#?([0-9a-f]{6})$/i.exec(hexText.trim())?.[1];
    if (hex !== undefined) {
      tool.foreground = `#${hex.toLowerCase()}`;
    }
  }
</script>

<div class="border-b border-pc-border bg-pc-panel p-3">
  <h2 class="mb-2 text-xs font-semibold uppercase tracking-wide text-pc-faint">Colors</h2>
  <div class="flex items-center gap-3">
    <label class="flex flex-col items-center gap-1 text-xs text-pc-muted">
      Foreground
      <input
        type="color"
        bind:value={tool.foreground}
        class="h-10 w-10 cursor-pointer rounded border border-pc-border bg-transparent"
        aria-label="Foreground color"
      />
    </label>
    <button
      class="rounded px-2 py-1 text-lg text-pc-text hover:bg-pc-hover"
      title="Swap colors (X)"
      onclick={swapColors}
      aria-label="Swap foreground and background colors"
    >
      ⇄
    </button>
    <label class="flex flex-col items-center gap-1 text-xs text-pc-muted">
      Background
      <input
        type="color"
        bind:value={tool.background}
        class="h-10 w-10 cursor-pointer rounded border border-pc-border bg-transparent"
        aria-label="Background color"
      />
    </label>
  </div>
  <label class="mt-2 flex items-center gap-2 text-xs text-pc-muted">
    Hex
    <input
      type="text"
      bind:value={hexText}
      oninput={onHexInput}
      onfocus={() => (hexFocused = true)}
      onblur={() => {
        hexFocused = false;
        hexText = tool.foreground;
      }}
      spellcheck="false"
      maxlength="7"
      class="w-20 rounded border border-pc-border bg-pc-bg px-2 py-1 font-mono text-pc-text"
      aria-label="Foreground color hex value"
    />
  </label>
</div>
