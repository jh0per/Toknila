// The page's state machine: reduce(state, msg) -> [state, effects]. Pure: no DOM, no network.

import type { EventRecord } from "../api/types";
import type { Effect, Msg } from "./msg";
import { reducePlan, withChain } from "./plan";
import { type Route, routePath } from "./route";
import { reduceSettings, setupLoads } from "./settings";
import { loadUsage, reduceUsage } from "./usage";
import {
  FIRST_RETRY_MS, MAX_RETRY_MS, type State, cardRoute, chainOfEvent, currentProject, ticketOfEvent,
} from "./state";

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
      const first = !msg.projects.some((p) => !p.archived);
      return [next, [...enter(next.route, next), ...(first ? setupLoads : [])]];
    }
    case "projectsLoaded":
      return [{ ...s, projects: msg.projects }, []];
    case "agentsLoaded":
      return [{ ...s, agents: msg.agents }, []];
    case "chainLoaded":
      return [{ ...withChain(s, msg.chain), error: "" }, []];
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
    case "answer":
      return [s, [msg]];
    case "loadFailed":
      return [{ ...s, error: msg.error }, []];
    case "event": {
      const [next, fx] = applyEvent(s, msg.event);
      // Transcripts are not in the journal, but any activity may have moved the numbers.
      return [next, s.route.name === "usage" ? [...fx, loadUsage(next)] : fx];
    }
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
      return [{ ...s, route: msg.route }, enter(msg.route, s)];
    case "projectAdded": {
      const [added, fx] = reduceSettings(s, msg);
      const closed = { ...added, plan: { ...added.plan, dialog: undefined } };
      return then(closed, fx, { name: "settings", project: msg.project.key });
    }
    case "chainImported": {
      const [imported, fx] = reduceSettings(s, msg);
      // Imported from the window's dialog: the dialog closes on the new chain.
      if (s.plan.dialog?.kind !== "import") return [imported, fx];
      const closed = { ...imported, plan: { ...imported.plan, dialog: undefined } };
      return then(closed, fx, { name: "projectChain", project: msg.project, chain: msg.chain });
    }
    case "recheck":
    case "installCli":
    case "cliLoaded":
    case "cliRejected":
    case "editDraft":
    case "submitProject":
    case "repoChecked":
    case "saveProject":
    case "importChain":
    case "checksLoaded":
    case "profilesLoaded":
    case "addRejected":
    case "projectSaved":
    case "settingsRejected":
      return reduceSettings(s, msg);
    case "chainCreated": {
      const [next, fx] = reducePlan(s, msg);
      return then(next, fx, { name: "projectChain", project: msg.project, chain: msg.chain.chain.name });
    }
    case "cardCreated": {
      const [next, fx] = reducePlan(s, msg);
      const chain = next.chains[msg.detail.ticket.chain_id];
      return chain ? then(next, fx, cardRoute(next, chain, msg.detail.ticket)) : [next, fx];
    }
    case "projectArchived": {
      const [next, fx] = reducePlan(s, msg);
      // The project in focus went out of sight: the page goes home, to the next one.
      const gone = msg.project.archived && currentProject(s)?.id === msg.project.id;
      return gone ? then(next, fx, { name: "chains" }) : [next, fx];
    }
    case "openDialog":
    case "closeDialog":
    case "hashChanged":
    case "createChain":
    case "createCard":
    case "editCard":
    case "saveCard":
    case "pickPlace":
    case "moveCard":
    case "archiveProject":
    case "showArchived":
    case "cardSaved":
    case "cardMoved":
    case "planRejected":
      return reducePlan(s, msg);
    case "setUsageDays":
    case "setUsageMetric":
    case "setMachineScan":
    case "usageLoaded":
    case "usageFailed":
      return reduceUsage(s, msg);
  }
}

/// Moves to a route: the URL changes by an effect, never by the reducer itself. A card's edit or
/// move form stays with the screen it was opened on.
function go(s: State, route: Route): [State, Effect[]] {
  const plan = { ...s.plan, editing: undefined, moving: undefined };
  return [{ ...s, route, plan }, [{ type: "pushUrl", path: routePath(route) }, ...enter(route, s)]];
}

/// The effects of a command's answer, then the move it leads to.
function then(s: State, fx: Effect[], route: Route): [State, Effect[]] {
  const [moved, more] = go(s, route);
  return [moved, [...fx, ...more]];
}

/// What a screen needs fetched when it opens.
function enter(route: Route, s: State): Effect[] {
  if (route.name === "settings") return [...setupLoads, { type: "loadCli" }];
  if (route.name === "usage") return [loadUsage(s)];
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
  const chain = chainOfEvent(s, e);
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
