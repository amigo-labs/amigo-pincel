<script lang="ts" module>
  // "Show every option" is the user's choice for the session, for both
  // editors: it holds across canvas presses and tool changes (AC-06).
  const all = $state({ wanted: false });
</script>

<script lang="ts">
  // The options bar (docs/specs/options-bar-layout.md): the active tool's
  // name, then its options, in one row that never scrolls sideways. The
  // options wrap; the row clips every line but the first, and the More
  // control expands the bar over the canvas area to show all of them. There
  // is one copy of the options, so focus and bound values stay where they
  // are.
  import { onMount, type Snippet } from 'svelte';
  import Icon from '../icons/Icon.svelte';
  import { t } from '../../i18n/index.svelte';

  interface Props {
    /** The active tool's name. */
    name: string;
    /** Phone layout: a 44 px row. */
    narrow?: boolean;
    /** The bar takes no input (an effect is open, an answer runs): an
     *  expanded bar closes (AC-09). */
    disabled?: boolean;
    children: Snippet;
  }
  const { name, narrow = false, disabled = false, children }: Props = $props();

  let host = $state<HTMLElement | null>(null);
  let bar = $state<HTMLElement | null>(null);
  /** At least one option is not in the first row. */
  let overflowing = $state(false);
  /** The bar shows every option: the user asked for it, there is more than
   *  one row of them, and the bar takes input (AC-09). */
  const expanded = $derived(all.wanted && overflowing && !disabled);

  /** Whether `el`, a direct child of the bar, sits below the first row. */
  function wrapped(el: HTMLElement): boolean {
    const first = bar?.firstElementChild;
    return first instanceof HTMLElement && el.offsetTop >= first.offsetTop + first.offsetHeight - 1;
  }

  /** Room the More control takes at the row's end, in px. */
  const MORE_ROOM = 32;

  // Whether the options fit one row, from their widths rather than from
  // where they wrapped: the row reserves room for the More control only
  // while it shows, so asking the wrapped layout would keep the control
  // once it had appeared (a bar a few px too narrow with the room taken).
  function measure(): void {
    if (!bar) return;
    const style = getComputedStyle(bar);
    const gap = Number.parseFloat(style.columnGap) || 0;
    const items = [...bar.children].filter((el): el is HTMLElement => el instanceof HTMLElement);
    const need = items.reduce((sum, el) => sum + el.getBoundingClientRect().width, 0) + gap * Math.max(0, items.length - 1);
    const room =
      bar.getBoundingClientRect().width -
      Number.parseFloat(style.paddingLeft) -
      Number.parseFloat(style.paddingRight) +
      (overflowing ? MORE_ROOM : 0);
    overflowing = need > room + 0.5;
  }

  onMount(() => {
    if (!bar) return;
    const resize = new ResizeObserver(measure);
    resize.observe(bar);
    const change = new MutationObserver(measure);
    change.observe(bar, { childList: true, subtree: true, characterData: true });
    // Listeners on the toolbar itself: it is a container, not a control.
    // Escape counts on the More control too, which sits beside the bar.
    const el = bar;
    const around = host ?? bar;
    el.addEventListener('focusin', onFocusIn);
    around.addEventListener('keydown', onKeydown);
    measure();
    return () => {
      resize.disconnect();
      change.disconnect();
      el.removeEventListener('focusin', onFocusIn);
      around.removeEventListener('keydown', onKeydown);
    };
  });

  // Focus on an option outside the row brings it into view (AC-07).
  function onFocusIn(e: FocusEvent): void {
    if (expanded || !bar || !(e.target instanceof Node)) return;
    const child = [...bar.children].find((el) => el.contains(e.target as Node));
    if (child instanceof HTMLElement && wrapped(child)) all.wanted = true;
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key !== 'Escape' || !expanded) return;
    e.preventDefault();
    e.stopPropagation();
    all.wanted = false;
  }
</script>

<!-- Always one row high in the layout: the expanded bar lies over the
     canvas area, so opening it, or options that come and go while it is
     open, never move the canvas (AC-05). -->
<div bind:this={host} class="relative shrink-0" class:h-11={narrow} class:h-10={!narrow}>
  <div
    bind:this={bar}
    class={[
      'options-bar flex flex-wrap items-center gap-x-3 border-b border-pc-border bg-pc-panel pl-3 text-xs text-pc-muted',
      narrow ? 'options-bar--narrow' : '',
      overflowing ? 'pr-11' : 'pr-3',
      expanded
        ? 'absolute inset-x-0 top-0 z-20 max-h-[40vh] overflow-y-auto pb-1.5 shadow-(--am-elev-2)'
        : // `clip`, not `hidden`: nothing may scroll a wrapped line into the row.
          'h-full overflow-clip',
    ]}
    data-frame="options"
    data-expanded={expanded ? '' : undefined}
    role="toolbar"
    aria-label={t('frame.toolOptions')}
  >
    <!-- The name is as high as the row, so the first line is the row and
         every wrapped line lies below the clip. -->
    <span class="options-name flex shrink-0 items-center font-semibold text-pc-text" data-tool-name="">{name}</span>
    {@render children()}
  </div>
  {#if overflowing}
    <button
      type="button"
      class="btn-press absolute right-1.5 z-[21] flex h-7 w-7 items-center justify-center rounded-pc-control text-pc-muted hover:bg-pc-hover hover:text-pc-text"
      class:top-2={narrow}
      class:top-1.5={!narrow}
      class:btn-on={expanded}
      aria-label={t('frame.moreOptions')}
      title={t('frame.moreOptions')}
      aria-expanded={expanded}
      data-more-options=""
      onclick={() => {
        all.wanted = !expanded;
        if (bar) bar.scrollTop = 0;
      }}
    >
      <Icon name="more" size={16} />
    </button>
  {/if}
</div>

<style>
  .options-bar {
    /* Wrapped lines start below the row while the bar is one row. */
    row-gap: 3rem;
    align-content: flex-start;
  }
  .options-bar[data-expanded] {
    row-gap: 0.25rem;
  }
  .options-name {
    min-height: 2.5rem;
  }
  .options-bar--narrow .options-name {
    min-height: 2.75rem;
  }
  /* Each option keeps its width and stays on one line, so it is wholly in
     the row or wholly out of it (AC-02). */
  .options-bar > :global(*) {
    flex-shrink: 0;
    white-space: nowrap;
  }
  /* Once the bar shows every option, one wider than the bar wraps. */
  .options-bar[data-expanded] > :global(*) {
    max-width: 100%;
    flex-shrink: 1;
  }
  .options-bar[data-expanded] > :global(span) {
    white-space: normal;
  }
</style>
