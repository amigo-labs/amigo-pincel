<script lang="ts">
  // Image and Layer transform menus (spec §10, §16.1). Each item dispatches a
  // transform through the controller; Resize / Scale open a dialog first.
  import { editor } from '../../stores/editor.svelte';
  import {
    transformLayer,
    rotateLayer90,
    flipCanvas,
    rotateCanvas,
    scaleImage,
    resizeCanvas,
    cropToSelection,
  } from '../../core/controller';
  import ResizeDialog from '../dialogs/ResizeDialog.svelte';
  import ScaleDialog from '../dialogs/ScaleDialog.svelte';

  let open = $state<'image' | 'layer' | null>(null);
  let dialog = $state<'resize' | 'scale' | null>(null);

  function run(action: () => void): void {
    action();
    open = null;
  }
</script>

<div class="relative flex items-center gap-1">
  <!-- Image menu -->
  <button
    class="rounded px-2 py-1 hover:bg-neutral-700"
    class:bg-neutral-700={open === 'image'}
    onclick={() => (open = open === 'image' ? null : 'image')}
  >
    Image
  </button>
  <!-- Layer menu -->
  <button
    class="rounded px-2 py-1 hover:bg-neutral-700"
    class:bg-neutral-700={open === 'layer'}
    onclick={() => (open = open === 'layer' ? null : 'layer')}
  >
    Layer
  </button>

  {#if open !== null}
    <!-- Backdrop closes the menu on an outside click. -->
    <button class="fixed inset-0 z-40 cursor-default" aria-label="Close menu" onclick={() => (open = null)}
    ></button>
  {/if}

  {#if open === 'image'}
    <div
      class="absolute left-0 top-8 z-50 w-52 rounded border border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)] py-1 text-sm shadow-xl"
    >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => flipCanvas(true))}>Flip Horizontal</button
      >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => flipCanvas(false))}>Flip Vertical</button
      >
      <div class="my-1 border-t border-[var(--fl-panel-border)]"></div>
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => rotateCanvas('cw90'))}>Rotate 90° CW</button
      >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => rotateCanvas('ccw90'))}>Rotate 90° CCW</button
      >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => rotateCanvas('rotate_180'))}>Rotate 180°</button
      >
      <div class="my-1 border-t border-[var(--fl-panel-border)]"></div>
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => {
          open = null;
          dialog = 'resize';
        }}>Resize Canvas…</button
      >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => {
          open = null;
          dialog = 'scale';
        }}>Scale Image…</button
      >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700 disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={() => run(cropToSelection)}
      >
        Crop to Selection
      </button>
    </div>
  {/if}

  {#if open === 'layer'}
    <div
      class="absolute left-14 top-8 z-50 w-52 rounded border border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)] py-1 text-sm shadow-xl"
    >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => transformLayer('flip_h'))}>Flip Horizontal</button
      >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => transformLayer('flip_v'))}>Flip Vertical</button
      >
      <div class="my-1 border-t border-[var(--fl-panel-border)]"></div>
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => rotateLayer90(false))}>Rotate 90° CW</button
      >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => rotateLayer90(true))}>Rotate 90° CCW</button
      >
      <button
        class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
        onclick={() => run(() => transformLayer('rotate_180'))}>Rotate 180°</button
      >
    </div>
  {/if}
</div>

{#if dialog === 'resize'}
  <ResizeDialog
    onApply={(w, h, anchor) => {
      resizeCanvas(w, h, anchor);
      dialog = null;
    }}
    onClose={() => (dialog = null)}
  />
{/if}
{#if dialog === 'scale'}
  <ScaleDialog
    onApply={(w, h, interp) => {
      scaleImage(w, h, interp);
      dialog = null;
    }}
    onClose={() => (dialog = null)}
  />
{/if}
