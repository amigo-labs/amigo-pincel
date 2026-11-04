<script lang="ts">
  // Test harness around `ToolRail` (see tool-rail.ts): one group with a
  // disabled middle variant, one single button.
  import ToolRail from '../../src/lib/components/layout/ToolRail.svelte';
  import type { ToolGroup } from '../../src/lib/components/layout/tools';

  type Id = 'one' | 'two' | 'three' | 'solo';

  let disabled = $state<Record<string, boolean>>({ two: true });
  let active = $state<Id>('solo');
  let doc = $state({ id: 1 });

  const groups: ToolGroup<Id>[] = $derived([
    {
      id: 'variants',
      label: 'Variants',
      shortcut: 'V',
      tools: [
        { id: 'one', label: 'One', icon: 'rect', disabled: disabled['one'] ?? false },
        { id: 'two', label: 'Two', icon: 'ellipse', disabled: disabled['two'] ?? false },
        { id: 'three', label: 'Three', icon: 'polygon', disabled: disabled['three'] ?? false },
      ],
    },
    { id: 'solo', label: 'Solo', tools: [{ id: 'solo', label: 'Solo', icon: 'pencil' }] },
  ]);

  /** Enables or disables one variant. */
  export function setDisabled(id: string, value: boolean): void {
    disabled[id] = value;
  }

  /** Swaps the document identity the rail sees. */
  export function replaceDocument(): void {
    doc = { id: doc.id + 1 };
  }
</script>

<ToolRail {groups} {active} onSelect={(next) => (active = next)} resetKey={doc} />
<output class="m-4" aria-label="Active tool">{active}</output>
