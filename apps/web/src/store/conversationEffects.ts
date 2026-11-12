// The network side of the chat: one page of the project's conversation. It decides nothing; the
// answer goes to the reducer.

import { get } from "../api/http";
import type { ConversationPage } from "../api/types";
import { PAGE, type ConversationEffect } from "./conversation";
import type { Msg } from "./msg";

/// Loads the newest page of the project's conversation, or the one before `e.before`.
export function runConversationEffect(e: ConversationEffect, dispatch: (msg: Msg) => void): void {
  const before = e.before === undefined ? "" : `&before=${encodeURIComponent(e.before)}`;
  get<ConversationPage>(`/projects/${encodeURIComponent(e.project)}/conversation?limit=${PAGE}${before}`)
    .then((page) => dispatch({ type: "conversationLoaded", project: e.project, page, before: e.before }))
    .catch((err) => dispatch({ type: "conversationFailed", project: e.project, error: String(err instanceof Error ? err.message : err) }));
}
