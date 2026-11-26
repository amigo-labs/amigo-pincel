<script lang="ts" module>
  /** What an editor hands the frame to draw the zoom control
   *  (docs/specs/frame-layout.md AC-09, AC-10). */
  export interface ZoomProps {
    /** Current zoom in the mode's format, e.g. `125%` or `8×`. */
    label: string;
    /** Current zoom as a multiple of actual size. */
    factor: number;
    /** The mode's zoom range, both ends included. */
    min: number;
    max: number;
    /** A zoom factor in the mode's format. */
    format: (factor: number) => string;
    onZoomIn: () => void;
    onZoomOut: () => void;
    onFit: () => void;
    /** Sets the zoom, keeping the centre of the canvas area in place. */
    onSet: (factor: number) => void;
  }

  /** The zoom steps the menu offers, cut to a mode's range. */
  export const ZOOM_STEPS: readonly number[] = [0.25, 0.5, 1, 2, 4, 8, 16, 32, 64];
</script>

<script lang="ts">
  // The zoom control both modes share (docs/specs/frame-layout.md AC-09 –
  // AC-11): out · current zoom · in. The zoom value opens a menu with Fit
  // to Window and the fixed zoom steps of the mode's range. The frame
  // places it: last in the status bar, or floating over the canvas corner
  // on a narrow window (`floating`, lifted by `lift` px above a sheet).
  import { tick } from 'svelte';
  import Icon from '../icons/Icon.svelte';
  import IconButton from './IconButton.svelte';
  import { activeOnFocus } from './menu-focus';
  import { hint } from '../../shell/shortcuts';
  import { t } from '../../i18n/index.svelte';

  interface Props extends ZoomProps {
    floating?: boolean;
    /** Floating only: distance from the canvas area's bottom edge, px. */
    lift?: number;
    disabled?: boolean;
    /** Changes whenever the open menu has to close without a choice: the
     *  active tab, an opening effect, the window's size class (AC-10). */
    closeKey?: unknown;
  }
  const {
    label,
    factor,
    min,
    max,
    format,
    onZoomIn,
    onZoomOut,
    onFit,
    onSet,
    floating = false,
    lift = 12,
    disabled = false,
    closeKey = null,
  }: Props = $props();

  const steps = $derived(ZOOM_STEPS.filter((s) => s >= min && s <= max));

  let open = $state(false);
  let valueButton: HTMLButtonElement | undefined = $state();
  let menu: HTMLDivElement | undefined = $state();
  let anchor = $state({ right: 0, bottom: 0 });

  function show(): void {
    if (!valueButton) return;
    const r = valueButton.getBoundingClientRect();
    // The menu opens upwards, its right edge on the button's.
    anchor = {
      right: Math.max(4, window.innerWidth - r.right),
      bottom: window.innerHeight - r.top + 4,
    };
    open = true;
    void tick().then(() => items()[0]?.focus());
  }

  function close(refocus: boolean): void {
    if (!open) return;
    open = false;
    if (refocus) valueButton?.focus();
  }

  function choose(run: () => void): void {
    run();
    close(true);
  }

  function items(): HTMLElement[] {
    return menu ? Array.from(menu.querySelectorAll<HTMLElement>('[role^="menuitem"]')) : [];
  }

  function onKeydown(e: KeyboardEvent): void {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
      return;
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      e.stopPropagation();
      const list = items();
      if (list.length === 0) return;
      const at = list.indexOf(document.activeElement as HTMLElement);
      const next =
        e.key === 'Home'
          ? 0
          : e.key === 'End'
            ? list.length - 1
            : (at + (e.key === 'ArrowDown' ? 1 : -1) + list.length) % list.length;
      list[next]?.focus();
      return;
    }
    if (e.key === 'Enter' && items().includes(document.activeElement as HTMLElement)) {
      // Enter chooses the focused entry in both modes; it never reaches the
      // editor's own Enter (Play / Pause in Pixel mode, AC-19).
      e.preventDefault();
      e.stopPropagation();
      (document.activeElement as HTMLElement).click();
    }
  }

  // The menu closes without a choice when its context changes.
  let seenKey: unknown = undefined;
  $effect(() => {
    const key = closeKey;
    if (seenKey !== undefined && key !== seenKey) close(false);
    seenKey = key;
  });
  $effect(() => {
    if (disabled) close(false);
  });
