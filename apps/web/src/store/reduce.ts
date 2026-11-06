// The page's state machine: reduce(state, msg) -> [state, effects]. Pure: no DOM, no network.

import type { EventRecord } from "../api/types";
import type { Effect, Msg } from "./msg";
import { type Route, routePath } from "./route";
import { reduceSettings, setupLoads } from "./settings";
import { FIRST_RETRY_MS, MAX_RETRY_MS, type State, chainOfTicket, ticketOfEvent } from "./state";

export function reduce(s: State, msg: Msg): [State, Effect[]] {
  switch (msg.type) {
    case "started":
      return [s, [{ type: "fetchTail" }]];
    case "journalTail":
      // The first load already has the state up to this event; the stream starts after it.
      return [
        { ...s, lastSeq: msg.seq, events: msg.events },
        [{ type: "connect", since: msg.seq, delayMs: 0 }],
      ];
    case "linkOpened":
      return [{ ...s, link: "live", retryMs: FIRST_RETRY_MS }, [{ type: "loadAll" }]];
    case "linkClosed":
      return [
        { ...s, link: "offline", retryMs: Math.min(s.retryMs * 2, MAX_RETRY_MS) },
        [{ type: "connect", since: s.lastSeq, delayMs: s.retryMs }],
      ];
    case "loaded": {
      const chains = Object.fromEntries(msg.chains.map((c) => [c.chain.id, c]));
      const next = {
        ...s,
        projects: msg.projects,
        chains,
        chainOrder: msg.chains.map((c) => c.chain.id),
        loaded: true,
        error: "",
      };
      // First run: no project yet, so the window offers the checks and a form to add one.
      return [next, [...enter(next.route), ...(msg.projects.length ? [] : setupLoads)]];
    }
    case "projectsLoaded":
      return [{ ...s, projects: msg.projects }, []];
    case "agentsLoaded":
      return [{ ...s, agents: msg.agents }, []];
    case "chainLoaded": {
      const id = msg.chain.chain.id;
      const chainOrder = s.chainOrder.includes(id) ? s.chainOrder : [...s.chainOrder, id];
      return [{ ...s, chains: { ...s.chains, [id]: msg.chain }, chainOrder, error: "" }, []];
    }
    case "inboxLoaded":
      return [{ ...s, inbox: msg.items }, []];
    case "detailLoaded":
      return [{ ...s, details: { ...s.details, [msg.detail.ticket.key]: msg.detail } }, []];
    case "threadsLoaded":
      return [{ ...s, threads: { ...s.threads, [msg.threads.ticket]: msg.threads } }, []];
    case "pickVersion":
      return [{ ...s, versionPick: { ...s.versionPick, [msg.thread]: msg.number } }, []];
    case "threadAction":
    case "leftovers":
    case "ticketAction":
    case "addComment":
      return [s, [msg]];
    case "loadFailed":
      return [{ ...s, error: msg.error }, []];
    case "event":
      return applyEvent(s, msg.event);
    case "selectTab":
      return [{ ...s, tab: msg.tab }, []];
    case "navigate":
      return go(s, msg.route);
    case "selectProject":
      return go(s, { name: "project", project: msg.project });
    case "selectChain":
      return go(s, { name: "projectChain", project: msg.project, chain: msg.chain });
    case "selectCard":
      return go(s, { name: "card", project: msg.project, chain: msg.chain, key: msg.key });
    case "urlChanged":
      return [{ ...s, route: msg.route }, enter(msg.route)];
    case "projectAdded": {
      const [added, fx] = reduceSettings(s, msg);
      const [moved, more] = go(added, { name: "settings", project: msg.project.key });
      return [moved, [...fx, ...more]];
    }
    case "recheck":
    case "addProject":
    case "saveProject":
    case "importChain":
    case "checksLoaded":
    case "profilesLoaded":
    case "addRejected":
    case "projectSaved":
    case "chainImported":
    case "settingsRejected":
      return reduceSettings(s, msg);
  }
}

/// Moves to a route: the URL changes by an effect, never by the reducer itself.
function go(s: State, route: Route): [State, Effect[]] {
  return [{ ...s, route }, [{ type: "pushUrl", path: routePath(route) }, ...enter(route)]];
}

/// What a screen needs fetched when it opens.
function enter(route: Route): Effect[] {
  if (route.name === "settings") return setupLoads;
  return route.name === "card"
    ? [{ type: "loadDetail", key: route.key }, { type: "loadThreads", key: route.key }]
    : [];
}

/// A journal event refreshes what it touches: its chain, the inbox, and the open card. A gap in
/// `seq` or an event the page cannot place reloads everything.
function applyEvent(s: State, e: EventRecord): [State, Effect[]] {
  if (e.seq <= s.lastSeq) return [s, []];
  const next = { ...s, lastSeq: e.seq, events: [...s.events, e] };
  if (e.seq !== s.lastSeq + 1) return [next, [{ type: "loadAll" }]];
  if (e.entity === "project") return [next, [{ type: "loadProjects" }, { type: "loadAgents" }]];
  if (e.entity === "thread") return [next, threadEventEffects(s, ticketOfEvent(e))];
  const chain = chainOf(s, e);
  if (!chain) return [next, [{ type: "loadAll" }]];
  const fx: Effect[] = [{ type: "loadChain", id: chain }, { type: "loadInbox" }, { type: "loadAgents" }];
  const ticket = ticketOfEvent(e);
  const open = s.route.name === "card" ? s.details[s.route.key]?.ticket : undefined;
  if (open && ticket === open.id) fx.push({ type: "loadDetail", key: open.key });
  return [next, fx];
}

/// A thread event changes the card's threads and its phase; the hand-in gate may move its status.
function threadEventEffects(s: State, key: string | undefined): Effect[] {
  const fx: Effect[] = [{ type: "loadInbox" }, { type: "loadAgents" }];
  if (key && s.route.name === "card" && s.route.key === key) {
    fx.push({ type: "loadThreads", key }, { type: "loadDetail", key });
  }
  return fx;
}

function chainOf(s: State, e: EventRecord): string | undefined {
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
