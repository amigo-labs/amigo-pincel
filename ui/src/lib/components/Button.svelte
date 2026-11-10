<script lang="ts">
  // The text button both editors and the shell use (spec §9.2). Variants,
  // the busy state and the shortcut keycap live here; the look is
  // `.panel-btn` in app.css, so markup that still writes the classes by
  // hand stays in step.
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Icon from './icons/Icon.svelte';
  import type { IconName } from './icons/icons';

  interface Props extends HTMLButtonAttributes {
    /** `primary`: the one main action; `ghost`: repeated row actions;
     *  `danger`: an action that loses work. */
    variant?: 'default' | 'primary' | 'ghost' | 'danger';
    /** The action runs: a spinner shows and the button takes no clicks. */
    busy?: boolean;
    /** Icon before the label. */
    icon?: IconName;
    /** Key that triggers the button, shown as a keycap after the label.
     *  Not part of the accessible name. */
    shortcut?: string;
    /** The button element, for focus management. */
    el?: HTMLButtonElement | undefined;
    children: Snippet;
  }
  let {
    variant = 'default',
    busy = false,
    icon,
    shortcut,
    el = $bindable(),
    children,
    class: className = '',
    type = 'button',
    disabled = false,
    ...rest
  }: Props = $props();
</script>

<button
  bind:this={el}
  {type}
  class={['panel-btn', variant !== 'default' && `panel-btn-${variant}`, className]}
  aria-busy={busy ? 'true' : undefined}
  disabled={disabled || busy}
  {...rest}
>
  {#if busy}
    <svg
      class="btn-spinner"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      aria-hidden="true"
    >
      <circle cx="8" cy="8" r="5.5" opacity="0.35" />
      <path d="M8 2.5a5.5 5.5 0 0 1 5.5 5.5" />
    </svg>
  {:else if icon}
    <Icon name={icon} size={14} />
  {/if}
  {@render children()}
  {#if shortcut}
    <kbd class="keycap" aria-hidden="true">{shortcut}</kbd>
  {/if}
</button>
