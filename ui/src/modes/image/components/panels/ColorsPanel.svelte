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
    const m = /^#?([0-9a-f]{6})$/i.exec(hexText.trim());
    if (m) {
      tool.foreground = `#${m[1].toLowerCase()}`;
    }
  }
</script>

<div class="border-b border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)] p-3">
  <h2 class="mb-2 text-xs font-semibold uppercase tracking-wide text-neutral-500">Colors</h2>
  <div class="flex items-center gap-3">
    <label class="flex flex-col items-center gap-1 text-xs text-neutral-400">
      Foreground
      <input
        type="color"
        bind:value={tool.foreground}
        class="h-10 w-10 cursor-pointer rounded border border-[var(--fl-panel-border)] bg-transparent"
        aria-label="Foreground color"
      />
    </label>
    <button
      class="rounded px-2 py-1 text-lg text-neutral-300 hover:bg-neutral-700"
      title="Swap colors (X)"
      onclick={swapColors}
      aria-label="Swap foreground and background colors"
    >
      ⇄
    </button>
    <label class="flex flex-col items-center gap-1 text-xs text-neutral-400">
      Background
      <input
        type="color"
        bind:value={tool.background}
        class="h-10 w-10 cursor-pointer rounded border border-[var(--fl-panel-border)] bg-transparent"
        aria-label="Background color"
      />
    </label>
  </div>
  <label class="mt-2 flex items-center gap-2 text-xs text-neutral-400">
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
      class="w-20 rounded border border-[var(--fl-panel-border)] bg-[var(--fl-app-bg)] px-2 py-1 font-mono text-neutral-200"
      aria-label="Foreground color hex value"
    />
  </label>
</div>
