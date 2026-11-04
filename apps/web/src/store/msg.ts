// Everything that changes the state: what the person did (intents), API answers, the stream.

import type { ChainDetail, EventRecord, TicketItem } from "../api/types";
import type { Route } from "./route";

/// What the person did.
export type Intent =
  | { type: "toggleNeedsHuman"; on: boolean }
  | { type: "navigate"; route: Route };

export type Msg =
  | Intent
  | { type: "started" }
  | { type: "journalTail"; seq: number }
  | { type: "loaded"; chains: ChainDetail[]; tickets: TicketItem[] }
  | { type: "chainLoaded"; chain: ChainDetail }
  | { type: "ticketsLoaded"; tickets: TicketItem[] }
  | { type: "loadFailed"; error: string }
  | { type: "linkOpened" }
  | { type: "linkClosed" }
  | { type: "event"; event: EventRecord }
  | { type: "urlChanged"; route: Route };

/// A side effect for the runner; the reducer only describes it.
export type Effect =
  | { type: "fetchTail" }
  | { type: "loadAll" }
  | { type: "loadChain"; id: string }
  | { type: "loadTickets" }
  | { type: "connect"; since: number; delayMs: number }
  | { type: "pushUrl"; path: string };
