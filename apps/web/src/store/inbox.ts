// The inbox as the window shows it: the daemon's list (core decides what waits on a person),
// grouped by kind for display. Nothing here decides what belongs in the inbox.

import type { InboxItem, InboxKind } from "../api/types";
import type { Route } from "./route";
import type { State } from "./state";

/// Display order of the groups, the order of the kinds in core.
export const KIND_ORDER: InboxKind[] = ["proposal", "question", "blocked", "review", "thread", "leftovers", "design_question", "pin"];

export const KIND_TITLE: Record<InboxKind, string> = {
  proposal: "Awaiting approval",
  question: "Agent questions",
  blocked: "Blocked",
  review: "Waiting for review",
  thread: "Threads to answer",
  leftovers: "Leftovers",
  design_question: "Questions of the assistant",
  pin: "Questions of the assistant about a design",
};

export interface InboxGroup {
  kind: InboxKind;
  title: string;
  items: InboxItem[];
}

/// The non-empty groups in display order; inside a group the daemon's newest-first order stays.
export function groupInbox(items: InboxItem[]): InboxGroup[] {
  return KIND_ORDER.map((kind) => ({ kind, title: KIND_TITLE[kind], items: items.filter((i) => i.kind === kind) }))
    .filter((g) => g.items.length > 0);
}

/// A key that tells two items apart (a card may have several threads).
export function itemKey(i: InboxItem): string {
  return `${i.kind}:${i.ticket ?? ""}:${i.thread ?? ""}${i.pin ?? ""}`;
}

/// The route of the item's card, if it has one and the page knows its chain.
export function itemRoute(s: State, i: InboxItem): Route | undefined {
  if (!i.ticket || !i.chain) return undefined;
  const chain = s.chains[i.chain];
  if (!chain) return undefined;
  const project = s.projects.find((p) => p.id === chain.chain.project_id);
  return { name: "card", project: project?.key ?? chain.chain.project_id, chain: chain.chain.name, key: i.ticket };
}
