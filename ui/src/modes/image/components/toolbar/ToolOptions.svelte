<script lang="ts">
  // Tool options bar (spec §16.3, §9.2). Controls shown depend on the active
  // tool. Bindings write straight into the tool store.
  import { tool, editor, isShapeTool, type ToolKind } from '../../stores/editor.svelte';

  // Size, tip, hardness and the pressure toggles are the same four tools'
  // business; each adds its own control below them.
  const BRUSH_TOOLS: ReadonlySet<ToolKind> = new Set<ToolKind>([
    'pencil',
    'eraser',
    'recolor',
    'smudge',
    'clone',
  ]);
  import {
    deselect,
    invertSelection,
    expandSelection,
    contractSelection,
    featherSelection,
  } from '../../core/controller';
  import { t } from '../../../../lib/i18n/index.svelte';
  import { commitText, textBoxOpen } from '../../core/live-text.svelte';
  import CloneOptions from './CloneOptions.svelte';

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

  // The tools that draw an edge Antialiasing can smooth; one shared setting
  // (docs/specs/image-edit-extras-tool-options.md AC-01).
  const showAntiAlias = $derived(
    ['pencil', 'eraser', 'recolor', 'smudge', 'text', 'ellipse_select', 'lasso', 'polygon_lasso'].includes(
      tool.kind,
    ) || isShapeTool(tool.kind),
  );

  // The tools that composite a paint color; the Gradient only on its Color
  // channel (AC-08).
  const showBlend = $derived(
    ['pencil', 'fill', 'text'].includes(tool.kind) ||
      isShapeTool(tool.kind) ||
      (tool.kind === 'gradient' && tool.gradientChannel === 'color'),
  );
</script>

<div class="flex shrink-0 items-center gap-5 text-sm whitespace-nowrap">
  {#if BRUSH_TOOLS.has(tool.kind)}
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
      <select bind:value={tool.shape} class="opt-field">
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

    <span class="opt opt-divided" title={t('pressure.hint')}>
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
        <select bind:value={tool.eraserMode} class="opt-field">
          <option value="to_transparent">{t('opt.toTransparent')}</option>
          <option value="to_background">{t('opt.toBackground')}</option>
        </select>
      </label>
    {:else if tool.kind === 'recolor'}
      <label class="flex items-center gap-2">
        <span class="text-pc-muted">{t('opt.tolerance')}</span>
        <input type="range" min="0" max="255" bind:value={tool.recolorTolerance} class="w-24" />
        <span class="w-10 tabular-nums text-pc-text">{tool.recolorTolerance}</span>
      </label>
      <span class="text-pc-faint">{t('opt.recolorHint')}</span>
    {:else if tool.kind === 'smudge'}
      <label class="flex items-center gap-2">
        <span class="text-pc-muted">{t('opt.strength')}</span>
        <input type="range" min="1" max="100" bind:value={tool.smudgeStrength} class="w-24" />
        <span class="w-10 tabular-nums text-pc-text">{tool.smudgeStrength}%</span>
      </label>
    {:else if tool.kind === 'clone'}
      <CloneOptions />
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
      <select bind:value={tool.fillSample} class="opt-field">
        <option value="current_layer">{t('opt.currentLayer')}</option>
        <option value="all_layers">{t('opt.allLayers')}</option>
      </select>
    </label>
  {:else if tool.kind === 'eyedropper'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.sample')}</span>
      <select bind:value={tool.eyedropperSample} class="opt-field">
        <option value="current_layer">{t('opt.currentLayer')}</option>
        <option value="all_layers">{t('opt.composite')}</option>
      </select>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.size')}</span>
      <select bind:value={tool.sampleSize} class="opt-field">
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
      <select bind:value={tool.wandSample} class="opt-field">
        <option value="current_layer">{t('opt.currentLayer')}</option>
        <option value="all_layers">{t('opt.allLayers')}</option>
      </select>
    </label>
  {:else if tool.kind === 'gradient'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.gradientKind')}</span>
      <select bind:value={tool.gradientKind} class="opt-field">
        <option value="linear">{t('opt.gradientLinear')}</option>
        <option value="reflected">{t('opt.gradientReflected')}</option>
        <option value="radial">{t('opt.gradientRadial')}</option>
        <option value="conical">{t('opt.gradientConical')}</option>
        <option value="diamond">{t('opt.gradientDiamond')}</option>
        <option value="spiral_cw">{t('opt.gradientSpiralCw')}</option>
        <option value="spiral_ccw">{t('opt.gradientSpiralCcw')}</option>
      </select>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.gradientChannel')}</span>
      <select bind:value={tool.gradientChannel} class="opt-field">
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
        <select bind:value={tool.shapeMode} class="opt-field">
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
      <select bind:value={tool.shapeDash} class="opt-field">
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

    <span class="text-pc-faint">{t('opt.shapeHint')}</span>
  {:else if tool.kind === 'text'}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.size')}</span>
      <input
        type="range"
        min="6"
        max="400"
        bind:value={tool.fontSize}
        class="w-32"
        aria-label={t('text.fontSize')}
      />
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
      <input type="checkbox" bind:checked={tool.textUnderline} />
      <span class="text-pc-muted">{t('opt.underline')}</span>
    </label>

    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.textStrikethrough} />
      <span class="text-pc-muted">{t('opt.strikethrough')}</span>
    </label>

    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.align')}</span>
      <select bind:value={tool.textAlign} class="opt-field">
        <option value="left">{t('opt.left')}</option>
        <option value="center">{t('opt.center')}</option>
        <option value="right">{t('opt.right')}</option>
      </select>
    </label>

    <!-- Finishes the live text (docs/specs/image-live-text.md AC-11). -->
    <button
      class="rounded px-2 py-1 hover:bg-pc-hover disabled:opacity-40"
      disabled={!textBoxOpen()}
      onclick={commitText}>{t('opt.finish')}</button
    >
    <span class="text-pc-faint">{t('opt.textHint')}</span>
  {/if}

  {#if showAntiAlias}
    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={tool.antiAlias} />
      <span class="text-pc-muted">{t('opt.antialiasing')}</span>
    </label>
  {/if}

  {#if showBlend}
    <label class="flex items-center gap-2">
      <span class="text-pc-muted">{t('opt.blending')}</span>
      <select bind:value={tool.blend} class="opt-field">
        <option value="normal">{t('opt.blendNormal')}</option>
        <option value="overwrite">{t('opt.blendOverwrite')}</option>
      </select>
    </label>
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
