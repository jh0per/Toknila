// The network side of moving a design reference between cards: one request, so the daemon lands
// it whole or changes nothing. It decides nothing; a refusal goes to the caller with its words.

import { post } from "../api/http";
import type { TicketDetail } from "../api/types";

/// Gives the reference at `index` of `card`'s list to `target`, a card of the same chain.
export function moveDesign(card: string, index: number, target: string): Promise<TicketDetail> {
  return post<TicketDetail>(`/tickets/${encodeURIComponent(card)}/designs/${index}/move`, { target });
}
