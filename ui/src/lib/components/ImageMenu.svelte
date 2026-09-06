<script lang="ts">
  // "Image…" dropdown: layer and canvas flips / rotations, resize, scale,
  // crop. Each entry calls `onAction` with a stable action id; the parent
  // runs the wasm command (or opens the resize dialog).
  export type ImageAction =
    | 'layer:flip_horizontal'
    | 'layer:flip_vertical'
    | 'layer:rotate_90_cw'
    | 'layer:rotate_90_ccw'
    | 'layer:rotate_180'
    | 'canvas:flip_horizontal'
    | 'canvas:flip_vertical'
    | 'canvas:rotate_90_cw'
    | 'canvas:rotate_90_ccw'
    | 'canvas:rotate_180'
    | 'canvas:resize'
    | 'canvas:scale'
    | 'canvas:crop';

  let {
    disabled = false,
    hasSelection = false,
    onAction,
  }: {
    disabled?: boolean;
    hasSelection?: boolean;
    onAction: (action: ImageAction) => void;
  } = $props();

  type Item = { id: ImageAction; label: string; needsSelection?: boolean };
  type Group = { title: string; items: Item[] };

  const groups: Group[] = [
    {
      title: 'Layer',
      items: [
        { id: 'layer:flip_horizontal', label: 'Flip Horizontal' },
        { id: 'layer:flip_vertical', label: 'Flip Vertical' },
        { id: 'layer:rotate_90_cw', label: 'Rotate 90° CW' },
        { id: 'layer:rotate_90_ccw', label: 'Rotate 90° CCW' },
        { id: 'layer:rotate_180', label: 'Rotate 180°' },
      ],
    },
    {
      title: 'Canvas',
      items: [
        { id: 'canvas:flip_horizontal', label: 'Flip Canvas Horizontal' },
        { id: 'canvas:flip_vertical', label: 'Flip Canvas Vertical' },
        { id: 'canvas:rotate_90_cw', label: 'Rotate Canvas 90° CW' },
        { id: 'canvas:rotate_90_ccw', label: 'Rotate Canvas 90° CCW' },
        { id: 'canvas:rotate_180', label: 'Rotate Canvas 180°' },
        { id: 'canvas:resize', label: 'Resize Canvas…' },
        { id: 'canvas:scale', label: 'Scale Image…' },
        { id: 'canvas:crop', label: 'Crop to Selection', needsSelection: true },
      ],
    },
  ];

  let open = $state(false);
  let root = $state<HTMLElement | null>(null);

  function pick(id: ImageAction) {
    open = false;
    onAction(id);
  }

  $effect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (root && e.target instanceof Node && root.contains(e.target)) return;
      open = false;
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') open = false;
    };
    window.addEventListener('pointerdown', onDown, true);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('pointerdown', onDown, true);
      window.removeEventListener('keydown', onKey);
    };
  });
</script>

<span bind:this={root} class="relative ml-1 flex items-center">
  <button
    class="panel-btn"
    class:panel-btn-primary={open}
    aria-haspopup="menu"
    aria-expanded={open}
    {disabled}
    onclick={() => (open = !open)}
  >
    Image…
  </button>
  {#if open}
    <ul
      class="absolute left-0 top-full z-10 mt-1 flex min-w-52 flex-col rounded border border-neutral-700 bg-neutral-900 py-1 shadow-lg"
      role="menu"
      aria-label="Image"
    >
      {#each groups as group (group.title)}
        <li
          class="px-3 pb-0.5 pt-1.5 text-[10px] uppercase tracking-wide text-neutral-500"
          role="presentation"
        >
          {group.title}
        </li>
        {#each group.items as item (item.id)}
          <li>
            <button
              class="w-full truncate px-3 py-1 text-left text-xs hover:bg-neutral-800 disabled:opacity-40"
              role="menuitem"
              disabled={item.needsSelection && !hasSelection}
              onclick={() => pick(item.id)}
            >
              {item.label}
            </button>
          </li>
        {/each}
      {/each}
    </ul>
  {/if}
</span>
