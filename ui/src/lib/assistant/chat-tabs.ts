// Which tab each Assistant chat belongs to (docs/specs/ai-chat-panel.md
// AC-26): the live host runs a chat's tool calls on that tab only. Plain
// TypeScript, kept apart from the chat store so the live host does not
// depend on it.

const tabs = new Map<string, number>();

/** The chat `chat` belongs to the tab with key `key`. */
export function bindChatTab(chat: string, key: number): void {
  tabs.set(chat, key);
}

/** The chat ended. */
export function unbindChatTab(chat: string): void {
  tabs.delete(chat);
}

/** The key of the tab `chat` belongs to, or `null` for an ended chat. */
export function chatTabKey(chat: string): number | null {
  return tabs.get(chat) ?? null;
}
