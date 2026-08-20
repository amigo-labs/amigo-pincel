<script lang="ts">
  // First-launch advisory dialog for `.aseprite` file associations.
  // On Tauri the install bundle registers the type at the OS level —
  // on macOS LaunchServices, on Windows the installer, on Linux the
  // `.desktop` file. We can't programmatically force Pincel to be the
  // default handler without elevation, so this dialog walks the user
  // through the per-OS steps once. "Don't show again" sets a pref so
  // the dialog never re-appears.
  let {
    platform,
    onDismiss,
  }: {
    platform: 'macos' | 'windows' | 'linux' | 'unknown';
    onDismiss: (dontShowAgain: boolean) => void;
  } = $props();

  let dontShowAgain = $state(false);
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/70 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="fileassoc-title"
>
  <div class="w-full max-w-lg rounded border border-neutral-700 bg-neutral-900 p-6 shadow-xl">
    <h2 id="fileassoc-title" class="mb-3 text-lg font-semibold">
      Open .aseprite files with Pincel
    </h2>
    <p class="mb-3 text-sm text-neutral-300">
      Pincel can edit Aseprite-format sprites. To make it the default
      handler for <code class="text-neutral-100">.aseprite</code> and
      <code class="text-neutral-100">.ase</code> files on your system:
    </p>
    {#if platform === 'macos'}
      <ol class="mb-4 list-decimal space-y-1 pl-5 text-sm text-neutral-300">
        <li>Right-click an .aseprite file in Finder → Get Info.</li>
        <li>Under "Open With", pick Pincel.</li>
        <li>Click "Change All…" to apply to every .aseprite file.</li>
      </ol>
    {:else if platform === 'windows'}
      <ol class="mb-4 list-decimal space-y-1 pl-5 text-sm text-neutral-300">
        <li>Open Settings → Apps → Default apps.</li>
        <li>Search for ".aseprite" and pick Pincel from the list.</li>
        <li>Repeat for ".ase".</li>
      </ol>
    {:else if platform === 'linux'}
      <ol class="mb-4 list-decimal space-y-1 pl-5 text-sm text-neutral-300">
        <li>
          Run <code class="text-neutral-100"
            >xdg-mime default pincel.desktop application/x-aseprite</code
          >.
        </li>
        <li>
          If your distro doesn't pick up new MIME types automatically,
          run <code class="text-neutral-100">update-desktop-database</code>
          afterward.
        </li>
      </ol>
    {:else}
      <p class="mb-4 text-sm text-neutral-400">
        Steps depend on your OS — consult its documentation for setting
        default file handlers.
      </p>
    {/if}
    <label class="mb-4 flex items-center gap-2 text-sm text-neutral-300">
      <input type="checkbox" bind:checked={dontShowAgain} />
      Don't show this again
    </label>
    <div class="flex justify-end">
      <button class="toolbar-btn" onclick={() => onDismiss(dontShowAgain)}>
        Got it
      </button>
    </div>
  </div>
</div>
