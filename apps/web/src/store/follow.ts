// The assistant opens a card in the window: a `window.show_card` event in the journal. A window
// follows it only when it looks at that project's assistant or chain; any other window ignores it.
// The notice says so and offers Back, to the page the person was on.

import type { EventRecord } from "../api/types";
import type { Route } from "./route";
import { type State, currentProject } from "./state";

/// The card the assistant opened and the page to go Back to.
export interface OpenedCard {
  key: string;
  /// The journal entry that opened it, so a late timer only dismisses its own notice.
  seq: number;
  back: Route;
}

export type FollowIntent = { type: "backFromOpened" } | { type: "dismissOpened" };

export type FollowResult = { type: "openedExpired"; seq: number };

export type FollowEffect = { type: "expireOpened"; seq: number };

/// How long the notice stays when nobody answers it.
export const OPENED_MS = 15_000;

/// The screens that count as looking at a project's plan: its assistant, a chain, a card of one.
const WATCHING: Route["name"][] = ["planner", "projectChain", "chain", "card"];

const str = (v: unknown): string => (typeof v === "string" ? v : "");

/// The card page a `window.show_card` event asks this window for, or nothing: another kind of
/// event, a project this window is not looking at, a screen that is not the assistant's or a
/// chain's, or the card already shown.
export function followTarget(s: State, e: EventRecord): Extract<Route, { name: "card" }> | undefined {
  if (e.entity !== "window" || e.kind !== "show_card") return undefined;
  const p = (e.payload ?? {}) as Record<string, unknown>;
  const [project, key, chain, ticket] = [str(p.project), str(p.project_key), str(p.chain), str(p.ticket)];
  if (!project || !key || !chain || !ticket) return undefined;
  if (!WATCHING.includes(s.route.name) || currentProject(s)?.id !== project) return undefined;
  if (s.route.name === "card" && s.route.key === ticket) return undefined;
  return { name: "card", project: key, chain, key: ticket };
}

/// The notice for the card just opened. A second card opened while the first one's notice is
/// still up keeps the first Back: the page the person was really on.
export function opened(s: State, target: Extract<Route, { name: "card" }>, seq: number): OpenedCard {
  return { key: target.key, seq, back: s.opened?.back ?? s.route };
}
