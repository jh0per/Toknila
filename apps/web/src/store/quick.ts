// Quick actions on the diagram: the small "new card" field that opens where the person pointed
// (a connector released on empty canvas, a double-click, N, a card's menu), and the card menu's
// need for fresh allowed actions. The daemon decides everything else: a refusal of the new card
// shows under the field as it said it.

import type { NewTicket } from "../api/types";
import type { Effect } from "./msg";
import { reducePlan } from "./plan";
import type { State } from "./state";

/// The reason a cancellation starts with in the confirmation; the daemon asks for one.
export const CANCEL_REASON = "Cancelled in the window";

/// The new-card field: where it sits on the canvas (canvas pixels) and what the card follows.
export interface QuickCard {
  chain: string;
  x: number;
  y: number;
  /// Key of the card the new one comes after (it becomes a dependency of the new card).
  after: string | undefined;
  /// Key of the card the new one comes before (the new card becomes a dependency of that card).
  before: string | undefined;
}

export interface QuickState {
  card: QuickCard | undefined;
  /// The card whose allowed actions were last asked for (its menu opened, or it got focus);
  /// `fresh` once the daemon answered.
  peek: { key: string; fresh: boolean } | undefined;
}

export const initialQuick: QuickState = { card: undefined, peek: undefined };

/// What the person did.
export type QuickIntent =
  | { type: "openQuickCard"; chain: string; x: number; y: number; after?: string; before?: string }
  | { type: "closeQuickCard" }
  | { type: "submitQuickCard"; title: string }
  | { type: "peekCard"; key: string };

/// The card the field makes, as the API takes it: after, before, or at the end of the chain.
export function quickInput(q: QuickCard, title: string): NewTicket {
  const t = title.trim();
  if (q.after) return { title: t, after: [q.after] };
  if (q.before) return { title: t, before: [q.before] };
  return { title: t, at_end: true };
}

/// The reducer of the quick actions.
export function reduceQuick(s: State, msg: QuickIntent): [State, Effect[]] {
  switch (msg.type) {
    case "openQuickCard": {
      const card: QuickCard = { chain: msg.chain, x: msg.x, y: msg.y, after: msg.after, before: msg.before };
      return [{ ...s, quick: { ...s.quick, card }, plan: { ...s.plan, error: undefined } }, []];
    }
    case "closeQuickCard":
      return [{ ...s, quick: { ...s.quick, card: undefined }, plan: { ...s.plan, error: undefined } }, []];
    case "submitQuickCard": {
      const q = s.quick.card;
      if (!q) return [s, []];
      return reducePlan(s, { type: "createCard", chain: q.chain, input: quickInput(q, msg.title) });
    }
    case "peekCard":
      return [{ ...s, quick: { ...s.quick, peek: { key: msg.key, fresh: false } } }, [{ type: "loadDetail", key: msg.key }]];
  }
}
