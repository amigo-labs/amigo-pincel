<script lang="ts">
  // The modal dialog shell both modes share (docs/specs/dialogs.md): a
  // backdrop, the shared `am-dialog` panel with a title and a close control,
  // the content, and Cancel / an optional second choice / the primary
  // button. Escape and the close control cancel, Enter on a field does what
  // the primary button does, Tab stays inside, and the focus goes to the
  // first field on open and back where it was on close.
  import { onMount, tick, type Snippet } from 'svelte';
  import IconButton from './layout/IconButton.svelte';
  import { t } from '../i18n/index.svelte';

  interface Props {
    title: string;
    /** The primary button's label: a verb (AC-04). */
    applyLabel: string;
    onApply: () => void;
    /** Cancel, the close control and Escape. */
    onClose: () => void;
    /** An extra choice between Cancel and the primary button. */
    secondary?: { label: string; onClick: () => void } | undefined;
    /** Disables the primary button, and Enter then does nothing. */
    applyDisabled?: boolean;
    /** Panel classes beside `am-dialog` (width). */
    class?: string;
    children: Snippet;
  }
  const {
    title,
    applyLabel,
    onApply,
    onClose,
    secondary,
    applyDisabled = false,
    class: panelClass = '',
    children,
  }: Props = $props();

  let panel: HTMLDivElement;
  const FOCUSABLE =
    'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
  const FIELD = 'input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled])';

  onMount(() => {
    // Where the focus goes back to (AC-07).
    const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const body = panel.querySelector<HTMLElement>('.am-body');
    const first =
      body?.querySelector<HTMLElement>(FIELD) ?? panel.querySelector<HTMLElement>('[data-dialog-primary]');
    first?.focus();
    return () => {
      void tick().then(() => {
        if (document.activeElement && document.activeElement !== document.body) return;
        if (before && before.isConnected && before.offsetParent !== null) before.focus();
        else
          document
            .querySelector<HTMLElement>('[data-editor]:not([inert]) [data-stage]')
            ?.focus({ preventScroll: true });
      });
    };
  });

  function focusables(): HTMLElement[] {
    return [...panel.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null);
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key === 'Tab') {
      // Tab wraps inside the dialog (AC-06).
      const all = focusables();
      const first = all[0];
      const last = all.at(-1);
      if (!first || !last) return;
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
      return;
    }
    if (e.key === 'Enter' && e.target instanceof Element && e.target.matches(FIELD)) {
      // A button keeps its own Enter; on a field Enter is the primary button (AC-03).
      if (e.target instanceof HTMLTextAreaElement) return;
      e.preventDefault();
      e.stopPropagation();
      if (!applyDisabled) onApply();
    }
  }
</script>

<!-- A press on the backdrop leaves the dialog open (AC-09). Keys typed in
     the dialog stop here, after the panel handled them, so a tool key never
     reaches the editors' window listeners (AC-08). Chords with Ctrl, ⌘ or
     Alt go on: the editors already leave them alone while a dialog is open,
     and the browser's own (Ctrl+P while a dialog is open, image-formats-
     more-print AC-09) must stay the browser's. -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
  role="presentation"
  {@attach (node: HTMLElement) => {
    const stop = (e: KeyboardEvent): void => {
      if (!e.ctrlKey && !e.metaKey && !e.altKey) e.stopPropagation();
    };
    node.addEventListener('keydown', stop);
    return () => node.removeEventListener('keydown', stop);
  }}
>
  <div
    bind:this={panel}
    class="am-dialog flex max-h-full max-w-full flex-col {panelClass}"
    role="dialog"
    aria-modal="true"
    aria-label={title}
    tabindex="-1"
    data-dialog=""
    {@attach (node: HTMLElement) => {
      node.addEventListener('keydown', onKeydown);
      return () => node.removeEventListener('keydown', onKeydown);
    }}
  >
    <header class="flex shrink-0 items-start justify-between gap-3">
      <h2>{title}</h2>
      <IconButton icon="close" label={t('common.close')} size="sm" onclick={onClose} />
    </header>

    <div class="am-body min-h-0 overflow-y-auto">
      {@render children()}
    </div>

    <footer class="shrink-0">
      <button type="button" class="am-btn" onclick={onClose}>{t('common.cancel')}</button>
      {#if secondary}
        <button type="button" class="am-btn" onclick={secondary.onClick}>{secondary.label}</button>
      {/if}
      <button
        type="button"
        class="am-btn am-btn--primary"
        disabled={applyDisabled}
        data-dialog-primary=""
        onclick={onApply}
      >
        {applyLabel}
      </button>
    </footer>
  </div>
</div>
