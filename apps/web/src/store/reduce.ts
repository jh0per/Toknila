// The page's state machine: reduce(state, msg) -> [state, effects]. Pure: no DOM, no network.

import type { EventRecord } from "../api/types";
import { firstImported, initialImports, reduceImports } from "./imports";
import type { Effect, Msg } from "./msg";
import { reduceAutostart } from "./autostart";
import { detailsWanted, knownKey } from "./cardDetails";
import { reduceTheme } from "./theme";
import { reduceEdges } from "./edges";
import { followTarget, opened } from "./follow";
import { reducePlacement } from "./placement";
import { reduceQuick } from "./quick";
import { type Dialog, reducePlan, withChain } from "./plan";
import { ONBOARDING_DRAFT, loadPlanner, reducePlanner } from "./planner";
import { reduceShortcuts } from "./shortcuts";
import { initialRelease, reduceRelease } from "./release";
import { dropChain, dropProject, initialRemoval, reduceRemoval, routeGone } from "./removal";
import { type Route, routePath } from "./route";
import { reduceSettings, setupLoads } from "./settings";
import { nextSettingsFrom } from "./settingsNav";
import { reduceUpdates } from "./updates";
import { loadUsage, reduceUsage } from "./usage";
import {
  FIRST_RETRY_MS, MAX_RETRY_MS, type State, activeProjects, cardRoute, chainOfEvent, currentProject, ticketOfEvent,
} from "./state";

