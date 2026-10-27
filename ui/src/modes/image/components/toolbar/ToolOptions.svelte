<script lang="ts">
  // Tool options bar (spec §16.3, §9.2). Controls shown depend on the active
  // tool. Bindings write straight into the tool store.
  import { tool, editor, isShapeTool, type ToolKind } from '../../stores/editor.svelte';
  import {
    deselect,
    invertSelection,
    expandSelection,
    contractSelection,
    featherSelection,
  } from '../../core/controller';
  import { t } from '../../../../lib/i18n/index.svelte';

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

<div class="flex shrink-0 items-center gap-5 text-sm whitespace-nowrap">
  {#if tool.kind === 'pencil' || tool.kind === 'eraser'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.size')}</span>
      <input type="range" min="1" max="500" bind:value={tool.size} class="w-32" />
      <span class="w-10 tabular-nums text-pc-text">{tool.size}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.opacity')}</span>
      <input type="range" min="1" max="100" bind:value={tool.opacity} class="w-28" />
      <span class="w-10 tabular-nums text-pc-text">{tool.opacity}%</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.shape')}</span>
      <select bind:value={tool.shape} class="rounded bg-pc-raised px-2 py-1">
        <option value="hard_round">{t('opt.hardRound')}</option>
        <option value="soft_round">{t('opt.softRound')}</option>
        <option value="flat">{t('opt.flat')}</option>
      </select>
    </label>

    {#if showHardness}
      <label class="flex items-center gap-2">
        <span class="text-pc-muted">{t('opt.hardness')}</span>
        <input type="range" min="0" max="100" bind:value={tool.hardness} class="w-28" />
        <span class="w-10 tabular-nums text-pc-text">{tool.hardness}%</span>
      </label>
    {/if}

    {#if tool.shape === 'flat'}
      <label class="flex items-center gap-2">
        <span class="text-pc-muted">{t('opt.angle')}</span>
        <input type="range" min="0" max="180" bind:value={tool.angle} class="w-28" />
        <span class="w-10 tabular-nums text-pc-text">{tool.angle}°</span>
      </label>
    {/if}

    <span class="flex items-center gap-3" title={t('pressure.hint')}>
      <span class="text-pc-muted">{t('pressure.label')}</span>
      <label class="flex items-center gap-1">
        <input type="checkbox" bind:checked={tool.pressureSize} />
        {t('pressure.size')}
      </label>
      <label class="flex items-center gap-1">
        <input type="checkbox" bind:checked={tool.pressureOpacity} />
        {t('pressure.opacity')}
      </label>
      {#if tool.shape === 'flat'}
        <label class="flex items-center gap-1" title={t('tilt.hint')}>
          <input type="checkbox" bind:checked={tool.tiltAngle} />
          {t('tilt.angle')}
        </label>
      {/if}
    </span>

    {#if tool.kind === 'eraser'}
      <label class="flex items-center gap-2">
        <span class="text-pc-muted">{t('opt.mode')}</span>
        <select bind:value={tool.eraserMode} class="rounded bg-pc-raised px-2 py-1">
          <option value="to_transparent">{t('opt.toTransparent')}</option>
          <option value="to_background">{t('opt.toBackground')}</option>
        </select>
      </label>
    {/if}
  {:else if tool.kind === 'fill'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.tolerance')}</span>
      <input type="range" min="0" max="255" bind:value={tool.tolerance} class="w-32" />
      <span class="w-10 tabular-nums text-pc-text">{tool.tolerance}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.opacity')}</span>
      <input type="range" min="1" max="100" bind:value={tool.opacity} class="w-28" />
      <span class="w-10 tabular-nums text-pc-text">{tool.opacity}%</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.contiguous} />
      <span class="text-pc-muted">{t('opt.contiguous')}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.sample')}</span>
      <select bind:value={tool.fillSample} class="rounded bg-pc-raised px-2 py-1">
        <option value="current_layer">{t('opt.currentLayer')}</option>
        <option value="all_layers">{t('opt.allLayers')}</option>
      </select>
    </label>
  {:else if tool.kind === 'eyedropper'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.sample')}</span>
      <select bind:value={tool.eyedropperSample} class="rounded bg-pc-raised px-2 py-1">
        <option value="current_layer">{t('opt.currentLayer')}</option>
        <option value="all_layers">{t('opt.composite')}</option>
      </select>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.size')}</span>
      <select bind:value={tool.sampleSize} class="rounded bg-pc-raised px-2 py-1">
        <option value={1}>1×1</option>
        <option value={3}>3×3</option>
        <option value={5}>5×5</option>
        <option value={11}>11×11</option>
        <option value={31}>31×31</option>
      </select>
    </label>
  {:else if tool.kind === 'move'}
    <span class="text-pc-faint">{t('opt.moveHint')}</span>
  {:else if isShapeSelect}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.feather')}</span>
      <input type="range" min="0" max="250" bind:value={tool.selectionFeather} class="w-28" />
      <span class="w-10 tabular-nums text-pc-text">{tool.selectionFeather}</span>
    </label>
    <span class="text-pc-faint">{t('opt.selectHint')}</span>
  {:else if tool.kind === 'magic_wand'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.tolerance')}</span>
      <input type="range" min="0" max="255" bind:value={tool.tolerance} class="w-32" />
      <span class="w-10 tabular-nums text-pc-text">{tool.tolerance}</span>
    </label>
    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.contiguous} />
      <span class="text-pc-muted">{t('opt.contiguous')}</span>
    </label>
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.sample')}</span>
      <select bind:value={tool.wandSample} class="rounded bg-pc-raised px-2 py-1">
        <option value="current_layer">{t('opt.currentLayer')}</option>
        <option value="all_layers">{t('opt.allLayers')}</option>
      </select>
    </label>
  {:else if tool.kind === 'gradient'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.gradientKind')}</span>
      <select bind:value={tool.gradientKind} class="rounded bg-pc-raised px-2 py-1">
        <option value="linear">{t('opt.gradientLinear')}</option>
        <option value="reflected">{t('opt.gradientReflected')}</option>
        <option value="radial">{t('opt.gradientRadial')}</option>
        <option value="conical">{t('opt.gradientConical')}</option>
        <option value="diamond">{t('opt.gradientDiamond')}</option>
      </select>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.gradientChannel')}</span>
      <select bind:value={tool.gradientChannel} class="rounded bg-pc-raised px-2 py-1">
        <option value="color">{t('opt.gradientColor')}</option>
        <option value="transparency">{t('opt.gradientTransparency')}</option>
      </select>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.opacity')}</span>
      <input type="range" min="1" max="100" bind:value={tool.opacity} class="w-24" />
      <span class="w-10 tabular-nums text-pc-text">{tool.opacity}%</span>
    </label>
    <span class="text-pc-faint">{t('opt.gradientHint')}</span>
  {:else if isShapeTool(tool.kind)}
    {#if tool.kind !== 'line'}
      <label class="flex items-center gap-2">
        <span class="text-pc-muted">{t('opt.mode')}</span>
        <select bind:value={tool.shapeMode} class="rounded bg-pc-raised px-2 py-1">
          <option value="outline">{t('opt.outline')}</option>
          <option value="fill">{t('opt.fillShape')}</option>
          <option value="fill_and_outline">{t('opt.fillOutline')}</option>
        </select>
      </label>
    {/if}

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.stroke')}</span>
      <input type="range" min="1" max="100" bind:value={tool.strokeWidth} class="w-24" />
      <span class="w-8 tabular-nums text-pc-text">{tool.strokeWidth}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.dash')}</span>
      <select bind:value={tool.shapeDash} class="rounded bg-pc-raised px-2 py-1">
        <option value="solid">{t('opt.solid')}</option>
        <option value="dashed">{t('opt.dashed')}</option>
        <option value="dotted">{t('opt.dotted')}</option>
      </select>
    </label>

    {#if tool.kind === 'polygon'}
      <label class="flex items-center gap-2">
        <span class="text-pc-muted">{t('opt.sides')}</span>
        <input type="range" min="3" max="100" bind:value={tool.shapeSides} class="w-24" />
        <span class="w-8 tabular-nums text-pc-text">{tool.shapeSides}</span>
      </label>
    {/if}

    {#if tool.kind === 'rounded_rect'}
      <label class="flex items-center gap-2">
        <span class="text-pc-muted">{t('opt.radius')}</span>
        <input type="range" min="0" max="200" bind:value={tool.cornerRadius} class="w-24" />
        <span class="w-8 tabular-nums text-pc-text">{tool.cornerRadius}</span>
      </label>
    {/if}

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.opacity')}</span>
      <input type="range" min="1" max="100" bind:value={tool.opacity} class="w-24" />
      <span class="w-10 tabular-nums text-pc-text">{tool.opacity}%</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.shapeAntiAlias} />
      <span class="text-pc-muted">{t('opt.antiAlias')}</span>
    </label>
    <span class="text-pc-faint">{t('opt.shapeHint')}</span>
  {:else if tool.kind === 'text'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.size')}</span>
      <input type="range" min="6" max="400" bind:value={tool.fontSize} class="w-32" />
      <span class="w-10 tabular-nums text-pc-text">{tool.fontSize}</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.textBold} />
      <span class="text-pc-muted">{t('opt.bold')}</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.textItalic} />
      <span class="text-pc-muted">{t('opt.italic')}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.align')}</span>
      <select bind:value={tool.textAlign} class="rounded bg-pc-raised px-2 py-1">
        <option value="left">{t('opt.left')}</option>
        <option value="center">{t('opt.center')}</option>
        <option value="right">{t('opt.right')}</option>
      </select>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.textAntiAlias} />
      <span class="text-pc-muted">{t('opt.antiAlias')}</span>
    </label>
    <span class="text-pc-faint">{t('opt.textHint')}</span>
  {/if}

  {#if isShapeSelect || tool.kind === 'magic_wand'}
    <!-- Operations on the existing selection (spec §8.4). -->
    <div class="ml-auto flex items-center gap-1.5 text-xs">
      <button
        class="rounded px-2 py-1 hover:bg-pc-hover disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={() => expandSelection(1)}>{t('opt.expand')}</button
      >
      <button
        class="rounded px-2 py-1 hover:bg-pc-hover disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={() => contractSelection(1)}>{t('opt.contract')}</button
      >
      <button
        class="rounded px-2 py-1 hover:bg-pc-hover disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={() => featherSelection(2)}>{t('opt.feather')}</button
      >
      <button
        class="rounded px-2 py-1 hover:bg-pc-hover disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={invertSelection}>{t('opt.invert')}</button
      >
      <button
        class="rounded px-2 py-1 hover:bg-pc-hover disabled:opacity-40"
        disabled={!editor.hasSelection}
        onclick={deselect}>{t('menu.deselect')}</button
      >
    </div>
  {/if}
</div>
