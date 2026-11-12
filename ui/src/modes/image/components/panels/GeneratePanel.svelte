<script lang="ts">
  // The Generate panel (docs/specs/comfyui-generate.md): prompt, model,
  // LoRA, strength, variants and seed; Generate / Cancel with the queue
  // position, progress and live preview; the variant strip with Apply /
  // Discard. Desktop only — the editor mounts it only there (AC-01).
  // Opening the panel needs a server (comfyui-connect AC-03).
  import Panel from '../../../../lib/components/layout/Panel.svelte';
  import { percent, t } from '../../../../lib/i18n/index.svelte';
  import { openImageGeneration } from '../../../../lib/comfyui/state.svelte';
  import {
    FAMILIES,
    FAMILY_NAMES,
    errorMessage,
    parseSeed,
    statusIssue,
    targetBox,
    type Family,
  } from '../../../../lib/comfyui/generate-params';
  import { editor, live, tabs } from '../../stores/editor.svelte';
  import {
    apply,
    cancel,
    currentFamily,
    discard,
    genInput,
    genServer,
    genSettings,
    genTabs,
    generate,
    hasCatalogFamily,
    refreshServer,
    saveGenSettings,
    selectVariant,
  } from '../../generate/generation.svelte';

  const tab = $derived(tabs.active);
  const gen = $derived(tab ? genTabs[tab.id] : undefined);
  const issue = $derived(statusIssue(genServer.status));
  const models = $derived(genServer.models);
  const family = $derived(currentFamily());
  const busy = $derived(gen !== undefined && gen.phase !== 'idle' && gen.phase !== 'done');
  const box = $derived(
    targetBox(editor.hasSelection ? live.selectionBox : null, editor.width, editor.height),
  );
  const seedOk = $derived(parseSeed(genInput.seed) !== undefined);
  const canGenerate = $derived(
    issue === null &&
      models !== null &&
      genSettings.model !== null &&
      genInput.prompt.trim() !== '' &&
      box !== null &&
      seedOk &&
      !busy &&
      tab !== null,
  );

  /** The panel body appears only while the panel is expanded. */
  function wake(): void {
    void refreshServer();
  }

  function setFamily(value: string): void {
    const model = genSettings.model;
    if (model === null) return;
    genSettings.families = { ...genSettings.families, [model]: value as Family };
    saveGenSettings();
  }

  function setNumber(key: 'loraStrength' | 'strength' | 'count', value: number, min: number, max: number): void {
    if (!Number.isFinite(value)) return;
    genSettings[key] = Math.min(max, Math.max(min, value));
    saveGenSettings();
  }

  function progressText(): string {
    if (!gen) return '';
    switch (gen.phase) {
      case 'starting':
        return t('gen.sending');
      case 'queued':
        return t('gen.queued', { ahead: gen.ahead });
      case 'running':
        return gen.total > 0 ? t('gen.step', { step: gen.step, total: gen.total }) : t('gen.running');
      case 'fetching':
        return t('gen.fetching');
      default:
        return '';
    }
  }
</script>

