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
import { ensureFace, imageFont, trackFontEdit } from './text-font';

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
  // A live star's options (docs/specs/image-shapes-curves-shapes.md AC-20).
  onChange(() => tool.starPoints, (obj, n) => shape({ star_points: n })(obj));
  onChange(() => tool.starInner, (obj, r) => shape({ inner_ratio: r / 100 })(obj));
  // A live line's caps (docs/specs/image-shapes-curves.md AC-12).
  const line = (fields: Parameters<typeof editLive>[0]) => (obj: LiveObjectSpec) => {
    if (obj.type === 'shape' && obj.shape === 'line') editLive(fields);
  };
  onChange(() => tool.lineStartCap, (obj, cap) => line({ start_cap: cap })(obj));
  onChange(() => tool.lineEndCap, (obj, cap) => line({ end_cap: cap })(obj));
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
  // Family, Bold and Italic choose the face; the live text switches to it
  // once it is read, and only what the face lacks is synthesized
  // (docs/specs/system-fonts.md AC-07, AC-09). The toggled option is always
  // part of its edit, so each option stays its own undo group.
  let fontSeq = 0;
  const font = (field: 'bold' | 'italic' | null) => (obj: LiveObjectSpec) => {
    if (obj.type !== 'text') return;
    fontSeq += 1;
    const seq = fontSeq;
    const edit = ensureFace().then((face) => {
      const cur = live.object;
      if (seq !== fontSeq || cur?.type !== 'text') return;
      const fields: Parameters<typeof editLive>[0] = {};
      if (face.id !== cur.font_id) fields.font_id = face.id;
      if (field === 'bold' || face.synthBold !== cur.bold) fields.bold = face.synthBold;
      if (field === 'italic' || face.synthItalic !== cur.italic) fields.italic = face.synthItalic;
      if (Object.keys(fields).length > 0) text(cur, fields);
    });
    trackFontEdit(edit.catch(() => undefined));
  };
  onChange(() => tool.textBold, (obj) => font('bold')(obj));
  onChange(() => tool.textItalic, (obj) => font('italic')(obj));
  onChange(() => imageFont.state.family, (obj) => font(null)(obj));
  onChange(() => tool.textUnderline, (obj, underline) => text(obj, { underline }));
  onChange(() => tool.textStrikethrough, (obj, strikethrough) => text(obj, { strikethrough }));
  onChange(() => tool.textAlign, (obj, align) => text(obj, { align }));
  onChange(() => tool.antiAlias, (obj, aa) => text(obj, { anti_alias: aa }));
  onChange(
    () => `${tool.foreground}|${tool.foregroundAlpha}`,
    (obj) => text(obj, { color: colorWithOpacity(tool.foreground, tool.foregroundAlpha) }),
  );
  // One Blending setting for every live object
  // (docs/specs/image-edit-extras-tool-options.md AC-08).
  onChange(
    () => tool.blend,
    (obj, blend) => {
      if (obj.type === 'text') text(obj, { blend });
      else editLive({ blend });
    },
  );
}
