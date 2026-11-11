<script lang="ts">
  // Foreground / background colors (spec §16.6, §4.2), on the shared
  // ColorPair control, with the recent colors and Image mode's palette
  // always in view below them (color-picker AC-25 – AC-29). The palette is
  // app state shared by every Image tab: the header's ⋯ menu loads a
  // palette file, saves it in one of Pixel mode's export formats or resets
  // it to the default, `+` appends the foreground color
  // (docs/specs/image-palette-gradient.md).
  import ColorPair from '../../../../lib/components/ColorPair.svelte';
  import SwatchGrid from '../../../../lib/components/color/SwatchGrid.svelte';
  import IconButton from '../../../../lib/components/layout/IconButton.svelte';
  import Panel from '../../../../lib/components/layout/Panel.svelte';
  import { hexToRgb, toHex, type Rgba8 } from '../../../../lib/color';
  import { colorDraft, recentColors } from '../../../../lib/colors/recent.svelte';
  import { pickAndOpen, saveExport } from '../../../../lib/fs';
  import { t } from '../../../../lib/i18n/index.svelte';
  import { core, type PaletteFileFormat } from '../../core/wasm';
  import { addColor, packRgba, shownName, unpackRgba, type PaletteEntry } from '../../palette';
  import { editor, tool, swapColors, resetColors } from '../../stores/editor.svelte';
  import { paletteEntries, setImagePalette } from '../../stores/palette.svelte';

  function setFg(c: Rgba8): void {
    tool.foreground = toHex(c);
    tool.foregroundAlpha = c.a;
  }

  function setBg(c: Rgba8): void {
    tool.background = toHex(c);
    tool.backgroundAlpha = c.a;
  }

  const pick = (c: Rgba8, target: 'fg' | 'bg'): void => (target === 'fg' ? setFg(c) : setBg(c));

  const entries = $derived(paletteEntries());
  const colors = $derived(entries.map((e) => e.color));
  const names = $derived(entries.map(shownName));

  // The Save palette submenu: Pixel mode's export entries (AC-06).
  const SAVES = [
    { id: 'gpl', format: 'gpl', ext: 'gpl', cell: 1, label: 'palette.exportGpl' },
    { id: 'hex', format: 'hex', ext: 'hex', cell: 1, label: 'palette.exportHex' },
    { id: 'pal', format: 'pal', ext: 'pal', cell: 1, label: 'palette.exportPal' },
    { id: 'png1', format: 'png', ext: 'png', cell: 1, label: 'palette.exportPng1' },
    { id: 'png8', format: 'png', ext: 'png', cell: 8, label: 'palette.exportPng8' },
  ] as const satisfies readonly {
    id: string;
    format: PaletteFileFormat;
    ext: string;
    cell: number;
    label: string;
  }[];
  const SAVE_FORMAT = { gpl: 'gpl', hex: 'hex', pal: 'pal', png: 'palette-png' } as const;

  let menu = $state<{ left: number; top: number } | null>(null);
  let saveOpen = $state(false);

  function openMenu(e: MouseEvent): void {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    saveOpen = false;
    menu = {
      left: Math.max(4, Math.min(r.right - 208, window.innerWidth - 212)),
      top: r.bottom + 4,
    };
  }

  const failed = (action: string, err: unknown): string =>
    t('status.failed', { action, error: err instanceof Error ? err.message : String(err) });

  /** Shows `next` and stores it; a failed store keeps it for the session
   *  and says so (AC-25), otherwise `done` is the notice. */
  async function change(next: PaletteEntry[] | null, done: string | null): Promise<void> {
    if (done !== null) editor.notice = done;
    if (!(await setImagePalette(next))) editor.notice = t('palette.storeFailed');
  }

  const toEntry = (rgba: number, name?: string): PaletteEntry =>
    name ? { color: unpackRgba(rgba), name } : { color: unpackRgba(rgba) };

  async function loadPalette(): Promise<void> {
    menu = null;
    let next: PaletteEntry[];
    try {
      const opened = await pickAndOpen('palette');
      if (!opened) return;
      next = core.readPaletteFile(opened.bytes).map((e) => toEntry(e.rgba, e.name));
    } catch (err) {
      editor.notice = failed(t('palette.load'), err);
      return;
    }
    await change(next, t('palette.loaded', { count: next.length }));
  }

  async function savePalette(entry: (typeof SAVES)[number]): Promise<void> {
    menu = null;
    const list = entries.map((e) =>
      e.name ? { rgba: packRgba(e.color), name: e.name } : { rgba: packRgba(e.color) },
    );
    try {
      const bytes = core.writePaletteFile(list, entry.format, 'palette', entry.cell);
      if (await saveExport(bytes, `palette.${entry.ext}`, SAVE_FORMAT[entry.format])) {
        editor.notice = t('palette.saved', { count: list.length });
      }
    } catch (err) {
      editor.notice = failed(t('palette.save'), err);
    }
  }

  function resetPalette(): void {
    menu = null;
    void change(null, null);
  }

  function addForeground(): void {
    const fg = { ...hexToRgb(tool.foreground), a: tool.foregroundAlpha };
    const next = addColor(entries, fg);
    if (next === 'duplicate') editor.notice = t('palette.duplicate');
    else if (next === 'full') editor.notice = t('palette.full');
    else void change(next, null);
  }
