<script lang="ts">
  // A dropdown of grouped effects / adjustments; each item opens the shared
  // effect dialog with a live preview (spec §11, §12). Both modes use one
  // catalog and one wire format (`lib/effects/catalog.ts`, spec §15,
  // 2026-10-11); this menu lists the `groups` it is given — the Effects
  // menu the four effect groups, the Adjustments menu `adjust`.
  import {
    EFFECTS,
    EFFECT_MENU_GROUPS,
    GROUP_LABELS,
    type EffectDef,
    type EffectGroup,
  } from '../../../../lib/effects/catalog';
  import EffectDialog from '../../../../lib/components/EffectDialog.svelte';
  import { applyEffect, clearEffectPreview, previewEffect } from '../../core/controller';
  import { editor, ui } from '../../stores/editor.svelte';

  interface Props {
    label?: string;
    groups?: EffectGroup[];
  }
  const { label = 'Effects', groups = EFFECT_MENU_GROUPS }: Props = $props();

  const sections = $derived(
    groups.map((g) => ({ group: g, items: EFFECTS.filter((e) => e.group === g) })),
  );

  let open = $state(false);
  let active = $state<EffectDef | null>(null);

  // The dialog owns Enter / Escape; keep the editor's single-key
  // shortcuts out while it is open.
  $effect(() => {
    ui.modalOpen = active !== null;
  });

  function choose(def: EffectDef): void {
    open = false;
    active = def;
  }

  function close(): void {
    clearEffectPreview();
    active = null;
  }
</script>

<div class="relative flex items-center">
  <button
    class="rounded px-2 py-1 hover:bg-neutral-700"
    class:bg-neutral-700={open}
    onclick={() => (open = !open)}
    disabled={editor.handle === null}
  >
    {label}
  </button>

  {#if open}
    <!-- Backdrop closes the menu on an outside click. -->
    <button
      class="fixed inset-0 z-40 cursor-default"
      aria-label="Close menu"
      onclick={() => (open = false)}
    ></button>
    <div
      class="absolute left-0 top-8 z-50 max-h-[70vh] w-52 overflow-y-auto rounded border border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)] py-1 text-sm shadow-xl"
      role="menu"
    >
      {#each sections as section, i (section.group)}
        {#if i > 0}
          <div class="my-1 border-t border-[var(--fl-panel-border)]"></div>
        {/if}
        {#if sections.length > 1}
          <div class="px-3 py-0.5 text-xs uppercase tracking-wide text-neutral-500">
            {GROUP_LABELS[section.group]}
          </div>
        {/if}
        {#each section.items as item (item.id)}
          <button
            class="block w-full px-3 py-1 text-left hover:bg-neutral-700"
            role="menuitem"
            onclick={() => choose(item)}
          >
            {item.label}{item.params.length > 0 ? '…' : ''}
          </button>
        {/each}
      {/each}
    </div>
  {/if}
</div>

{#if active}
  {#key active.id}
    <EffectDialog
      def={active}
      onPreview={(params) => {
        if (!active) return;
        // An empty vector means "preview off": show the real composite.
        if (params.length === 0 && active.params.length > 0) clearEffectPreview();
        else previewEffect(active.id, params);
      }}
      onApply={(params) => {
        if (active) applyEffect(active.id, params);
        active = null;
      }}
      onCancel={close}
    />
  {/key}
{/if}
