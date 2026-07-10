// Packed-color helpers shared by the toolbar color input (App.svelte),
// the Slices panel, and the Tile Editor. The wasm surface speaks packed
// `0xRRGGBBAA` (red in the high byte, alpha in the low byte); DOM
// `<input type="color">` speaks `#RRGGBB`.

/** `#RRGGBB` → packed `0xRRGGBBAA`. Alpha defaults to fully opaque. */
export function packColor(hex: string, alpha = 0xff): number {
  const rgb = Number.parseInt(hex.slice(1), 16);
  return ((rgb << 8) | (alpha & 0xff)) >>> 0;
}

/**
 * Packed `0xRRGGBBAA` → `#RRGGBB`. Alpha is intentionally dropped — the
 * color input has no alpha control; callers that need it can read the
 * raw u32 themselves.
 */
export function unpackColor(rgba: number): string {
  const rgb = (rgba >>> 8) & 0xffffff;
  return '#' + rgb.toString(16).padStart(6, '0');
}
