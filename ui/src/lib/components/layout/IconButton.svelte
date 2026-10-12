<script lang="ts">
  // Square icon-only button used across the editor chrome (spec §9.2).
  // `label` is both the accessible name and the tooltip.
  import Icon from '../icons/Icon.svelte';
  import type { IconName } from '../icons/icons';

  interface Props {
    icon: IconName;
    label: string;
    /** Tooltip suffix, e.g. a shortcut. */
    hint?: string;
    onclick: (e: MouseEvent) => void;
    disabled?: boolean;
    /** Toggle state; renders `aria-pressed` when set. */
    pressed?: boolean;
    size?: 'sm' | 'md';
  }
  const { icon, label, hint, onclick, disabled = false, pressed, size = 'md' }: Props = $props();
</script>

<button
  type="button"
  class="flex shrink-0 items-center justify-center rounded text-pc-muted hover:bg-pc-hover hover:text-pc-text disabled:opacity-35 disabled:hover:bg-transparent"
  class:h-8={size === 'md'}
  class:w-8={size === 'md'}
  class:h-6={size === 'sm'}
  class:w-6={size === 'sm'}
  class:bg-pc-active={pressed === true}
  class:text-pc-text={pressed === true}
  title={hint ? `${label} (${hint})` : label}
  aria-label={label}
  aria-pressed={pressed}
  {disabled}
  {onclick}
>
  <Icon name={icon} size={size === 'md' ? 18 : 15} />
</button>
