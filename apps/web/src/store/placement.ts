// Where cards sit on the canvas. A card dragged (or moved with the arrow keys) keeps its new place
// at once, as `pending`, while the daemon is asked to save it; the daemon's answer makes it the
// card's own place and the pending one goes. "Arrange" hands every card back to the layout.

import type { ChainDetail, Pos } from "../api/types";
import type { Effect } from "./msg";
import { withChain } from "./plan";
import type { State } from "./state";

export interface PlacementState {
  /// The place just given to a card, by ticket id, until the daemon confirms it; `null` is a card
  /// handed back to the layout.
  pending: Record<string, Pos | null>;
}

export const initialPlacement: PlacementState = { pending: {} };

/// What the person did.
export type PlacementIntent =
  | { type: "placeCard"; chain: string; ticket: string; key: string; pos: Pos | null }
  | { type: "arrangeChain"; chain: string };

/// What the daemon answered.
export type PlacementResult =
  | { type: "cardPlaced"; chain: string; ticket: string; pos: Pos | null }
  | { type: "cardPlaceRefused"; ticket: string; message: string }
  | { type: "chainArranged"; chain: ChainDetail };

export type PlacementMsg = PlacementIntent | PlacementResult;

export type PlacementEffect = PlacementIntent;

const same = (a: Pos | null | undefined, b: Pos | null | undefined): boolean =>
  (a?.x ?? null) === (b?.x ?? null) && (a?.y ?? null) === (b?.y ?? null);

/// The chain with the ticket's own place set, or taken away.
const withPos = (c: ChainDetail, ticket: string, pos: Pos | null): ChainDetail => ({
  ...c,
  tickets: c.tickets.map((t) => {
    if (t.id !== ticket) return t;
    const { pos: _old, ...rest } = t;
    return pos ? { ...rest, pos } : rest;
  }),
});

const without = (pending: Record<string, Pos | null>, ticket: string): Record<string, Pos | null> => {
  const { [ticket]: _gone, ...rest } = pending;
  return rest;
};

/// The reducer of card places.
export function reducePlacement(s: State, msg: PlacementMsg): [State, Effect[]] {
  const set = (pending: Record<string, Pos | null>, base: State = s): State => ({ ...base, placement: { pending } });
  switch (msg.type) {
    case "placeCard":
      return [set({ ...s.placement.pending, [msg.ticket]: msg.pos }), [msg]];
    case "arrangeChain":
      return [s, [msg]];
    case "cardPlaced": {
      const c = s.chains[msg.chain];
      // A newer move is still on its way: it decides what the card shows.
      if (msg.ticket in s.placement.pending && !same(s.placement.pending[msg.ticket], msg.pos)) return [s, []];
      const next = set(without(s.placement.pending, msg.ticket));
      return [c ? withChain(next, withPos(c, msg.ticket, msg.pos)) : next, []];
    }
    case "cardPlaceRefused":
      return [{ ...set(without(s.placement.pending, msg.ticket)), edges: { ...s.edges, error: msg.message } }, []];
    case "chainArranged": {
      const mine = new Set(msg.chain.tickets.map((t) => t.id));
      const pending = Object.fromEntries(Object.entries(s.placement.pending).filter(([id]) => !mine.has(id)));
      return [withChain(set(pending), msg.chain), []];
    }
  }
}
