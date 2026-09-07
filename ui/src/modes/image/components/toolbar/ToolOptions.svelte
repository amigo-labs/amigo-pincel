<script lang="ts">
  // Tool options bar (spec §16.3, §9.2). Controls shown depend on the active
  // tool. Bindings write straight into the tool store.
  import { tool, editor, type ToolKind } from '../../stores/editor.svelte';
  import {
    deselect,
    invertSelection,
    expandSelection,
    contractSelection,
    featherSelection,
  } from '../../core/controller';

  // Hardness only matters for soft/flat tips.
  const showHardness = $derived(tool.shape !== 'hard_round');

  // Selection tools that draw a feather-able shape (not the wand).
  const shapeSelectTools: ReadonlySet<ToolKind> = new Set([
    'rect_select',
    'ellipse_select',
    'lasso',
    'polygon_lasso',
  ]);
  const isShapeSelect = $derived(shapeSelectTools.has(tool.kind));
</script>

<div
  class="flex items-center gap-6 border-b border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)] px-4 py-2 text-sm"
>
  {#if tool.kind === 'pencil' || tool.kind === 'eraser'}
    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Size</span>
      <input type="range" min="1" max="500" bind:value={tool.size} class="w-32" />
      <span class="w-10 tabular-nums text-neutral-300">{tool.size}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Opacity</span>
      <input type="range" min="1" max="100" bind:value={tool.opacity} class="w-28" />
      <span class="w-10 tabular-nums text-neutral-300">{tool.opacity}%</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Shape</span>
      <select bind:value={tool.shape} class="rounded bg-neutral-800 px-2 py-1">
        <option value="hard_round">Hard Round</option>
        <option value="soft_round">Soft Round</option>
        <option value="flat">Flat</option>
      </select>
    </label>

    {#if showHardness}
      <label class="flex items-center gap-2">
        <span class="text-neutral-400">Hardness</span>
        <input type="range" min="0" max="100" bind:value={tool.hardness} class="w-28" />
        <span class="w-10 tabular-nums text-neutral-300">{tool.hardness}%</span>
      </label>
    {/if}

    {#if tool.kind === 'eraser'}
      <label class="flex items-center gap-2">
        <span class="text-neutral-400">Mode</span>
        <select bind:value={tool.eraserMode} class="rounded bg-neutral-800 px-2 py-1">
          <option value="to_transparent">To Transparent</option>
          <option value="to_background">To Background</option>
        </select>
      </label>
    {/if}
  {:else if tool.kind === 'fill'}
    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Tolerance</span>
      <input type="range" min="0" max="255" bind:value={tool.tolerance} class="w-32" />
      <span class="w-10 tabular-nums text-neutral-300">{tool.tolerance}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Opacity</span>
      <input type="range" min="1" max="100" bind:value={tool.opacity} class="w-28" />
      <span class="w-10 tabular-nums text-neutral-300">{tool.opacity}%</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.contiguous} />
      <span class="text-neutral-400">Contiguous</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Sample</span>
      <select bind:value={tool.fillSample} class="rounded bg-neutral-800 px-2 py-1">
        <option value="current_layer">Current Layer</option>
        <option value="all_layers">All Layers</option>
      </select>
    </label>
  {:else if tool.kind === 'eyedropper'}
    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Sample</span>
      <select bind:value={tool.eyedropperSample} class="rounded bg-neutral-800 px-2 py-1">
        <option value="current_layer">Current Layer</option>
        <option value="all_layers">Composite</option>
      </select>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Size</span>
      <select bind:value={tool.sampleSize} class="rounded bg-neutral-800 px-2 py-1">
        <option value={1}>1×1</option>
        <option value={3}>3×3</option>
        <option value={5}>5×5</option>
        <option value={11}>11×11</option>
        <option value={31}>31×31</option>
      </select>
    </label>
  {:else if tool.kind === 'move'}
    <span class="text-neutral-500">Drag to translate the active layer.</span>
  {:else if isShapeSelect}
    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Feather</span>
      <input type="range" min="0" max="250" bind:value={tool.selectionFeather} class="w-28" />
      <span class="w-10 tabular-nums text-neutral-300">{tool.selectionFeather}</span>
    </label>
    <span class="text-neutral-500">Shift = add · Alt = subtract · Shift+Alt = intersect</span>
  {:else if tool.kind === 'magic_wand'}
    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Tolerance</span>
      <input type="range" min="0" max="255" bind:value={tool.tolerance} class="w-32" />
      <span class="w-10 tabular-nums text-neutral-300">{tool.tolerance}</span>
    </label>
    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.contiguous} />
      <span class="text-neutral-400">Contiguous</span>
    </label>
    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Sample</span>
      <select bind:value={tool.wandSample} class="rounded bg-neutral-800 px-2 py-1">
        <option value="current_layer">Current Layer</option>
        <option value="all_layers">All Layers</option>
      </select>
    </label>
  {:else if tool.kind === 'shapes'}
    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Shape</span>
      <select bind:value={tool.shapeKind} class="rounded bg-neutral-800 px-2 py-1">
        <option value="line">Line</option>
        <option value="rectangle">Rectangle</option>
        <option value="rounded_rectangle">Rounded Rectangle</option>
        <option value="ellipse">Ellipse</option>
        <option value="polygon">Polygon</option>
      </select>
    </label>

    {#if tool.shapeKind !== 'line'}
      <label class="flex items-center gap-2">
        <span class="text-neutral-400">Mode</span>
        <select bind:value={tool.shapeMode} class="rounded bg-neutral-800 px-2 py-1">
          <option value="outline">Outline</option>
          <option value="fill">Fill</option>
          <option value="fill_and_outline">Fill + Outline</option>
        </select>
      </label>
    {/if}

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Stroke</span>
      <input type="range" min="1" max="100" bind:value={tool.strokeWidth} class="w-24" />
      <span class="w-8 tabular-nums text-neutral-300">{tool.strokeWidth}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Dash</span>
      <select bind:value={tool.shapeDash} class="rounded bg-neutral-800 px-2 py-1">
        <option value="solid">Solid</option>
        <option value="dashed">Dashed</option>
        <option value="dotted">Dotted</option>
      </select>
    </label>

    {#if tool.shapeKind === 'polygon'}
      <label class="flex items-center gap-2">
        <span class="text-neutral-400">Sides</span>
        <input type="range" min="3" max="100" bind:value={tool.shapeSides} class="w-24" />
        <span class="w-8 tabular-nums text-neutral-300">{tool.shapeSides}</span>
      </label>
    {/if}

    {#if tool.shapeKind === 'rounded_rectangle'}
      <label class="flex items-center gap-2">
        <span class="text-neutral-400">Radius</span>
        <input type="range" min="0" max="200" bind:value={tool.cornerRadius} class="w-24" />
        <span class="w-8 tabular-nums text-neutral-300">{tool.cornerRadius}</span>
      </label>
    {/if}

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Opacity</span>
      <input type="range" min="1" max="100" bind:value={tool.opacity} class="w-24" />
      <span class="w-10 tabular-nums text-neutral-300">{tool.opacity}%</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.shapeAntiAlias} />
      <span class="text-neutral-400">Anti-alias</span>
    </label>
    <span class="text-neutral-500">Stroke = FG · Fill = BG · Shift = constrain</span>
  {:else if tool.kind === 'text'}
    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Size</span>
      <input type="range" min="6" max="400" bind:value={tool.fontSize} class="w-32" />
      <span class="w-10 tabular-nums text-neutral-300">{tool.fontSize}</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.textBold} />
      <span class="text-neutral-400">Bold</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.textItalic} />
      <span class="text-neutral-400">Italic</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-neutral-400">Align</span>
      <select bind:value={tool.textAlign} class="rounded bg-neutral-800 px-2 py-1">
        <option value="left">Left</option>
        <option value="center">Center</option>
        <option value="right">Right</option>
      </select>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.textAntiAlias} />
      <span class="text-neutral-400">Anti-alias</span>
    </label>
    <span class="text-neutral-500">Click to place · Esc or Ctrl+Enter to commit</span>
  {/if}

  {#if isShapeSelect || tool.kind === 'magic_wand'}
    <!-- Operations on the existing selection (spec §8.4). -->
    <div class="ml-auto flex items-center gap-1.5 text-xs">
      <button
        class="rounded px-2 py-1 hover:bg-neutral-700 disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={() => expandSelection(1)}>Expand</button
      >
      <button
        class="rounded px-2 py-1 hover:bg-neutral-700 disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={() => contractSelection(1)}>Contract</button
      >
      <button
        class="rounded px-2 py-1 hover:bg-neutral-700 disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={() => featherSelection(2)}>Feather</button
      >
      <button
        class="rounded px-2 py-1 hover:bg-neutral-700 disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={invertSelection}>Invert</button
      >
      <button
        class="rounded px-2 py-1 hover:bg-neutral-700 disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={deselect}>Deselect</button
      >
    </div>
  {/if}
</div>
