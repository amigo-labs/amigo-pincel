// The render backend of the shown Pixel editor, for the About dialog
// (docs/specs/frame-layout.md AC-17). `null` on the start screen, in
// Image mode and before the renderer has chosen a backend.
export const renderInfo = $state<{ backend: string | null }>({ backend: null });