export function reduce(s: State, msg: Msg): [State, Effect[]] {
  switch (msg.type) {
    case "started":
      return [s, [{ type: "fetchTail" }, { type: "watchUpdates" }]];
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
      return [next, [...enter(next.route, next), ...detailsWanted(next, msg.chains, true), ...(first ? setupLoads : [])]];
    }
    case "projectsLoaded":
      return [{ ...s, projects: msg.projects }, []];
    case "agentsLoaded":
      return [{ ...s, agents: msg.agents }, []];
    case "chainLoaded": {
      const next = { ...withChain(s, msg.chain), error: "" };
      return [next, detailsWanted(next, [msg.chain])];
    }
    case "suggestionsLoaded":
      return [{ ...s, suggestions: msg.suggestions }, []];
    case "inboxLoaded":
      return [{ ...s, inbox: msg.items }, []];
    case "detailLoaded": {
      const key = msg.detail.ticket.key;
      const peek = s.quick.peek?.key === key ? { key, fresh: true } : s.quick.peek;
      return [{ ...s, details: { ...s.details, [key]: msg.detail }, quick: { ...s.quick, peek } }, []];
    }
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
    case "selectCard":
      return go(s, { name: "card", project: msg.project, chain: msg.chain, key: msg.key });
    case "urlChanged":
      return [calm({ ...s, route: msg.route, opened: undefined, settingsFrom: nextSettingsFrom(s.route, msg.route, s.settingsFrom) }), enter(msg.route, s)];
    case "projectAdded": {
      const [added, fx] = reduceSettings(s, msg);
      const key = msg.project.key;
      // The new project's repository may hold plans; they are offered once the scan answers.
      const scan: Effect = { type: "loadPlans", project: key, reason: "added" };
      // The first project leads into the assistant; what was typed under "Describe your project"
      // is sent to it. A project added later goes on to its settings.
      if (activeProjects(s).length > 0) return then(withDialog(added, undefined), [...fx, scan], { name: "settings", project: key });
      const { [ONBOARDING_DRAFT]: described = "", ...rest } = added.planner.drafts;
      const kept = { ...added, planner: { ...added.planner, drafts: { ...rest, [key]: described } } };
      const [asked, ask] = reducePlanner(kept, { type: "askPlanner", project: key });
      return then(withDialog(asked, undefined), [...fx, ...(ask.length ? ask : [scan])], { name: "planner", project: key });
    }
    case "chainImported":
    case "recheck":
    case "installCli":
    case "cliLoaded":
    case "cliRejected":
    case "editDraft":
    case "submitProject":
    case "browseFolder":
    case "dismissSettingsNotes":
    case "repoChecked":
    case "saveProject":
    case "importChain":
    case "checksLoaded":
    case "profilesLoaded":
    case "placesLoaded":
    case "addRejected":
    case "projectSaved":
    case "settingsRejected":
      return reduceSettings(s, msg);
    case "chainCreated": {
      const [next, fx] = reducePlan(s, msg);
      return then(next, fx, { name: "projectChain", project: msg.project, chain: msg.chain.chain.name });
    }
    case "cardCreated": {
      const [planned, fx] = reducePlan(s, msg);
      const next = { ...planned, quick: { ...planned.quick, card: undefined } };
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
    case "closeDialog": {
      const [next, fx] = reducePlan(s, msg);
      // A dialog starts clean: no answer or refusal of the last one. The found plans go with a
      // new dialog or their own; closing another dialog leaves them where they show.
      const keep = msg.type === "closeDialog" && s.plan.dialog?.kind !== "foundPlans";
      const found = keep ? next.imports.found : undefined;
      return [{ ...next, imports: { ...initialImports, found }, removal: initialRemoval, release: initialRelease }, fx];
    }
    case "textImported":
    case "importAnswered": {
      const [next, fx] = reduceImports(s, msg);
      const chain = msg.type === "textImported" ? msg.chain : msg.result.chains[0]?.chain;
      return chain ? then(next, fx, { name: "projectChain", project: msg.project, chain: chain.chain.name }) : [next, fx];
    }
    case "plansImported": {
      const [next, fx] = reduceImports(s, msg);
      const f = next.imports.found;
      if (!f) return [next, fx];
      // The outcomes stay in sight: in the dialog they were picked in, else in the list's own
      // (picked in an empty project, whose page turns into the first imported chain).
      const shown = next.plan.dialog?.kind === "import" || next.plan.dialog?.kind === "foundPlans";
      const open = shown ? next : withDialog(next, { kind: "foundPlans" });
      const first = firstImported(f);
      return first ? then(open, fx, { name: "projectChain", project: f.project, chain: first }) : [open, fx];
    }
    case "importText":
    case "importPath":
    case "importRefused":
    case "scanPlans":
    case "plansScanned":
    case "scanFailed":
    case "togglePlan":
    case "importSelected":
    case "planImported":
    case "skipPlans":
      return reduceImports(s, msg);
    case "deleteProject":
    case "deleteChain":
    case "deleteRefused":
      return reduceRemoval(s, msg);
    case "chainDeleted": {
      const [next, fx] = reduceRemoval(s, msg);
      // The chain on screen is gone: back to its project.
      return routeGone(next) ? then(next, fx, { name: "project", project: msg.deletion.project }) : [next, fx];
    }
    case "openRelease":
    case "setReleaseVersion":
    case "startRelease":
    case "releaseOffered":
    case "releaseStarted":
    case "releaseRun":
    case "releaseRefused":
      return reduceRelease(s, msg);
    case "selectEdge":
    case "dismissEdgeError":
    case "addEdge":
    case "moveEdge":
    case "removeEdge":
    case "edgesChanged":
    case "edgeRefused":
      return reduceEdges(s, msg);
    case "placeCard":
    case "arrangeChain":
    case "sizeCard":
    case "cardPlaced":
    case "cardSized":
    case "cardPlaceRefused":
    case "chainArranged":
      return reducePlacement(s, msg);
    case "setAutostart":
    case "autostartSet":
    case "autostartRefused":
      return reduceAutostart(s, msg);
    case "openQuickCard":
    case "closeQuickCard":
    case "submitQuickCard":
    case "peekCard":
      return reduceQuick(s, msg);
    case "projectDeleted": {
      const [next, fx] = reduceRemoval(s, msg);
      return then(next, fx, { name: "chains" });
    }
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
    case "setTheme":
      return reduceTheme(s, msg);
    case "checkForUpdates":
    case "restartToUpdate":
    case "restartAnyway":
    case "updateLater":
    case "updateViewLoaded":
    case "updateWarned":
    case "updateFailed":
      return reduceUpdates(s, msg);
    case "askPlanner": {
      const [next, fx] = reducePlanner(s, msg);
      // The conversation is on the assistant's screen: a message sent from elsewhere goes there.
      const there = s.route.name === "planner" && s.route.project === msg.project;
      return fx.length && !there ? then(next, fx, { name: "planner", project: msg.project }) : [next, fx];
    }
    case "discussCard":
    case "startPlanner":
    case "stopPlanner":
    case "refreshPlanner":
    case "pickPlannerProfile":
    case "editPlannerDraft":
    case "plannerAsked":
    case "plannerLoaded":
    case "plannerFailed":
      return reducePlanner(s, msg);
    case "toggleShortcuts":
    case "closeShortcuts":
      return [reduceShortcuts(s, msg), []];
    case "backFromOpened":
      return s.opened ? go(s, s.opened.back) : [s, []];
    case "dismissOpened":
      return [{ ...s, opened: undefined }, []];
    case "openedExpired":
      return s.opened?.seq === msg.seq ? [{ ...s, opened: undefined }, []] : [s, []];
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
  const settingsFrom = nextSettingsFrom(s.route, route, s.settingsFrom);
  // The person going anywhere ends the assistant's notice; `applyWindow` puts its own back.
  return [{ ...calm({ ...s, route, plan, settingsFrom, opened: undefined }) }, [{ type: "pushUrl", path: routePath(route) }, ...enter(route, s)]];
}

/// A settings section starts without the last section's "Saved" or refusal.
function calm(s: State): State {
  if (s.route.name !== "settings") return s;
  return { ...s, settings: { ...s.settings, saved: false, error: undefined, problems: [], branches: [], imported: undefined } };
}

const withDialog = (s: State, dialog: Dialog | undefined): State => ({ ...s, plan: { ...s.plan, dialog } });

/// The effects of a command's answer, then the move it leads to.
function then(s: State, fx: Effect[], route: Route): [State, Effect[]] {
  const [moved, more] = go(s, route);
  return [moved, [...fx, ...more]];
}

/// What a screen needs fetched when it opens.
function enter(route: Route, s: State): Effect[] {
  if (route.name === "settings") return [...setupLoads, { type: "loadCli" }];
  if (route.name === "usage") return [loadUsage(s)];
  if (route.name === "planner") return [loadPlanner(route.project), { type: "loadProfiles" }];
  return route.name === "card"
    ? [{ type: "loadDetail", key: route.key }, { type: "loadThreads", key: route.key }]
    : [];
}

/// A journal event refreshes what it touches: its chain, the inbox, and the open card. A gap in
/// `seq` or an event the page cannot place reloads everything.
function applyEvent(s: State, e: EventRecord): [State, Effect[]] {
  if (e.seq <= s.lastSeq) return [s, []];
  const next = { ...s, lastSeq: e.seq, events: [...s.events, e] };
  if (e.entity === "window") return applyWindow(next, e, e.seq !== s.lastSeq + 1);
  if (e.seq !== s.lastSeq + 1) return [next, [{ type: "loadAll" }]];
  if (e.kind === "deleted" && (e.entity === "chain" || e.entity === "project")) {
    // A deleted project or chain is dropped; a page that showed it goes home.
    const dropped = e.entity === "chain" ? dropChain(next, e.entity_id) : dropProject(next, e.entity_id);
    const fx: Effect[] = [{ type: "loadInbox" }, { type: "loadAgents" }];
    return routeGone(dropped) ? then(dropped, fx, { name: "chains" }) : [dropped, fx];
  }
  if (e.entity === "project") return [next, [{ type: "loadProjects" }, { type: "loadAgents" }]];
  if (e.entity === "thread") return [next, threadEventEffects(s, ticketOfEvent(e))];
  const chain = chainOfEvent(s, e);
  if (!chain) return [next, [{ type: "loadAll" }]];
  const fx: Effect[] = [{ type: "loadChain", id: chain }, { type: "loadInbox" }, { type: "loadAgents" }];
  const ticket = ticketOfEvent(e);
  const open = s.route.name === "card" ? s.details[s.route.key]?.ticket : undefined;
  if (open && ticket === open.id) fx.push({ type: "loadDetail", key: open.key });
  // A card the diagram shows more of (subtasks, proposal buttons) keeps its detail fresh too.
  const known = knownKey(s, ticket);
  if (known && known !== open?.key) fx.push({ type: "loadDetail", key: known });
  return [next, fx];
}

/// A window event is about what this window shows, not about the plan: it is followed once (the
/// `seq` guard of `applyEvent` makes it once per event) or ignored. A gap in `seq` still reloads.
function applyWindow(s: State, e: EventRecord, gap: boolean): [State, Effect[]] {
  const fx: Effect[] = gap ? [{ type: "loadAll" }] : [];
  if (e.kind === "suggest" || e.kind === "suggestion_cleared") return [s, [...fx, { type: "loadInbox" }]];
  const target = followTarget(s, e);
  if (!target) return [s, fx];
  const [moved, more] = go(s, target);
  return [{ ...moved, opened: opened(s, target, e.seq) }, [...fx, ...more, { type: "expireOpened", seq: e.seq }]];
}

/// A thread event changes the card's threads and its phase; the hand-in gate may move its status.
function threadEventEffects(s: State, key: string | undefined): Effect[] {
  const fx: Effect[] = [{ type: "loadInbox" }, { type: "loadAgents" }];
  if (key && s.route.name === "card" && s.route.key === key) {
    fx.push({ type: "loadThreads", key }, { type: "loadDetail", key });
  }
  return fx;
}
