// The page's one normalized store and the selectors the views read.

import type {
  ChainDetail, EventRecord, InboxItem, Project, ProjectAgents, Suggestion, Ticket, TicketDetail, TicketThreads,
} from "../api/types";
import { type AssistantSlice, initialAssistant } from "./assistant";
import { type EdgesState, initialEdges } from "./edges";
import { type EpicUi, initialEpic } from "./epic";
import { type RoadmapUi, initialRoadmap } from "./roadmap";
import type { OpenedCard } from "./follow";
import { type PlacementState, initialPlacement } from "./placement";
import { type ImportState, initialImports } from "./imports";
import { type PlanState, initialPlan } from "./plan";
import { type QuickState, initialQuick } from "./quick";
import { type ConversationSlice, initialConversation } from "./conversation";
import { type PlannerSlice, initialPlanner } from "./planner";
import { type ReleaseUi, initialRelease } from "./release";
import { type RemovalState, initialRemoval } from "./removal";
import { type Route, type Tab, routePath } from "./route";
import { type SettingsState, initialSettings } from "./settings";
import type { Theme } from "./theme";
import { type UpdatesState, initialUpdates } from "./updates";
import { type UsageState, initialUsage } from "./usage";
import { type AttachState, initialAttach } from "./attach";

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
  /// The planner's standing suggestion of each project that has one (the next step).
  suggestions: Suggestion[];
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
  /// What the daemon said an answer did ("Allowed always in this project: Bash(cargo test *)").
  notice: string;
  link: Link;
  /// Newest journal seq applied; the stream resumes after it.
  lastSeq: number;
  /// Delay before the next reconnect attempt, doubled on each failure.
  retryMs: number;
  route: Route;
  /// The page the settings were opened from, for "Back to work".
  settingsFrom: Route | undefined;
  tab: Tab;
  /// Settings and onboarding: checks, profiles, the answers of the last form.
  settings: SettingsState;
  /// Planning from the window: the open dialog, a card being edited or moved, archived projects.
  plan: PlanState;
  /// Each project's planner: whether its process is alive.
  planner: PlannerSlice;
  /// Each project's conversation with its assistant, for the chats that have been opened.
  conversation: ConversationSlice;
  /// Imports from the window and the plans found in a repository.
  imports: ImportState;
  /// Deleting a project or a chain: the dialog's answer.
  removal: RemovalState;
  /// Releasing a finished chain: the dialog's offer and the run's steps.
  release: ReleaseUi;
  /// The epic and release panels, and the version a new chain suggests.
  epic: EpicUi;
  /// The roadmap screen: the daemon's refusal of the last move.
  roadmap: RoadmapUi;
  /// Editing dependencies: the selected line and the daemon's last refusal.
  edges: EdgesState;
  placement: PlacementState;
  /// The quick new-card field and the card menu on the diagram.
  quick: QuickState;
  /// The Usage page: window, metric and the daemon's answer.
  usage: UsageState;
  /// Files on their way to a card's designs, and the last drop's refusals.
  attach: AttachState;
  /// Updates of the desktop app (empty in a browser).
  updates: UpdatesState;
  /// The viewer's colour theme; remembered in this browser only.
  theme: Theme;
  /// The assistant panel beside the screens: open, width, filter, the context chip.
  assistant: AssistantSlice;
  /// The "?" list of keyboard shortcuts is open.
  shortcutsOpen: boolean;
  /// The card the assistant just opened here, while its notice (with Back) is up.
  opened: OpenedCard | undefined;
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
    suggestions: [],
    details: {},
    threads: {},
    versionPick: {},
    events: [],
    loaded: false,
    error: "",
    notice: "",
    link: "connecting",
    lastSeq: 0,
    retryMs: FIRST_RETRY_MS,
    route,
    settingsFrom: undefined,
    tab: "details",
    settings: initialSettings,
    plan: initialPlan,
    planner: initialPlanner,
    conversation: initialConversation,
    imports: initialImports,
    removal: initialRemoval,
    release: initialRelease,
    epic: initialEpic,
    roadmap: initialRoadmap,
    edges: initialEdges,
    placement: initialPlacement,
    quick: initialQuick,
    usage: initialUsage,
    attach: initialAttach,
    updates: initialUpdates,
    theme: "system",
    assistant: initialAssistant,
    shortcutsOpen: false,
    opened: undefined,
  };
}

function allChains(s: State): ChainDetail[] {
  return s.chainOrder.map((id) => s.chains[id]).filter((c): c is ChainDetail => !!c);
}

/// Projects the sidebar lists: every one that is not archived, in the daemon's order.
export const activeProjects = (s: State): Project[] => s.projects.filter((p) => !p.archived);

/// Archived projects, listed only when the person asks to see them.
export const archivedProjects = (s: State): Project[] => s.projects.filter((p) => p.archived);

/// The project in focus: the one in the route, the chain's for the `/chains` paths, else the first one
/// that is not archived.
export function currentProject(s: State): Project | undefined {
  const r = s.route;
  if ("project" in r && r.project) return s.projects.find((p) => p.key === r.project || p.id === r.project);
  if (r.name === "chain") {
    const c = allChains(s).find((x) => x.chain.name === r.chain);
    return s.projects.find((p) => p.id === c?.chain.project_id);
  }
  return activeProjects(s)[0];
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

/// The chain shown when a project is open but no chain is picked: the one with the newest journal
/// entry (the most recently active), else the first in the daemon's order.
export function defaultChain(s: State, project: Project | undefined): ChainDetail | undefined {
  const mine = chainsOf(s, project);
  if (mine.length <= 1) return mine[0];
  const ids = new Set(mine.map((c) => c.chain.id));
  for (let i = s.events.length - 1; i >= 0; i--) {
    const id = chainOfEvent(s, s.events[i]);
    if (id && ids.has(id)) return s.chains[id];
  }
  return mine[0];
}

/// The chain in focus: the one the route names, or the project's default when it names none.
export function currentChain(s: State): ChainDetail | undefined {
  const r = s.route;
  // The planner's screen shows the project's most recently active chain above its terminal, so
  // the diagram follows what the planner edits.
  if (r.name === "project" || r.name === "chains" || r.name === "planner") {
    return defaultChain(s, currentProject(s));
  }
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

/// The chain a journal event is about, if the page can place it.
export function chainOfEvent(s: State, e: EventRecord): string | undefined {
  const p = (e.payload ?? {}) as Record<string, Record<string, unknown> | undefined>;
  const str = (v: unknown) => (typeof v === "string" ? v : undefined);
  switch (e.entity) {
    case "chain":
      return str(p.chain?.id) ?? str(p.edge?.chain_id);
    case "ticket":
      return str(p.ticket?.chain_id);
    case "release":
      return str(p.release?.chain_id);
    case "comment":
    case "subtask": {
      const ticket = ticketOfEvent(e);
      return ticket && chainOfTicket(s, ticket);
    }
    default:
      return undefined;
  }
}

/// The cards of a chain in its order (the dependency order the daemon computes), for pickers.
export function ticketsInOrder(c: ChainDetail): Ticket[] {
  const byId = new Map(c.tickets.map((t) => [t.id, t]));
  const listed = c.order.filter((n) => n.kind === "ticket").map((n) => byId.get(n.id)).filter((t): t is Ticket => !!t);
  const seen = new Set(listed.map((t) => t.id));
  return [...listed, ...c.tickets.filter((t) => !seen.has(t.id))];
}

/// The journal of one ticket, oldest first.
export function ticketEvents(s: State, ticketId: string): EventRecord[] {
  return s.events.filter((e) => ticketOfEvent(e) === ticketId);
}
