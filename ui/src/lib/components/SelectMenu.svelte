<script lang="ts">
  // "Select…" dropdown: whole-selection operations that have no gesture
  // (all / none / invert / expand / contract). The parent runs the wasm
  // calls; shortcuts are listed for discoverability only.
  export type SelectAction = 'all' | 'none' | 'invert' | 'expand' | 'contract';

  let {
    disabled = false,
    hasSelection = false,
    onAction,
  }: {
    disabled?: boolean;
    hasSelection?: boolean;
    onAction: (action: SelectAction) => void;
  } = $props();

  const items: { id: SelectAction; label: string; hint: string; needsSelection?: boolean }[] = [
    { id: 'all', label: 'Select All', hint: 'Ctrl+A' },
    { id: 'none', label: 'Deselect', hint: 'Ctrl+D' },
    { id: 'invert', label: 'Invert Selection', hint: 'Ctrl+Shift+I' },
    { id: 'expand', label: 'Expand 1 px', hint: '', needsSelection: true },
    { id: 'contract', label: 'Contract 1 px', hint: '', needsSelection: true },
  ];

  let open = $state(false);
  let root = $state<HTMLElement | null>(null);

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
    Select…
  </button>
  {#if open}
    <ul
      class="absolute left-0 top-full z-10 mt-1 flex min-w-52 flex-col rounded border border-neutral-700 bg-neutral-900 py-1 shadow-lg"
      role="menu"
      aria-label="Select"
    >
      {#each items as item (item.id)}
        <li>
          <button
            class="flex w-full items-center justify-between gap-4 px-3 py-1 text-left text-xs hover:bg-neutral-800 disabled:opacity-40"
            role="menuitem"
            disabled={item.needsSelection && !hasSelection}
            onclick={() => {
              open = false;
              onAction(item.id);
            }}
          >
            <span>{item.label}</span>
            <span class="text-neutral-500">{item.hint}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</span>
