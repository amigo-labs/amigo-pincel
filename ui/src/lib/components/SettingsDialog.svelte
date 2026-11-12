<script lang="ts">
  // Edit ▸ Settings… (docs/specs/settings-dialog.md): the app settings that
  // used to sit in View. Each control writes through the setter its former
  // menu entry used, so effect and persistence are unchanged (AC-04).
  import { onMount } from 'svelte';
  import { LOCALES, isLocale, locale, t } from '../i18n/index.svelte';
  import { setLocale } from '../i18n/locale';
  import { motion, setReduceMotion } from '../motion.svelte';
  import { setTouchGestures, touchGestures } from '../shell/touch-gestures.svelte';

  let { onClose }: { onClose: () => void } = $props();

  let languageField: HTMLSelectElement | undefined = $state();

  // Focus moves into the dialog and back to where it was when it closes
  // (AC-06); keys stay here, so a tool key or Ctrl+Z never reaches the
  // editor behind it (the editors also gate on `help.settings`).
  onMount(() => {
    const returnTo = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    languageField?.focus();
    return () => returnTo?.focus();
  });

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="settings-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div class="w-full max-w-sm rounded border border-pc-border bg-pc-panel p-6 shadow-xl">
    <h2 id="settings-title" class="mb-4 text-lg font-semibold">{t('settings.title')}</h2>
    <div class="flex flex-col gap-4 text-sm">
      <label class="flex items-center justify-between gap-3">
        <span>{t('settings.language')}</span>
        <select
          bind:this={languageField}
          class="panel-btn leading-4 pointer-coarse:min-h-11"
          value={locale()}
          onchange={(e) => {
            const v = e.currentTarget.value;
            if (isLocale(v)) setLocale(v);
          }}
        >
          {#each LOCALES as l (l.id)}
            <option value={l.id}>{l.label}</option>
          {/each}
        </select>
      </label>
      <div class="flex flex-col gap-1">
        <label class="flex items-center gap-2 pointer-coarse:min-h-11">
          <input
            type="checkbox"
            checked={motion.option}
            aria-describedby={motion.system ? 'settings-motion-system' : undefined}
            onchange={(e) => setReduceMotion(e.currentTarget.checked)}
          />
          {t('settings.reduceMotion')}
        </label>
        {#if motion.system}
          <p id="settings-motion-system" class="ml-6 text-xs text-pc-muted">
            {t('settings.reduceMotionSystem')}
          </p>
        {/if}
      </div>
      <label class="flex items-center gap-2 pointer-coarse:min-h-11">
        <input
          type="checkbox"
          checked={touchGestures.enabled}
          onchange={(e) => setTouchGestures(e.currentTarget.checked)}
        />
        {t('settings.touchGestures')}
      </label>
    </div>
    <div class="mt-6 flex justify-end">
      <button class="panel-btn" onclick={onClose}>{t('common.close')}</button>
    </div>
  </div>
</div>
