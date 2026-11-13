// Renderer side of image generation (src-tauri/src/comfyui/generate.rs,
// docs/specs/comfyui-generate.md): the backend runs the job against
// ComfyUI and reports it as `comfyui:generation` events; the webview reads
// the input and mask from its core and fetches the finished variants.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Family, GenError, Models } from './generate-params';

/** `GenerateRequest` in src-tauri/src/comfyui/generate.rs. */
export interface GenerateRequest {
  model: string;
  family: Family;
  lora: string | null;
  loraStrength: number;
  prompt: string;
  negative: string;
  seed: number | null;
  count: number;
  /** 0.01–1.0. */
  strength: number;
  width: number;
  height: number;
  /** PNG in base64. */
  input: string | null;
  mask: string | null;
}

/** `Started` in src-tauri/src/comfyui/generate.rs. */
export interface Started {
  job: number;
  seed: number;
}

/** `GenEvent` in src-tauri/src/comfyui/generate.rs. */
export type GenEvent =
  | { kind: 'queued'; job: number; ahead: number }
  | { kind: 'running'; job: number }
  | { kind: 'progress'; job: number; step: number; total: number }
  | { kind: 'preview'; job: number; image: string }
  | { kind: 'finished'; job: number; count: number; error: GenError | null }
  | { kind: 'failed'; job: number; error: GenError }
  | { kind: 'cancelled'; job: number };

/** The server's checkpoints and LoRAs; rejects with the reason (in
 *  English) when no server is usable. */
export function generationModels(): Promise<Models> {
  return invoke<Models>('comfyui_generation_models');
}

/** Starts a generation; rejects with a `GenError`. */
export function startGeneration(request: GenerateRequest): Promise<Started> {
  return invoke<Started>('comfyui_generate', { request });
}

export function cancelGeneration(job: number): Promise<void> {
  return invoke('comfyui_generation_cancel', { job });
}

/** Variant `index` of a finished job, as encoded bytes. */
export async function fetchVariant(job: number, index: number): Promise<Uint8Array> {
  const data = await invoke<ArrayBuffer | number[]>('comfyui_variant', { job, index });
  return data instanceof ArrayBuffer ? new Uint8Array(data) : Uint8Array.from(data);
}

/** The editor no longer needs the job; a running one is cancelled. */
export function releaseGeneration(job: number): Promise<void> {
  return invoke('comfyui_generation_release', { job });
}

export function onGenerationEvent(handler: (event: GenEvent) => void): Promise<UnlistenFn> {
  return listen<GenEvent>('comfyui:generation', (e) => handler(e.payload));
}

/** `bytes` as base64, in chunks small enough for `fromCharCode`. */
export function toBase64(bytes: Uint8Array): string {
  let text = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(text);
}
