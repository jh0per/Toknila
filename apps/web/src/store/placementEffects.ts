// The network side of card places: one PATCH per move, one POST for "Arrange". It decides
// nothing; a refusal goes to the reducer with the daemon's own words.

import { patch, post } from "../api/http";
import type { ChainDetail, TicketDetail } from "../api/types";
import type { Msg } from "./msg";
import type { PlacementEffect } from "./placement";

const enc = encodeURIComponent;
const text = (e: unknown) => (e instanceof Error ? e.message : String(e));

/// Runs one effect of card places.
export function runPlacementEffect(e: PlacementEffect, dispatch: (msg: Msg) => void): void {
  switch (e.type) {
    case "placeCard":
      patch<TicketDetail>(`/tickets/${enc(e.key)}`, { pos: e.pos })
        .then(() => dispatch({ type: "cardPlaced", chain: e.chain, ticket: e.ticket, pos: e.pos }))
        .catch((err: unknown) => dispatch({ type: "cardPlaceRefused", ticket: e.ticket, message: text(err) }));
      return;
    case "arrangeChain":
      post<ChainDetail>(`/chains/${enc(e.chain)}/arrange`, {})
        .then((chain) => dispatch({ type: "chainArranged", chain }))
        .catch((err: unknown) => dispatch({ type: "edgeRefused", message: text(err) }));
      return;
  }
}
