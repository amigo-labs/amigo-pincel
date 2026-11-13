// The desktop host tools `gen.image` and `gen.models`
// (docs/specs/comfyui-generate.md AC-16 – AC-20): the MCP endpoint forwards
// them like any call; they ask the ComfyUI backend for the models, check
// the arguments before anything is sent to the server, and let the editor
// of the targeted tab generate and apply one variant.
import { activateTab, session } from '../shell/session.svelte';
import { getPref } from '../idb/prefs';
import { generationModels } from '../comfyui/generate-bridge';
import {
  GEN_PREF,
  asGenError,
  checkGenImage,
  errorEnglish,
  type Models,
} from '../comfyui/generate-params';
import { targetTab } from './tab-calls';
import { fail, ok, type Wire } from './wire';

/** Whether `tool` is a generation host tool. */
export function isGenTool(tool: string): boolean {
  return tool === 'gen.image' || tool === 'gen.models';
}

/** The server's models, or the reason no server is usable (AC-19). */
async function models(): Promise<Models | string> {
  try {
    return await generationModels();
  } catch (err) {
    return typeof err === 'string' ? err : err instanceof Error ? err.message : String(err);
  }
}

/** The Generate panel's model, the default of `gen.image` (AC-16). */
async function panelModel(): Promise<string | null> {
  try {
    const stored = (await getPref(GEN_PREF)) as { model?: unknown } | undefined;
    return typeof stored?.model === 'string' ? stored.model : null;
  } catch {
    return null;
  }
}

/** Runs a `gen.*` call; `signal` aborts when the call times out (AC-20). */
export async function genTool(tool: string, args: Record<string, unknown>, signal: AbortSignal): Promise<Wire> {
  const available = await models();
  if (typeof available === 'string') return fail(available);
  if (tool === 'gen.models') {
    return ok({
      checkpoints: available.checkpoints.map((c) => c.name),
      loras: available.loras,
    });
  }
  const checked = checkGenImage(args, available, await panelModel());
  if (typeof checked === 'string') return fail(checked);
  const tab = targetTab(args);
  if (typeof tab === 'string') return fail(tab);
  // The user sees the change land (mcp-live-desktop, Decisions).
  if (session.active !== tab) activateTab(tab);
  try {
    const made = await tab.doc.generateImage(checked, available, signal);
    return ok({ layer: made.layer, seed: made.seed, model: made.model });
  } catch (err) {
    return fail(err instanceof Error ? err.message : errorEnglish(asGenError(err)));
  }
}
