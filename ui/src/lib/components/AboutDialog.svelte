<script lang="ts">
  // Help ▸ About Pincel, the start screen's button and, on macOS, the
  // native menu's About (docs/specs/desktop-app.md AC-17). In a desktop
  // copy that can update itself it also holds the update part: the
  // downloaded version, the update choice, Check for Updates, Install and
  // Restart Now (AC-28, AC-31, AC-32).
  import { onMount } from 'svelte';
  import { chooseMode, help, runUpdateCheck } from '../help/state.svelte';
  import type { UpdateMode } from '../help/updater';
  import { t } from '../i18n/index.svelte';
  import { requestRestart } from '../shell/close-guard';

  let { onClose }: { onClose: () => void } = $props();

  let closeButton: HTMLButtonElement | undefined = $state();

  // `aria-modal` suspends no keyboard handler: without taking the focus and
  // stopping the keys here, Delete or Ctrl+Z would reach the editor behind
  // the dialog and edit the document (the editors also gate on `help.about`).
  onMount(() => closeButton?.focus());

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  // Windows installs a downloaded version when Pincel exits rather than
  // replacing the app at once (AC-28).
  const windows = typeof navigator !== 'undefined' && /Win/i.test(navigator.userAgent);

  function readyText(version: string): string {
    return t(windows ? 'about.readyOnExit' : 'about.ready', { version });
  }

  const updater = $derived(help.updater);
  const supported = $derived(updater !== null && updater.supported);

  const status = $derived.by(() => {
    if (help.checking) return t('about.checking');
    const r = help.result;
    if (r === null) return updater?.ready ? readyText(updater.ready) : null;
    if (r.kind === 'upToDate') return t('about.upToDate');
    if (r.kind === 'ready') return readyText(r.version);
    if (r.kind === 'available') return t('about.available', { version: r.version });
    return t('about.failed', { error: r.error });
  });

  const available = $derived(
    !help.checking && help.result?.kind === 'available' ? help.result.version : null,
  );
  const canRestart = $derived(supported && !help.checking && (updater?.ready ?? null) !== null);

  const reason = $derived.by(() => {
    if (updater === null || updater.supported) return null;
    if (updater.unsupported === 'package') return t('about.package');
    if (updater.unsupported === 'appimage') return t('about.appimage');
    return null;
  });

  const MODES: readonly UpdateMode[] = ['auto', 'notify', 'off'];
  const modeLabel = (mode: UpdateMode): string => t(`update.${mode}`);

  // Release version stamped in by the release workflow; `dev` locally.
  const version = import.meta.env.VITE_APP_VERSION ?? 'dev';
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="about-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div class="w-full max-w-sm rounded border border-pc-border bg-pc-panel p-6 shadow-xl">
    <h2 id="about-title" class="mb-1 text-lg font-semibold">{t('menu.about')}</h2>
    <p class="mb-3 text-sm text-pc-muted">{t('about.version', { version })}</p>
    <p class="mb-4 text-sm text-pc-text">{t('about.tagline')}</p>
    {#if supported && updater !== null}
      <fieldset class="mb-4" role="radiogroup" aria-labelledby="about-updates">
        <legend id="about-updates" class="mb-1 text-sm font-semibold">{t('about.updates')}</legend>
        {#each MODES as mode (mode)}
          <label class="flex items-center gap-2 text-sm text-pc-text">
            <input
              type="radio"
              name="about-update-mode"
              value={mode}
              checked={updater.mode === mode}
              onchange={() => void chooseMode(mode)}
            />
            {modeLabel(mode)}
          </label>
        {/each}
        {#if help.notSaved}
          <p class="mt-1 text-xs text-pc-muted">{t('about.notSaved')}</p>
        {/if}
      </fieldset>
    {:else if reason !== null}
      <p class="mb-4 text-sm text-pc-muted">{reason}</p>
    {/if}
    {#if status !== null}
      <p class="mb-4 text-sm text-pc-accent" role="status">{status}</p>
    {/if}
    {#if help.restartError !== null}
      <p class="mb-4 text-sm text-pc-accent" role="alert">
        {t('about.restartFailed', { error: help.restartError })}
      </p>
    {/if}
    <div class="flex flex-wrap justify-end gap-2">
      {#if supported}
        {#if available !== null}
          <button class="panel-btn" onclick={runUpdateCheck}>{t('about.install')}</button>
        {/if}
        {#if canRestart}
          <button class="panel-btn" onclick={() => void requestRestart()}>
            {t('about.restart')}
          </button>
        {/if}
        <button class="panel-btn" disabled={help.checking} onclick={runUpdateCheck}>
          {t('about.check')}
        </button>
      {/if}
      <button bind:this={closeButton} class="panel-btn" onclick={onClose}>
        {t('about.close')}
      </button>
    </div>
  </div>
</div>
