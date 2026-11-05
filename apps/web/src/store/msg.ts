// Everything that changes the state: what the person did (intents), API answers, the stream.

import type { ChainDetail, EventRecord, Project, TicketDetail, TicketItem } from "../api/types";
import type { Route, Tab } from "./route";

/// What the person did.
export type Intent =
  | { type: "navigate"; route: Route }
  | { type: "selectProject"; project: string }
  | { type: "selectChain"; project: string; chain: string }
  | { type: "selectCard"; project: string; chain: string; key: string }
  | { type: "selectTab"; tab: Tab };

export type Msg =
  | Intent
  | { type: "started" }
  | { type: "journalTail"; seq: number; events: EventRecord[] }
  | { type: "loaded"; projects: Project[]; chains: ChainDetail[]; tickets: TicketItem[] }
  | { type: "projectsLoaded"; projects: Project[] }
  | { type: "chainLoaded"; chain: ChainDetail }
  | { type: "ticketsLoaded"; tickets: TicketItem[] }
  | { type: "detailLoaded"; detail: TicketDetail }
  | { type: "loadFailed"; error: string }
  | { type: "linkOpened" }
  | { type: "linkClosed" }
  | { type: "event"; event: EventRecord }
  | { type: "urlChanged"; route: Route };

/// A side effect for the runner; the reducer only describes it.
export type Effect =
  | { type: "fetchTail" }
  | { type: "loadAll" }
  | { type: "loadProjects" }
  | { type: "loadChain"; id: string }
  | { type: "loadTickets" }
  | { type: "loadDetail"; key: string }
  | { type: "connect"; since: number; delayMs: number }
  | { type: "pushUrl"; path: string };