<Panel>
  <div {@attach wake} class="flex flex-col gap-2 text-xs text-pc-muted">
    {#if issue !== null}
      <div class="flex flex-col gap-1.5" data-testid="generate-status">
        <p role="status" class="text-pc-danger-text">{t(issue.key, issue.params)}</p>
        <button type="button" class="panel-btn min-h-8 self-start" onclick={openImageGeneration}
          >{t('gen.setUp')}</button
        >
      </div>
    {:else if models === null}
      <p role="status">{t('gen.checking')}</p>
    {/if}
    {#if genServer.gone !== null}
      <p role="note" class="text-pc-accent-text">{t('gen.noLonger', { name: genServer.gone })}</p>
    {/if}

    <label class="flex flex-col gap-1">
      <span>{t('gen.prompt')}</span>
      <textarea
        class="opt-field min-h-16 py-1"
        rows="3"
        aria-label={t('gen.prompt')}
        bind:value={genInput.prompt}
      ></textarea>
    </label>
    <label class="flex flex-col gap-1">
      <span>{t('gen.negative')}</span>
      <textarea
        class="opt-field min-h-10 py-1 disabled:opacity-50"
        rows="2"
        aria-label={t('gen.negative')}
        disabled={family === 'flux'}
        title={family === 'flux' ? t('gen.fluxNoNegative') : undefined}
        bind:value={genSettings.negative}
        onchange={saveGenSettings}
      ></textarea>
    </label>

    <label class="flex items-center gap-2">
      <span class="w-20 shrink-0">{t('gen.model')}</span>
      <select
        class="opt-field min-w-0 flex-1"
        value={genSettings.model ?? ''}
        aria-label={t('gen.model')}
        disabled={models === null}
        onchange={(e) => {
          genSettings.model = e.currentTarget.value;
          saveGenSettings();
        }}
      >
        {#each models?.checkpoints ?? [] as c (c.name)}
          <option value={c.name}>{c.name}</option>
        {/each}
      </select>
    </label>
    {#if models !== null && genSettings.model !== null && !hasCatalogFamily()}
      <label class="flex items-center gap-2">
        <span class="w-20 shrink-0">{t('gen.modelType')}</span>
        <select
          class="opt-field min-w-0 flex-1"
          value={family}
          aria-label={t('gen.modelType')}
          onchange={(e) => setFamily(e.currentTarget.value)}
        >
          {#each FAMILIES as f (f)}
            <option value={f}>{FAMILY_NAMES[f]}</option>
          {/each}
        </select>
      </label>
    {/if}
    <label class="flex items-center gap-2">
      <span class="w-20 shrink-0">{t('gen.lora')}</span>
      <select
        class="opt-field min-w-0 flex-1"
        value={genSettings.lora ?? ''}
        aria-label={t('gen.lora')}
        disabled={models === null}
        onchange={(e) => {
          genSettings.lora = e.currentTarget.value === '' ? null : e.currentTarget.value;
          saveGenSettings();
        }}
      >
        <option value="">{t('gen.noLora')}</option>
        {#each models?.loras ?? [] as name (name)}
          <option value={name}>{name}</option>
        {/each}
      </select>
    </label>
    <label class="flex items-center gap-2">
      <span class="w-20 shrink-0">{t('gen.loraStrength')}</span>
      <input
        type="range"
        min="0"
        max="2"
        step="0.05"
        class="min-w-0 flex-1"
        aria-label={t('gen.loraStrength')}
        disabled={genSettings.lora === null}
        value={genSettings.loraStrength}
        onchange={(e) => setNumber('loraStrength', Number(e.currentTarget.value), 0, 2)}
      />
      <span class="w-9 text-right tabular-nums">{genSettings.loraStrength.toFixed(2)}</span>
    </label>
    <label class="flex items-center gap-2">
      <span class="w-20 shrink-0">{t('gen.strength')}</span>
      <input
        type="range"
        min="1"
        max="100"
        class="min-w-0 flex-1"
        aria-label={t('gen.strength')}
        value={genSettings.strength}
        oninput={(e) => (genSettings.strength = Number(e.currentTarget.value))}
        onchange={(e) => setNumber('strength', Number(e.currentTarget.value), 1, 100)}
      />
      <span class="w-9 text-right tabular-nums">{percent(genSettings.strength)}</span>
    </label>
    <label class="flex items-center gap-2">
      <span class="w-20 shrink-0">{t('gen.variants')}</span>
      <input
        type="number"
        min="1"
        max="8"
        class="opt-field w-16"
        aria-label={t('gen.variants')}
        value={genSettings.count}
        onchange={(e) => setNumber('count', Math.round(Number(e.currentTarget.value)), 1, 8)}
      />
    </label>
    <label class="flex items-center gap-2">
      <span class="w-20 shrink-0">{t('gen.seed')}</span>
      <input
        type="text"
        inputmode="numeric"
        class="opt-field min-w-0 flex-1"
        class:border-pc-danger={!seedOk}
        placeholder={t('gen.seedRandom')}
        aria-label={t('gen.seed')}
        bind:value={genInput.seed}
      />
    </label>

    {#if busy && gen}
      <div class="flex flex-col gap-1.5">
        <p role="status" class="text-pc-text" data-testid="generate-progress">{progressText()}</p>
        {#if gen.live}
          <img src={gen.live} alt={t('gen.livePreview')} class="max-h-40 self-start rounded border border-pc-border" />
        {/if}
        <button type="button" class="panel-btn min-h-8" onclick={() => tab && cancel(tab.id)}>{t('gen.cancel')}</button>
      </div>
    {:else}
      <button
        type="button"
        class="panel-btn panel-btn-primary min-h-8"
        disabled={!canGenerate}
        onclick={() => tab && void generate(tab)}>{t('gen.generate')}</button
      >
    {/if}

    {#if gen?.error}
      {@const message = errorMessage(gen.error)}
      <p role="alert" class="text-pc-danger-text">{t(message.key, message.params)}</p>
    {/if}

    {#if gen && gen.phase === 'done' && gen.variants.length > 0}
      <div class="flex flex-wrap gap-1" role="listbox" aria-label={t('gen.strip')}>
        {#each gen.thumbs as src, i (src)}
          <button
            type="button"
            role="option"
            aria-selected={gen.selected === i}
            aria-label={t('gen.variant', { n: i + 1 })}
            class="h-12 w-12 overflow-hidden rounded border-2"
            class:border-pc-accent={gen.selected === i}
            class:border-pc-border={gen.selected !== i}
            onclick={() => tab && selectVariant(tab.id, i)}
          >
            <img {src} alt="" class="h-full w-full object-cover" />
          </button>
        {/each}
      </div>
      <div class="flex gap-2">
        <button type="button" class="panel-btn panel-btn-primary min-h-8 flex-1" onclick={() => tab && apply(tab)}
          >{t('gen.apply')}</button
        >
        <button type="button" class="panel-btn min-h-8 flex-1" onclick={() => tab && discard(tab.id)}
          >{t('gen.discard')}</button
        >
      </div>
    {/if}
  </div>
</Panel>
