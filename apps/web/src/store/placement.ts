// Where cards sit on the canvas and how big they are. A card dragged (or moved with the arrow keys)
// or resized keeps its new place or size at once, as `pending` or `sizes`, while the daemon is
// asked to save it; the daemon's answer makes it the card's own and the pending one goes.
// "Arrange" hands every card back to the layout. A card that has not started can also go to
// another epic of its project (the card menu, or a drag onto an epic's row in the sidebar): the
// daemon decides, and its refusal shows on the diagram like the others.

import type { ChainDetail, MoveInput, Pos, Size, Ticket, TicketDetail } from "../api/types";
import type { Effect } from "./msg";
import { withChain } from "./plan";
import type { State } from "./state";

export interface PlacementState {
  /// The place just given to a card, by ticket id, until the daemon confirms it; `null` is a card
  /// handed back to the layout.
  pending: Record<string, Pos | null>;
  /// The same for sizes; `null` is a card that fits its text again.
  sizes: Record<string, Size | null>;
  /// The epic (chain id) whose sidebar row a dragged card is over: the row shows as a drop target.
  dropEpic: string | undefined;
}

export const initialPlacement: PlacementState = { pending: {}, sizes: {}, dropEpic: undefined };

/// What the person did.
export type PlacementIntent =
  | { type: "placeCard"; chain: string; ticket: string; key: string; pos: Pos | null }
  | { type: "sizeCard"; chain: string; ticket: string; key: string; size: Size | null }
  | { type: "arrangeChain"; chain: string }
  | { type: "overEpic"; chain: string | undefined }
  /// The card `key`, now in the chain `from` (an id), goes to the epic `chain` (id or name): to the
  /// end, or next to a card of it.
  | { type: "moveToEpic"; key: string; from: string; chain: string; place?: MoveInput };

/// What the daemon answered.
export type PlacementResult =
  | { type: "cardPlaced"; chain: string; ticket: string; pos: Pos | null }
  | { type: "cardSized"; chain: string; ticket: string; size: Size | null }
  | { type: "cardPlaceRefused"; ticket: string; message: string }
  | { type: "chainArranged"; chain: ChainDetail }
  | { type: "movedToEpic"; from: string; detail: TicketDetail };

export type PlacementMsg = PlacementIntent | PlacementResult;

export type PlacementEffect = Extract<PlacementIntent, { type: "placeCard" | "sizeCard" | "arrangeChain" | "moveToEpic" }>;

const same = (a: Pos | null | undefined, b: Pos | null | undefined): boolean =>
  (a?.x ?? null) === (b?.x ?? null) && (a?.y ?? null) === (b?.y ?? null);

const sameSize = (a: Size | null | undefined, b: Size | null | undefined): boolean =>
  (a?.w ?? null) === (b?.w ?? null) && (a?.h ?? null) === (b?.h ?? null);

/// The chain with one field of the ticket set, or taken away.
const withTicket = (c: ChainDetail, ticket: string, change: (t: Ticket) => Ticket): ChainDetail => ({
  ...c,
  tickets: c.tickets.map((t) => (t.id === ticket ? change(t) : t)),
});

const withPos = (c: ChainDetail, ticket: string, pos: Pos | null): ChainDetail =>
  withTicket(c, ticket, (t) => {
    const { pos: _old, ...rest } = t;
    return pos ? { ...rest, pos } : rest;
  });

const withSize = (c: ChainDetail, ticket: string, size: Size | null): ChainDetail =>
  withTicket(c, ticket, (t) => {
    const { size: _old, ...rest } = t;
    return size ? { ...rest, size } : rest;
  });

const without = <T>(map: Record<string, T>, ticket: string): Record<string, T> => {
  const { [ticket]: _gone, ...rest } = map;
  return rest;
};

/// The reducer of card places and sizes.
export function reducePlacement(s: State, msg: PlacementMsg): [State, Effect[]] {
  const set = (placement: Partial<PlacementState>, base: State = s): State => ({
    ...base,
    placement: { ...s.placement, ...placement },
  });
  switch (msg.type) {
    case "placeCard":
      return [set({ pending: { ...s.placement.pending, [msg.ticket]: msg.pos } }), [msg]];
    case "sizeCard":
      return [set({ sizes: { ...s.placement.sizes, [msg.ticket]: msg.size } }), [msg]];
    case "arrangeChain":
      return [s, [msg]];
    case "overEpic":
      return [set({ dropEpic: msg.chain }), []];
    case "moveToEpic":
      return [set({ dropEpic: undefined }), [msg]];
    case "movedToEpic": {
      // Both diagrams are read again: the old one loses the card, the new one shows it at the end.
      const d = msg.detail;
      const next = set({ pending: without(s.placement.pending, d.ticket.id) }, { ...s, details: { ...s.details, [d.ticket.key]: d } });
      return [next, [{ type: "loadChain", id: msg.from }, { type: "loadChain", id: d.ticket.chain_id }]];
    }
    case "cardPlaced": {
      const c = s.chains[msg.chain];
      // A newer move is still on its way: it decides what the card shows.
      if (msg.ticket in s.placement.pending && !same(s.placement.pending[msg.ticket], msg.pos)) return [s, []];
      const next = set({ pending: without(s.placement.pending, msg.ticket) });
      return [c ? withChain(next, withPos(c, msg.ticket, msg.pos)) : next, []];
    }
    case "cardSized": {
      const c = s.chains[msg.chain];
      if (msg.ticket in s.placement.sizes && !sameSize(s.placement.sizes[msg.ticket], msg.size)) return [s, []];
      const next = set({ sizes: without(s.placement.sizes, msg.ticket) });
      return [c ? withChain(next, withSize(c, msg.ticket, msg.size)) : next, []];
    }
    case "cardPlaceRefused": {
      const next = set({ pending: without(s.placement.pending, msg.ticket), sizes: without(s.placement.sizes, msg.ticket) });
      return [{ ...next, edges: { ...s.edges, error: msg.message } }, []];
    }
    case "chainArranged": {
      const mine = new Set(msg.chain.tickets.map((t) => t.id));
      const keep = <T>(m: Record<string, T>) => Object.fromEntries(Object.entries(m).filter(([id]) => !mine.has(id)));
      return [withChain(set({ pending: keep(s.placement.pending), sizes: keep(s.placement.sizes) }), msg.chain), []];
    }
  }
}
