<script lang="ts">
  // One icon from `icons.ts`, drawn in `currentColor` (spec §9.2).
  import { ICONS, type IconName, type IconPath } from './icons';

  interface Props {
    name: IconName;
    /** Edge length in CSS pixels. */
    size?: number;
    class?: string;
  }
  const { name, size = 18, class: className = '' }: Props = $props();

  const paths: readonly IconPath[] = $derived(ICONS[name]);
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#each paths as p, i (i)}
    <path
      d={p.d}
      fill={p.fill ? 'currentColor' : 'none'}
      stroke-dasharray={p.dash ? '2.5 2.5' : undefined}
    />
  {/each}
</svg>
