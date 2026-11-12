// What an Assistant chat shows, and how the CLI's events change it
// (docs/specs/ai-chat-panel.md AC-05, AC-06, AC-11, AC-20). The events
// mirror `ChatEvent` in src-tauri/src/assistant/mod.rs. Plain TypeScript,
// so a Node-side spec can test it.

/** The recognised cause of a failed answer (AC-20, AC-27); ai-chat-clis
 *  adds `localServer` (AC-08) and `noTools` (Edge cases). */
export type Hint =
  'signIn' | 'offline' | 'unknownModel' | 'permission' | 'unexpected' | 'localServer' | 'noTools';

/** How an answer ended. */
export type Outcome = 'finished' | 'stopped' | 'failed';

/** A failed answer's error. */
export interface Failure {
  text: string;
  hint: Hint;
}

/** One step of an answer, as the `assistant-event` payload carries it. */
export type ChatEvent =
  | { kind: 'session'; id: string }
  | { kind: 'text'; text: string }
  | { kind: 'toolStart'; id: string; name: string; args: unknown }
  | { kind: 'toolEnd'; id: string; ok: boolean; text: string; images: [number, number][] }
  | { kind: 'done'; outcome: Outcome; error: Failure | null };

/** A tool row's state: running, then success / error, or marked when the
 *  answer was stopped or failed before the call returned. */
export type ToolState = 'running' | 'ok' | 'error' | 'stopped' | 'failed';

/** One entry of the message list. */
export type ChatItem =
  | { readonly kind: 'user'; readonly text: string }
  | { readonly kind: 'agent'; readonly text: string }
  | {
      readonly kind: 'tool';
      readonly id: string;
      readonly name: string;
      readonly args: unknown;
      readonly state: ToolState;
      readonly text: string;
      readonly images: readonly (readonly [number, number])[];
    }
  /** The answer was stopped or failed (AC-11, AC-20). */
  | { readonly kind: 'end'; readonly outcome: 'stopped' | 'failed'; readonly error: Failure | null }
  /** The answer could not start (AC-28). */
  | { readonly kind: 'error'; readonly text: string };

/** `items` after `event`. A `session` event changes no item; the caller
 *  keeps the session id. */
export function applyEvent(items: readonly ChatItem[], event: ChatEvent): ChatItem[] {
  switch (event.kind) {
    case 'session':
      return [...items];
    case 'text': {
      const last = items.at(-1);
      if (last?.kind === 'agent') {
        return [...items.slice(0, -1), { kind: 'agent', text: last.text + event.text }];
      }
      return [...items, { kind: 'agent', text: event.text }];
    }
    case 'toolStart':
      return [
        ...items,
        {
          kind: 'tool',
          id: event.id,
          name: event.name,
          args: event.args,
          state: 'running',
          text: '',
          images: [],
        },
      ];
    case 'toolEnd':
      return items.map((it) =>
        it.kind === 'tool' && it.id === event.id && it.state === 'running'
          ? { ...it, state: event.ok ? 'ok' : 'error', text: event.text, images: event.images }
          : it,
      );
    case 'done': {
      if (event.outcome === 'finished') return [...items];
      const mark: ToolState = event.outcome === 'stopped' ? 'stopped' : 'failed';
      const marked = items.map((it) =>
        it.kind === 'tool' && it.state === 'running' ? { ...it, state: mark } : it,
      );
      return [...marked, { kind: 'end', outcome: event.outcome, error: event.error }];
    }
  }
}
