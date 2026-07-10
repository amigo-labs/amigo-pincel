/// <reference types="svelte" />
/// <reference types="vite/client" />

interface ImportMetaEnv {
  /**
   * Release version stamped in by the release workflow
   * (`VITE_APP_VERSION`, e.g. `v0.1.0`). Unset in dev builds, where the
   * footer falls back to `dev`.
   */
  readonly VITE_APP_VERSION?: string;
}
