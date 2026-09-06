<script lang="ts">
  // Shared effect/adjustment dialog (spec §11, §12): renders parameter controls
  // for one effect, shows a debounced live preview on the canvas, commits on
  // Apply. Supports scalar, array-element (index), boolean and select fields.
  import { onMount, untrack } from 'svelte';
  import type { EffectCommand } from '../../core/wasm';
  import { applyEffect, previewEffect, clearEffectPreview } from '../../core/controller';
  import type { EffectDef, Field } from '../menus/effects';
  import Modal from './Modal.svelte';

  interface Props {
    def: EffectDef;
    onClose: () => void;
  }
  const { def, onClose }: Props = $props();

  type Value = number | string | boolean;

  /** Unique control id: the field key, plus the array index when present. */
  function fieldId(f: Field): string {
    return f.index === undefined ? f.key : `${f.key}.${f.index}`;
  }

  /** Reads a field's initial value from the default command. */
  function initial(f: Field): Value {
    const rec = base as unknown as Record<string, Value | Value[]>;
    const v = rec[f.key];
    return f.index === undefined ? (v as Value) : (v as Value[])[f.index];
  }

  // `base` carries the effect type plus any non-UI fields (e.g. radial centre).
  // `def` is fixed for the dialog's lifetime, so these initial reads are
  // intentional (untrack silences the state-referenced-locally lint).
  const base = untrack(() => def.make());
  const params = $state<Record<string, Value>>(
    untrack(() => Object.fromEntries(def.fields.map((f) => [fieldId(f), initial(f)]))),
  );

  /** Rebuilds the command by writing each field's value into a clone of base. */
  function command(): EffectCommand {
    const cmd = structuredClone(base) as unknown as Record<string, Value | Value[]>;
    for (const f of def.fields) {
      const v = params[fieldId(f)];
      if (f.index === undefined) {
        cmd[f.key] = v;
      } else {
        (cmd[f.key] as Value[])[f.index] = v;
      }
    }
    return cmd as unknown as EffectCommand;
  }

  let timer: ReturnType<typeof setTimeout> | undefined;

  // Re-preview (debounced) whenever a parameter changes; the first run previews
  // the defaults as the dialog opens.
  $effect(() => {
    const cmd = command();
    clearTimeout(timer);
    timer = setTimeout(() => previewEffect(cmd), 120);
  });

  onMount(() => () => clearTimeout(timer));

  function apply(): void {
    clearTimeout(timer);
    applyEffect(command());
    onClose();
  }

  function close(): void {
    clearTimeout(timer);
    clearEffectPreview();
    onClose();
  }
</script>

<Modal title={def.title} applyLabel="Apply" onApply={apply} onClose={close}>
  {#if def.fields.length === 0}
    <p class="mb-3 text-neutral-400">Applies immediately — preview shown on the canvas.</p>
  {/if}
  {#each def.fields as field (fieldId(field))}
    <label class="mb-2 flex items-center justify-between gap-3">
      <span class="text-neutral-400">{field.label}</span>
      {#if field.kind === 'range'}
        <span class="flex items-center gap-2">
          <input
            type="range"
            min={field.min}
            max={field.max}
            step={field.step}
            value={params[fieldId(field)] as number}
            oninput={(e) => (params[fieldId(field)] = e.currentTarget.valueAsNumber)}
          />
          <span class="w-12 text-right tabular-nums">{params[fieldId(field)]}</span>
        </span>
      {:else if field.kind === 'toggle'}
        <input
          type="checkbox"
          checked={params[fieldId(field)] as boolean}
          onchange={(e) => (params[fieldId(field)] = e.currentTarget.checked)}
        />
      {:else}
        <select
          value={params[fieldId(field)] as string}
          onchange={(e) => (params[fieldId(field)] = e.currentTarget.value)}
          class="rounded border border-[var(--fl-panel-border)] bg-neutral-800 px-2 py-1"
        >
          {#each field.options as opt (opt.value)}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
      {/if}
    </label>
  {/each}
</Modal>
