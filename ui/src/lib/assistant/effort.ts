// The effort field's rules (docs/specs/ai-chat-clis.md AC-10, AC-15);
// mirrors src-tauri/src/assistant/effort.rs. Rune-free, so a Node-side
// spec can import it.

/** The longest effort text a send may carry (AC-15). */
export const EFFORT_MAX_LEN = 32;

const VARIANT_NAME = /^[A-Za-z0-9_-]+$/;

/** `true` when `text`, trimmed, may be sent: empty (the CLI's default) or
 *  1 to 32 letters, digits, `-` or `_` (AC-15). */
export function effortValid(text: string): boolean {
  const effort = text.trim();
  return effort === '' || (effort.length <= EFFORT_MAX_LEN && VARIANT_NAME.test(effort));
}

/** The effort a send passes: `null` for the CLI's default. */
export function effortOfSend(text: string): string | null {
  const effort = text.trim();
  return effort === '' ? null : effort;
}
