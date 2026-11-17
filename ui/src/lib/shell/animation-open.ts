// Which files ask "animation or first frame?" before they open
// (docs/specs/animated-image-import.md AC-01, AC-02, AC-08, AC-13). A
// cheap header look (`mayBeAnimated`) filters the static files; the rest
// go to the Pixel core's probe, which decodes the first frame, so its
// wasm module is fetched only for files that announce several frames.
import { mayBeAnimated } from '../fs/animation-hint';
import { formatFromName, sniffFormat, type OpenedFile } from '../fs';

/** What the dialog shows about an animated file. */
export interface AnimatedFile {
  width: number;
  height: number;
  /** Frames the file announces (at least 2). */
  frames: number;
  /** `frames × width × height`. */
  totalPixels: number;
  /** The limit that keeps it from opening as an animation (AC-08). */
  exceeded: 'side' | 'pixels' | null;
  maxSide: number;
  maxPixels: number;
}

/** The user's answer in the dialog. */
export type AnimationChoice = 'animation' | 'first' | 'cancel';

/**
 * The animation `opened` holds, or `null` for a file that opens without
 * the dialog: a static image, another format, or a file whose first frame
 * cannot be decoded — that one goes the usual open path, which fails as
 * for any unreadable image (AC-13).
 */
export async function probeAnimatedFile(opened: OpenedFile): Promise<AnimatedFile | null> {
  const format = sniffFormat(opened.bytes) ?? formatFromName(opened.name);
  if (!mayBeAnimated(opened.bytes, format)) return null;
  const core = await import('../core');
  await core.loadCore();
  let probe;
  try {
    probe = core.probeAnimation(opened.bytes);
  } catch {
    return null;
  }
  if (!probe) return null;
  try {
    const exceeded = probe.exceeded;
    return {
      width: probe.width,
      height: probe.height,
      frames: probe.frameCount,
      totalPixels: probe.totalPixels,
      exceeded: exceeded === 'side' || exceeded === 'pixels' ? exceeded : null,
      maxSide: probe.maxSide,
      maxPixels: probe.maxPixels,
    };
  } finally {
    probe.free();
  }
}

/** The name of the Pixel document an animation opens as: the extension
 *  replaced by `.aseprite` (`walk.gif` → `walk.aseprite`, AC-10). */
export function animationDocName(name: string): string {
  const dot = name.lastIndexOf('.');
  return `${dot > 0 ? name.slice(0, dot) : name}.aseprite`;
}
