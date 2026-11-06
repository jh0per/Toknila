// Everything that changes the state: what the person did (intents), API answers, the stream.

import type {
  ChainDetail, CommentKind, EventRecord, Project, TicketAction, TicketDetail, TicketItem, TicketThreads,
} from "../api/types";
import type { Route, Tab } from "./route";

/// What the person did.
export type Intent =
  | { type: "navigate"; route: Route }
  | { type: "selectProject"; project: string }
  | { type: "selectChain"; project: string; chain: string }
  | { type: "selectCard"; project: string; chain: string; key: string }
  | { type: "selectTab"; tab: Tab }
  | { type: "pickVersion"; thread: string; number: number }
  | CardCommand
  | ThreadAction;

/// What a person does to a card: an action of its status (a return carries the comment), or a
/// comment of the template's kinds.
export type CardCommand =
  | { type: "ticketAction"; key: string; action: TicketAction; text?: string }
  | { type: "addComment"; key: string; kind: CommentKind; text: string };

/// What a person does in a thread; `text` is the message or the comment of a request for changes.
export type ThreadAction = {
  type: "threadAction";
  key: string;
  thread: string;
  action: "message" | "changes" | "approve";
  text?: string;
};

export type Msg =
  | Intent
  | { type: "started" }
  | { type: "journalTail"; seq: number; events: EventRecord[] }
  | { type: "loaded"; projects: Project[]; chains: ChainDetail[]; tickets: TicketItem[] }
  | { type: "projectsLoaded"; projects: Project[] }
  | { type: "chainLoaded"; chain: ChainDetail }
  | { type: "ticketsLoaded"; tickets: TicketItem[] }
  | { type: "detailLoaded"; detail: TicketDetail }
  | { type: "threadsLoaded"; threads: TicketThreads }
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
  | { type: "loadThreads"; key: string }
  | CardCommand
  | ThreadAction
  | { type: "connect"; since: number; delayMs: number }
  | { type: "pushUrl"; path: string };
