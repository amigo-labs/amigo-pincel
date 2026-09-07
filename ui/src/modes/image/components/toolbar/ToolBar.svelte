<script lang="ts">
  // Vertical tool strip (spec §16.2). Selecting a tool sets editor `tool.kind`;
  // the canvas pointer handler reads it to dispatch behavior.
  import { tool, type ToolKind } from '../../stores/editor.svelte';

  interface ToolDef {
    kind: ToolKind;
    icon: string;
    label: string;
    key: string;
  }

  const tools: ToolDef[] = [
    { kind: 'rect_select', icon: '▭', label: 'Rectangle Select', key: 'M' },
    { kind: 'ellipse_select', icon: '◯', label: 'Ellipse Select', key: 'M' },
    { kind: 'lasso', icon: '🪢', label: 'Lasso', key: 'L' },
    { kind: 'polygon_lasso', icon: '⬠', label: 'Polygonal Lasso', key: 'L' },
    { kind: 'magic_wand', icon: '🪄', label: 'Magic Wand', key: 'W' },
    { kind: 'pencil', icon: '✏️', label: 'Pencil', key: 'B' },
    { kind: 'eraser', icon: '🩹', label: 'Eraser', key: 'E' },
    { kind: 'fill', icon: '🪣', label: 'Fill', key: 'G' },
    { kind: 'eyedropper', icon: '💉', label: 'Eyedropper', key: 'I' },
    { kind: 'move', icon: '✥', label: 'Move', key: 'V' },
    { kind: 'shapes', icon: '⬚', label: 'Shapes', key: 'U' },
    { kind: 'text', icon: 'T', label: 'Text', key: 'T' },
  ];
</script>

<div
  class="flex w-12 flex-col items-center gap-1 border-r border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)] py-2"
>
  {#each tools as t (t.kind)}
    <button
      class="flex h-9 w-9 items-center justify-center rounded text-lg"
      class:bg-[var(--fl-accent)]={tool.kind === t.kind}
      class:text-white={tool.kind === t.kind}
      title={`${t.label} (${t.key})`}
      aria-label={t.label}
      aria-pressed={tool.kind === t.kind}
      onclick={() => (tool.kind = t.kind)}
    >
      {t.icon}
    </button>
  {/each}
</div>
