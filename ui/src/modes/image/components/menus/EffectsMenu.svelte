<script lang="ts">
  // A dropdown of grouped effects/adjustments; each item opens the shared effect
  // dialog with a live preview (spec §11, §12). Reused for both menus via props.
  import { EFFECT_GROUPS, type EffectDef, type EffectGroup } from './effects';
  import EffectDialog from '../dialogs/EffectDialog.svelte';

  interface Props {
    label?: string;
    groups?: EffectGroup[];
  }
  const { label = 'Effects', groups = EFFECT_GROUPS }: Props = $props();

  let open = $state(false);
  let active = $state<EffectDef | null>(null);

  function choose(def: EffectDef): void {
    open = false;
    active = def;
  }
</script>

<div class="relative flex items-center">
  <button
    class="rounded px-2 py-1 hover:bg-neutral-700"
    class:bg-neutral-700={open}
    onclick={() => (open = !open)}
  >
    {label}
  </button>

  {#if open}
    <!-- Backdrop closes the menu on an outside click. -->
    <button class="fixed inset-0 z-40 cursor-default" aria-label="Close menu" onclick={() => (open = false)}
    ></button>
    <div
      class="absolute left-0 top-8 z-50 max-h-[70vh] w-52 overflow-y-auto rounded border border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)] py-1 text-sm shadow-xl"
    >
      {#each groups as group, i (group.group)}
        {#if i > 0}
          <div class="my-1 border-t border-[var(--fl-panel-border)]"></div>
        {/if}
        <div class="px-3 py-0.5 text-xs uppercase tracking-wide text-neutral-500">{group.group}</div>
        {#each group.items as item (item.label)}
          <button class="block w-full px-3 py-1 text-left hover:bg-neutral-700" onclick={() => choose(item)}>
            {item.label}
          </button>
        {/each}
      {/each}
    </div>
  {/if}
</div>

{#if active}
  <EffectDialog def={active} onClose={() => (active = null)} />
{/if}