</script>

<Panel>
  {#snippet actions()}
    <IconButton icon="plus" label={t('palette.addColor')} size="sm" onclick={addForeground} />
    <IconButton icon="more" label={t('palette.more')} size="sm" onclick={openMenu} />
  {/snippet}

  {#if menu}
    <!-- Backdrop closes the menu on an outside click. -->
    <button
      type="button"
      class="fixed inset-0 z-40 cursor-default"
      tabindex="-1"
      aria-label={t('frame.closeMenu')}
      onclick={() => (menu = null)}
    ></button>
    <div
      class="pal-menu fixed z-50 w-52 shadow-lg"
      style:left="{menu.left}px"
      style:top="{menu.top}px"
      role="menu"
      aria-label={t('palette.more')}
      tabindex="-1"
      onkeydown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          menu = null;
        }
      }}
    >
      <button type="button" class="pal-menu-row" role="menuitem" onclick={() => void loadPalette()}>
        {t('palette.loadAction')}
      </button>
      <button
        type="button"
        class="pal-menu-row flex items-center justify-between"
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={saveOpen}
        onclick={() => (saveOpen = !saveOpen)}
      >
        <span>{t('palette.save')}</span>
        <span aria-hidden="true">{saveOpen ? '▾' : '▸'}</span>
      </button>
      {#if saveOpen}
        <div role="menu" aria-label={t('palette.save')} tabindex="-1">
          {#each SAVES as entry (entry.id)}
            <button
              type="button"
              class="pal-menu-row pl-6"
              role="menuitem"
              onclick={() => void savePalette(entry)}
            >
              {t(entry.label)}
            </button>
          {/each}
        </div>
      {/if}
      <div class="my-1 border-t border-pc-border" role="separator"></div>
      <button type="button" class="pal-menu-row" role="menuitem" onclick={resetPalette}>
        {t('palette.reset')}
      </button>
    </div>
  {/if}

  <div class="flex flex-col gap-3">
    <ColorPair
      fg={{ ...hexToRgb(tool.foreground), a: tool.foregroundAlpha }}
      bg={{ ...hexToRgb(tool.background), a: tool.backgroundAlpha }}
      onFg={setFg}
      onBg={setBg}
      onSwap={swapColors}
      onReset={resetColors}
      recent={recentColors.image}
      onDraft={(d) => (colorDraft.image = d)}
    />
    <SwatchGrid label={t('color.recent')} colors={recentColors.image} onPick={pick} />
    <SwatchGrid label={t('color.palette')} {colors} {names} maxRows={8} onPick={pick} />
  </div>
</Panel>

<style>
  .pal-menu {
    border-radius: 0.375rem;
    border: 1px solid var(--color-pc-border);
    background-color: var(--color-pc-panel);
    padding: 0.25rem 0;
    font-size: 0.875rem;
  }
  .pal-menu-row {
    display: block;
    width: 100%;
    padding: 0.3rem 0.75rem;
    text-align: left;
  }
  .pal-menu-row:hover,
  .pal-menu-row:focus-visible {
    background-color: var(--color-pc-hover);
    outline: none;
  }
</style>
