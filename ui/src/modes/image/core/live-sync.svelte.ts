// Toolbar and color changes reach the live object
// (docs/specs/image-live-objects-ui.md AC-06, AC-07). Only a change is sent:
// an object that becomes live through Undo / Redo keeps its parameters, and
// the toolbar keeps the user's values. Consecutive changes of one option
// merge in the core (image-live-objects AC-03).
import { untrack } from 'svelte';
import { live, tool } from '../stores/editor.svelte';
import type { LiveObjectSpec } from './wasm';
import { activeColor, colorWithOpacity, opacity01 } from './controller';
import { editLive } from './live';
import { endBurst, requestTextFocus } from './live-text.svelte';

/** Runs `apply` with the live object whenever `read` changes value. */
function onChange<T>(read: () => T, apply: (obj: LiveObjectSpec, value: T) => void): void {
  let prev = untrack(read);
  $effect(() => {
    const value = read();
    if (value === prev) return;
    prev = value;
    untrack(() => {
      if (live.object !== null) apply(live.object, value);
    });
  });
}

/** Installs the forwarding; call once while the editor component mounts. */
export function installLiveSync(): void {
  const shape = (fields: Parameters<typeof editLive>[0]) => (obj: LiveObjectSpec) => {
    if (obj.type === 'shape') editLive(fields);
  };
  onChange(() => tool.shapeMode, (obj, mode) => shape({ mode })(obj));
  onChange(() => tool.strokeWidth, (obj, w) => shape({ stroke_width: w })(obj));
  onChange(() => tool.shapeDash, (obj, dash) => shape({ dash })(obj));
  onChange(() => tool.antiAlias, (obj, aa) => shape({ anti_alias: aa })(obj));
  onChange(() => tool.cornerRadius, (obj, r) => shape({ corner_radius: r })(obj));
  onChange(() => tool.shapeSides, (obj, n) => shape({ sides: n })(obj));
  // Outline = foreground, fill = background, both at the tool opacity: one
  // colors group, so a swap or an opacity drag is one step.
  onChange(
    () =>
      `${tool.foreground}|${tool.foregroundAlpha}|${tool.background}|${tool.backgroundAlpha}|${tool.opacity}`,
    (obj) =>
      shape({
        stroke_color: colorWithOpacity(tool.foreground, tool.foregroundAlpha),
        fill_color: colorWithOpacity(tool.background, tool.backgroundAlpha),
      })(obj),
  );
  const gradient = (obj: LiveObjectSpec, fields: Parameters<typeof editLive>[0]) => {
    if (obj.type === 'gradient') editLive(fields);
  };
  onChange(() => tool.gradientKind, (obj, kind) => gradient(obj, { kind }));
  onChange(() => tool.gradientChannel, (obj, channel) => gradient(obj, { channel }));
  onChange(() => tool.opacity, (obj) => gradient(obj, { opacity: opacity01() }));
  // A Transparency gradient ignores the colors: no step (AC-07).
  onChange(
    () => `${tool.foreground}|${tool.foregroundAlpha}|${tool.background}|${tool.backgroundAlpha}`,
    (obj) => {
      if (obj.type === 'gradient' && obj.channel !== 'transparency') {
        editLive({ from: activeColor(false), to: activeColor(true) });
      }
    },
  );
  // The live text: each option is one group; the edit ends a typing burst
  // and focus goes back to the text (docs/specs/image-live-text.md AC-08).
  const text = (obj: LiveObjectSpec, fields: Parameters<typeof editLive>[0]) => {
    if (obj.type !== 'text') return;
    endBurst();
    editLive(fields);
    requestTextFocus();
  };
  onChange(() => tool.fontSize, (obj, size) => text(obj, { size }));
  onChange(() => tool.textBold, (obj, bold) => text(obj, { bold }));
  onChange(() => tool.textItalic, (obj, italic) => text(obj, { italic }));
  onChange(() => tool.textUnderline, (obj, underline) => text(obj, { underline }));
  onChange(() => tool.textStrikethrough, (obj, strikethrough) => text(obj, { strikethrough }));
  onChange(() => tool.textAlign, (obj, align) => text(obj, { align }));
  onChange(() => tool.antiAlias, (obj, aa) => text(obj, { anti_alias: aa }));
  onChange(
    () => `${tool.foreground}|${tool.foregroundAlpha}`,
    (obj) => text(obj, { color: colorWithOpacity(tool.foreground, tool.foregroundAlpha) }),
  );
}
