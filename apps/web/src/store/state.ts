// The page's one normalized store and the selectors the views read.

import type {
  ChainDetail, EventRecord, InboxItem, Project, ProjectAgents, Ticket, TicketDetail, TicketThreads,
} from "../api/types";
import { type Route, type Tab, routePath } from "./route";
import { type SettingsState, initialSettings } from "./settings";
import { type UsageState, initialUsage } from "./usage";

export type Link = "connecting" | "live" | "offline";

export interface State {
  projects: Project[];
  /// Agent slots and queue per project, as the daemon computes them.
  agents: ProjectAgents[];
  /// Chains by id, in the order the daemon lists them.
  chains: Record<string, ChainDetail>;
  chainOrder: string[];
  /// Everything waiting on a person, as the daemon computes it (newest first).
  inbox: InboxItem[];
  /// Ticket details (subtasks, comments) by ticket key, for cards that were opened.
  details: Record<string, TicketDetail>;
  /// Threads (and the phase) by ticket key, for cards that were opened.
  threads: Record<string, TicketThreads>;
  /// The version number shown per thread id, when the person picked an older one.
  versionPick: Record<string, number>;
  /// The journal as the page has seen it, oldest first.
  events: EventRecord[];
  loaded: boolean;
  error: string;
  link: Link;
  /// Newest journal seq applied; the stream resumes after it.
  lastSeq: number;
  /// Delay before the next reconnect attempt, doubled on each failure.
  retryMs: number;
  route: Route;
  tab: Tab;
  /// Settings and onboarding: checks, profiles, the answers of the last form.
  settings: SettingsState;
  /// The Usage page: window, metric and the daemon's answer.
  usage: UsageState;
}

export const FIRST_RETRY_MS = 500;
export const MAX_RETRY_MS = 10_000;

export function initialState(route: Route): State {
  return {
    projects: [],
    agents: [],
    chains: {},
    chainOrder: [],
    inbox: [],
    details: {},
    threads: {},
    versionPick: {},
    events: [],
    loaded: false,
    error: "",
    link: "connecting",
    lastSeq: 0,
    retryMs: FIRST_RETRY_MS,
    route,
    tab: "details",
    settings: initialSettings,
    usage: initialUsage,
  };
}

function allChains(s: State): ChainDetail[] {
  return s.chainOrder.map((id) => s.chains[id]).filter((c): c is ChainDetail => !!c);
}

/// The project in focus: the one in the route, the chain's for phase 0 paths, else the first.
export function currentProject(s: State): Project | undefined {
  const r = s.route;
  if ("project" in r && r.project) return s.projects.find((p) => p.key === r.project || p.id === r.project);
  if (r.name === "chain") {
    const c = allChains(s).find((x) => x.chain.name === r.chain);
    return s.projects.find((p) => p.id === c?.chain.project_id);
  }
  return s.projects[0];
}

/// Slots of the project in focus: agents working now and how many may.
export function slotsOf(s: State): { used: number; limit: number } | undefined {
  const a = s.agents.find((x) => x.project === currentProject(s)?.id);
  return a && { used: a.used, limit: a.limit };
}

/// Place in line (1 is next) of a card waiting for an agent slot.
export function queuePosition(s: State, ticketId: string): number | undefined {
  for (const a of s.agents) {
    const q = a.queued.find((c) => c.ticket === ticketId);
    if (q) return q.position;
  }
  return undefined;
}

/// Chains of a project, in the daemon's order.
export function chainsOf(s: State, project: Project | undefined): ChainDetail[] {
  return project ? allChains(s).filter((c) => c.chain.project_id === project.id) : [];
}

/// The chain in focus, if the route names one.
export function currentChain(s: State): ChainDetail | undefined {
  const r = s.route;
  if (r.name !== "chain" && r.name !== "projectChain" && r.name !== "card") return undefined;
  const mine = r.name === "chain" ? allChains(s) : chainsOf(s, currentProject(s));
  return mine.find((c) => c.chain.name === r.chain);
}

/// The card in focus, if the route names one and the chain has it.
export function currentCard(s: State): Ticket | undefined {
  if (s.route.name !== "card") return undefined;
  const key = s.route.key;
  return currentChain(s)?.tickets.find((t) => t.key === key);
}

/// Every ticket the store knows, with its chain name, in chain order.
export function allTickets(s: State): { ticket: Ticket; chain: ChainDetail }[] {
  return allChains(s).flatMap((chain) => chain.tickets.map((ticket) => ({ ticket, chain })));
}

/// Tickets with a running agent (running state, until real runners exist).
export function runningAgents(s: State): { ticket: Ticket; chain: ChainDetail }[] {
  return allTickets(s).filter((x) => x.ticket.status === "running");
}

/// The card route of a ticket, if its chain's project is known.
export function cardRoute(s: State, chain: ChainDetail, ticket: Ticket): Route {
  const project = s.projects.find((p) => p.id === chain.chain.project_id);
  return { name: "card", project: project?.key ?? chain.chain.project_id, chain: chain.chain.name, key: ticket.key };
}

/// The path of a route, for links.
export const hrefOf = (r: Route): string => `${routePath(r)}${window.location.search}`;

/// The chain a ticket belongs to, if the store has it.
export function chainOfTicket(s: State, ticketId: string): string | undefined {
  return s.chainOrder.find((id) => s.chains[id]?.tickets.some((t) => t.id === ticketId));
}

/// The ticket a journal event is about, if it is about one.
export function ticketOfEvent(e: EventRecord): string | undefined {
  const p = (e.payload ?? {}) as Record<string, Record<string, unknown> | undefined>;
  const str = (v: unknown) => (typeof v === "string" ? v : undefined);
  switch (e.entity) {
    case "thread": return str(p.ticket);
    case "ticket": return str(p.ticket?.id) ?? (e.entity_id || undefined);
    case "comment": return str(p.comment?.ticket_id);
    case "subtask": return str(p.subtask?.ticket_id);
    default: return undefined;
  }
}

/// The journal of one ticket, oldest first.
export function ticketEvents(s: State, ticketId: string): EventRecord[] {
  return s.events.filter((e) => ticketOfEvent(e) === ticketId);
}