</script>

<svelte:window onkeydowncapture={onKeydown} onresize={() => close(false)} onblur={() => close(false)} />

{#snippet checkMark()}
  <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5l3 3 6-7" /></svg>
{/snippet}

<div
  class={[
    'flex shrink-0 items-center gap-0.5 text-xs',
    floating
      ? 'absolute right-3 z-(--pc-z-float-control) rounded-pc-control border border-pc-border bg-pc-panel/90 p-0.5 shadow-lg backdrop-blur'
      : 'h-full px-1.5',
  ]}
  style:bottom={floating ? `${lift}px` : undefined}
  role="group"
  aria-label={t('zoom.label')}
  data-zoom-control=""
>
  <IconButton
    icon="zoom-out"
    label={t('zoom.out')}
    hint={hint('zoomOut')}
    size="sm"
    onclick={onZoomOut}
    disabled={disabled || factor <= min}
  />
  <button
    bind:this={valueButton}
    type="button"
    class="am-btn am-btn--ghost am-btn--sm min-w-14 gap-0.5 px-1 tabular-nums"
    title={t('zoom.label')}
    aria-label={t('zoom.value', { zoom: label })}
    aria-haspopup="menu"
    aria-expanded={open}
    {disabled}
    onclick={() => (open ? close(true) : show())}
  >
    {label}
    <Icon name="chevron-down" size={12} class="shrink-0 text-pc-faint" />
  </button>
  <IconButton
    icon="zoom-in"
    label={t('zoom.in')}
    hint={hint('zoomIn')}
    size="sm"
    onclick={onZoomIn}
    disabled={disabled || factor >= max}
  />
</div>

{#if open}
  <!-- A click outside closes the menu and goes nowhere else, as for the
       menu bar's menus. -->
  <button
    type="button"
    class="fixed inset-0 z-(--pc-z-popover) cursor-default"
    aria-hidden="true"
    tabindex="-1"
    onclick={() => close(false)}
  ></button>
  <div
    bind:this={menu}
    class="am-menu fixed z-(--pc-z-popover-control) max-h-[70vh] min-w-44 overflow-y-auto *:shrink-0"
    style:right="{anchor.right}px"
    style:bottom="{anchor.bottom}px"
    role="menu"
    aria-label={t('zoom.label')}
  >
    <button type="button" class="w-full" use:activeOnFocus role="menuitem" onclick={() => choose(onFit)}>
      <span class="am-menu-label"
        ><span class="am-menu-check align-middle" aria-hidden="true"></span>{t('menu.zoomFit')}</span
      >
      {#if hint('zoomFit')}<kbd class="am-kbd" aria-hidden="true">{hint('zoomFit')}</kbd>{/if}
    </button>
    <hr />
    {#each steps as step (step)}
      {@const checked = factor === step}
      <button
        type="button"
        class="w-full"
        use:activeOnFocus
        role="menuitemcheckbox"
        aria-checked={checked}
        onclick={() => choose(() => onSet(step))}
      >
        <span class="am-menu-label"
          ><span class="am-menu-check align-middle text-pc-accent-text" aria-hidden="true"
            >{#if checked}{@render checkMark()}{/if}</span
          >{format(step)}</span
        >
        {#if step === 1 && hint('zoomActual')}
          <kbd class="am-kbd" aria-hidden="true">{hint('zoomActual')}</kbd>
        {/if}
      </button>
    {/each}
  </div>
{/if}
