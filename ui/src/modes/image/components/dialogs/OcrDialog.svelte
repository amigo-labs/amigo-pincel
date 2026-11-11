<script lang="ts">
  // Tools ▸ Recognize Text… (docs/specs/ocr-dialog.md): a modal dialog that
  // reads the text in the document's source area — the model downloaded
  // on first use — and shows it in an editable field with Copy. The
  // document never changes: the dialog works on a snapshot.
  import { onDestroy, onMount, tick } from 'svelte';
  import Button from '../../../../lib/components/Button.svelte';
  import { locale, t, type MessageKey } from '../../../../lib/i18n/index.svelte';
  import { OCR_LANGS, type OcrLang } from '../../ocr/catalog';
  import { preselectedOcrLanguage, rememberOcrLanguage } from '../../ocr/language';
  import { startOcr, type OcrFailure, type OcrRun, type OcrStep } from '../../ocr/run';

  interface Props {
    /** `snapshotDocument` bytes of the document to read. */
    snapshot: Uint8Array;
    onClose: () => void;
  }
  const { snapshot, onClose }: Props = $props();

  type View =
    | { readonly kind: 'work'; readonly step: OcrStep | null }
    | { readonly kind: 'done'; readonly empty: boolean }
    | { readonly kind: 'error'; readonly failure: OcrFailure };

  const LANG_LABELS: Record<OcrLang, MessageKey> = { deu: 'ocr.lang.deu', eng: 'ocr.lang.eng' };
  const DOWNLOADING: Record<OcrLang, MessageKey> = {
    deu: 'ocr.downloading.deu',
    eng: 'ocr.downloading.eng',
  };
  const FAILURES: Record<OcrFailure, MessageKey> = {
    verify: 'ocr.verifyFailed',
    download: 'ocr.downloadFailed',
    failed: 'ocr.failed',
  };

  let lang = $state<OcrLang | null>(null);
  let view = $state<View>({ kind: 'work', step: null });
  let text = $state('');
  let copied = $state(false);
  let copyFailed = $state(false);
  let field = $state<HTMLTextAreaElement | undefined>();
  let languageSelect = $state<HTMLSelectElement | undefined>();

  let run: OcrRun | null = null;
  // Bumped by every start and by close: an older run's reports are stale.
  let generation = 0;

  const megabytes = (bytes: number): string =>
    new Intl.NumberFormat(locale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(
      bytes / 1_000_000,
    );

  const status = $derived.by((): string => {
    if (view.kind === 'error') return t(FAILURES[view.failure]);
    if (view.kind === 'done') return view.empty ? t('ocr.noText') : '';
    const step = view.step;
    if (step === null) return '';
    if (step.step === 'download') {
      const bytes =
        step.total === null
          ? t('ocr.bytes', { loaded: megabytes(step.loaded) })
          : t('ocr.bytesOf', { loaded: megabytes(step.loaded), total: megabytes(step.total) });
      return `${t(DOWNLOADING[step.lang])} ${bytes}`;
    }
    if (step.step === 'lines' && step.total > 0) {
      return t('ocr.line', { k: Math.min(step.done + 1, step.total), m: step.total });
    }
    return t('ocr.finding');
  });

  const canCopy = $derived(text.trim() !== '');

  /** Starts (or restarts) the recognition in the chosen language. */
  function start(): void {
    if (lang === null) return;
    run?.cancel();
    generation += 1;
    const mine = generation;
    const chosen = lang;
    view = { kind: 'work', step: null };
    text = '';
    copied = false;
    copyFailed = false;
    const current = startOcr(snapshot, chosen, {
      step: (step) => {
        if (mine === generation) view = { kind: 'work', step };
      },
      recognizing: () => rememberOcrLanguage(chosen),
    });
    run = current;
    void current.outcome.then(async (outcome) => {
      if (mine !== generation || outcome === null) return;
      run = null;
      if (!outcome.ok) {
        view = { kind: 'error', failure: outcome.failure };
        return;
      }
      text = outcome.text;
      view = { kind: 'done', empty: outcome.text.trim() === '' };
      await tick();
      if (field) {
        field.focus();
        field.setSelectionRange(0, 0);
        field.scrollTop = 0;
      }
    });
  }

  function close(): void {
    generation += 1;
    run?.cancel();
    run = null;
    onClose();
  }

  async function copy(): Promise<void> {
    if (!canCopy) return;
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      copyFailed = false;
    } catch {
      copied = false;
      copyFailed = true;
    }
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    close();
  }

  onMount(() => {
    void preselectedOcrLanguage(locale()).then(async (preselected) => {
      if (lang !== null) return;
      lang = preselected;
      await tick();
      languageSelect?.focus();
      start();
    });
  });

  onDestroy(() => {
    generation += 1;
    run?.cancel();
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50" role="presentation">
  <div
    class="flex w-[32rem] max-w-[calc(100vw-2rem)] flex-col gap-3 rounded-lg border border-pc-border bg-pc-panel p-4 text-sm text-pc-text"
    role="dialog"
    aria-modal="true"
    aria-label={t('ocr.title')}
    tabindex="-1"
  >
    <h2 class="text-sm font-semibold">{t('ocr.title')}</h2>

    <div class="flex items-center gap-2">
      <label for="ocr-language">{t('ocr.language')}</label>
      <select
        id="ocr-language"
        bind:this={languageSelect}
        class="rounded border border-pc-border bg-pc-raised px-2 py-1"
        value={lang ?? ''}
        onchange={(e) => {
          const next = e.currentTarget.value;
          if (next === 'deu' || next === 'eng') {
            lang = next;
            start();
          }
        }}
      >
        {#each OCR_LANGS as id (id)}
          <option value={id}>{t(LANG_LABELS[id])}</option>
        {/each}
      </select>
    </div>

    {#if view.kind === 'done' && !view.empty}
      <textarea
        bind:this={field}
        bind:value={text}
        class="h-56 w-full resize-y rounded border border-pc-border bg-pc-raised p-2 font-mono text-xs"
        aria-label={t('ocr.field')}
        spellcheck="false"
        oninput={() => {
          copied = false;
          copyFailed = false;
        }}></textarea>
      <p class="min-h-5 text-pc-muted" role="status" data-testid="ocr-copy-status">
        {copyFailed ? t('ocr.copyFailed') : copied ? t('ocr.copied') : ''}
      </p>
    {:else}
      <p class="min-h-5" role="status" data-testid="ocr-status">{status}</p>
    {/if}

    <div class="flex justify-end gap-2">
      {#if view.kind === 'work'}
        <Button onclick={close}>{t('common.cancel')}</Button>
      {:else if view.kind === 'error'}
        <Button onclick={close}>{t('common.close')}</Button>
        <Button variant="primary" onclick={start}>{t('ocr.retry')}</Button>
      {:else}
        <Button onclick={close}>{t('common.close')}</Button>
        {#if !view.empty}
          <Button variant="primary" disabled={!canCopy} onclick={() => void copy()}>
            {t('ocr.copy')}
          </Button>
        {/if}
      {/if}
    </div>
  </div>
</div>
