<script lang="ts">
  import {
    EFFECTS,
    EFFECT_MENU_GROUPS,
    GROUP_LABELS,
    type EffectGroup,
  } from '../effects/catalog';

  // Two dropdown triggers — "Effects" (blur / sharpen / distort / noise)
  // and "Adjust" (colour adjustments). Picking an entry calls `onPick`
  // with the effect id; the parent opens the parameter dialog.
  let {
    disabled = false,
    onPick,
  }: {
    disabled?: boolean;
    onPick: (id: string) => void;
  } = $props();

  let open = $state<'effects' | 'adjust' | null>(null);
  let root = $state<HTMLElement | null>(null);

  function toggle(which: 'effects' | 'adjust') {
    open = open === which ? null : which;
  }

  function pick(id: string) {
    open = null;
    onPick(id);
  }

  function groupsFor(which: 'effects' | 'adjust'): EffectGroup[] {
    return which === 'effects' ? EFFECT_MENU_GROUPS : ['adjust'];
  }

  // Close on outside click / Escape, like the Recent menu.
  $effect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (root && e.target instanceof Node && root.contains(e.target)) return;
      open = null;
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') open = null;
    };
    window.addEventListener('pointerdown', onDown, true);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('pointerdown', onDown, true);
      window.removeEventListener('keydown', onKey);
    };
  });
</script>

<span bind:this={root} class="ml-2 flex items-center gap-1">
  {#each ['effects', 'adjust'] as const as which (which)}
    <span class="relative">
      <button
        class="panel-btn"
        class:panel-btn-primary={open === which}
        aria-haspopup="menu"
        aria-expanded={open === which}
        {disabled}
        onclick={() => toggle(which)}
      >
        {which === 'effects' ? 'Effects…' : 'Adjust…'}
      </button>
      {#if open === which}
        <ul
          class="absolute left-0 top-full z-10 mt-1 flex min-w-48 flex-col rounded border border-neutral-700 bg-neutral-900 py-1 shadow-lg"
          role="menu"
          aria-label={which === 'effects' ? 'Effects' : 'Adjustments'}
        >
          {#each groupsFor(which) as group (group)}
            {#if which === 'effects'}
              <li
                class="px-3 pb-0.5 pt-1.5 text-[10px] uppercase tracking-wide text-neutral-500"
                role="presentation"
              >
                {GROUP_LABELS[group]}
              </li>
            {/if}
            {#each EFFECTS.filter((e) => e.group === group) as def (def.id)}
              <li>
                <button
                  class="w-full truncate px-3 py-1 text-left text-xs hover:bg-neutral-800"
                  role="menuitem"
                  onclick={() => pick(def.id)}
                >
                  {def.label}{def.params.length > 0 ? '…' : ''}
                </button>
              </li>
            {/each}
          {/each}
        </ul>
      {/if}
    </span>
  {/each}
</span>
