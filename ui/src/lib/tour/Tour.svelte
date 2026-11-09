<script lang="ts">
  // The first-run tour's overlay (docs/specs/first-run-tutorial.md AC-04 –
  // AC-09): a highlight around the step's anchor and a callout with one
  // sentence, Back / Next / Skip and an "n / N" counter. A step whose anchor
  // is missing in this layout centres its callout (AC-06). Keyboard: Esc
  // skips, → and Enter go on, ← goes back; focus stays in the callout.
  import { tick } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import { TOUR_STEPS, type Anchor } from './steps';
  import { endTour, tour } from './tour-state.svelte';

  const steps = $derived(tour.context ? TOUR_STEPS[tour.context] : []);
  const step = $derived(steps[tour.step] ?? null);
  const last = $derived(tour.step >= steps.length - 1);

  let rect = $state<DOMRect | null>(null);
  let viewport = $state({ w: 0, h: 0 });
  let callout: HTMLElement | undefined = $state();
  let calloutSize = $state({ w: 320, h: 140 });

  function find(anchor: Anchor): HTMLElement | null {
    if (!anchor) return null;
    let candidates: HTMLElement[];
    if ('tour' in anchor) {
      candidates = [...document.querySelectorAll<HTMLElement>(`[data-tour="${anchor.tour}"]`)];
    } else {
      const implicit = anchor.role === 'complementary' ? 'aside, ' : '';
      candidates = [
        ...document.querySelectorAll<HTMLElement>(`${implicit}[role="${anchor.role}"]`),
      ];
      if (anchor.name) {
        const name = t(anchor.name);
        candidates = candidates.filter((el) => el.getAttribute('aria-label') === name);
      }
    }
    return (
      candidates.find((el) => {
        const r = el.getBoundingClientRect();
        return r.width > 0 && r.height > 0 && !el.closest('[data-tour-overlay]');
      }) ?? null
    );
  }

  function measure(): void {
    viewport = { w: window.innerWidth, h: window.innerHeight };
    rect = step ? (find(step.anchor)?.getBoundingClientRect() ?? null) : null;
    if (callout) calloutSize = { w: callout.offsetWidth, h: callout.offsetHeight };
  }

  // Each step: measure its anchor, focus the callout's main button and let
  // the live region announce the text (AC-08).
  $effect(() => {
    void tour.step;
    void tour.context;
    measure();
    void tick().then(() => {
      measure();
      callout?.querySelector<HTMLElement>('[data-tour-next]')?.focus();
    });
  });

  const PAD = 6;
  const GAP = 10;
  const position = $derived.by(() => {
    const { w, h } = calloutSize;
    const vw = viewport.w;
    const vh = viewport.h;
    const clampX = (x: number) => Math.max(8, Math.min(x, vw - w - 8));
    const clampY = (y: number) => Math.max(8, Math.min(y, vh - h - 8));
    if (!rect) return { left: clampX((vw - w) / 2), top: clampY((vh - h) / 2) };
    const below = rect.bottom + PAD + GAP;
    const above = rect.top - PAD - GAP - h;
    const top = below + h <= vh - 8 ? below : above >= 8 ? above : (vh - h) / 2;
    return { left: clampX(rect.left + rect.width / 2 - w / 2), top: clampY(top) };
  });

  function next(): void {
    if (last) endTour();
    else tour.step += 1;
  }

  function back(): void {
    if (tour.step > 0) tour.step -= 1;
  }

  function onKeydown(e: KeyboardEvent): void {
    if (!tour.context) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      endTour();
    } else if (e.key === 'ArrowRight' || (e.key === 'Enter' && !(e.target instanceof HTMLButtonElement))) {
      e.preventDefault();
      e.stopPropagation();
      next();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      e.stopPropagation();
      back();
    } else if (e.key === 'Tab' && callout) {
      // Focus stays inside the callout (AC-07).
      const items = [...callout.querySelectorAll<HTMLElement>('button')];
      const first = items[0];
      const end = items.at(-1);
      if (!first || !end) return;
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        end.focus();
      } else if (!e.shiftKey && document.activeElement === end) {
        e.preventDefault();
        first.focus();
      } else if (!callout.contains(document.activeElement)) {
        e.preventDefault();
        first.focus();
      }
    } else {
      e.stopPropagation();
    }
  }
</script>

<svelte:window onresize={measure} onkeydowncapture={onKeydown} />

{#if tour.context && step}
  <div class="fixed inset-0 z-50" data-tour-overlay>
    {#if rect}
      <div
        class="pointer-events-none absolute rounded-md ring-2 ring-pc-accent motion-safe:transition-all motion-safe:duration-200"
        style:left="{rect.left - PAD}px"
        style:top="{rect.top - PAD}px"
        style:width="{rect.width + PAD * 2}px"
        style:height="{rect.height + PAD * 2}px"
        style:box-shadow="0 0 0 9999px rgb(0 0 0 / 0.55)"
        data-tour-highlight
      ></div>
    {:else}
      <div class="absolute inset-0 bg-black/55"></div>
    {/if}
    <div
      bind:this={callout}
      role="dialog"
      aria-modal="true"
      aria-label={t('tour.title')}
      class="absolute flex w-80 max-w-[calc(100vw-16px)] flex-col gap-3 rounded-lg border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl motion-safe:transition-all motion-safe:duration-200"
      style:left="{position.left}px"
      style:top="{position.top}px"
      data-tour-callout
    >
      <p aria-live="polite" aria-atomic="true">{t(step.text)}</p>
      <div class="flex items-center gap-2">
        <span class="text-xs tabular-nums text-pc-muted" data-tour-counter
          >{tour.step + 1} / {steps.length}</span
        >
        <span class="flex-1"></span>
        <button type="button" class="panel-btn" onclick={endTour}>{t('tour.skip')}</button>
        <button type="button" class="panel-btn" disabled={tour.step === 0} onclick={back}
          >{t('tour.back')}</button
        >
        <button type="button" class="panel-btn panel-btn-primary" data-tour-next onclick={next}
          >{last ? t('tour.done') : t('tour.next')}</button
        >
      </div>
    </div>
  </div>
{/if}
