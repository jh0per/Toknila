// Runs the effects the reducer describes. The only place that touches the network or history.

import { get, post, streamUrl } from "../api/http";
import type {
  ChainDetail, ChainItem, EventRecord, InboxItem, Project, ProjectAgents, TicketDetail, TicketThreads,
} from "../api/types";
import type { CardCommand, Effect, Msg, ThreadAction } from "./msg";
import { runImportEffect } from "./importEffects";
import { runPlanEffect } from "./planEffects";
import { runSettingsEffect } from "./settingsEffects";
import { runUpdateEffect } from "./updatesEffects";
import { runUsageEffect } from "./usageEffects";

type Dispatch = (msg: Msg) => void;

/// Coalesces bursts of refreshes (one command journals several events).
const DEBOUNCE_MS = 50;

/// Waits for a pause in typing before the folder field asks for a suggestion.
const TYPING_MS = 250;

const ACTION_PATH = { message: "messages", changes: "changes", approve: "approve" } as const;

function threadPath(e: ThreadAction): string {
  const ticket = encodeURIComponent(e.key);
  return `/tickets/${ticket}/threads/${encodeURIComponent(e.thread)}/${ACTION_PATH[e.action]}`;
}

/// A card command is a POST that answers with the card's new detail.
function cardRequest(e: CardCommand): [string, unknown] {
  const ticket = encodeURIComponent(e.key);
  switch (e.type) {
    case "ticketAction": return [`/tickets/${ticket}/actions/${e.action}`, { text: e.text }];
    case "leftovers": return [`/tickets/${ticket}/leftovers/${e.action}`, {}];
    case "addComment": return [`/tickets/${ticket}/comments`, { kind: e.kind, text: e.text }];
    case "answer": return [`/tickets/${ticket}/runner/answer`, { role: e.role, answer: e.answer, text: e.text }];
  }
}

export function effectRunner(dispatch: Dispatch): { run: (e: Effect) => void; stop: () => void } {
  let ws: WebSocket | null = null;
  let stopped = false;
  let stopWatch: (() => void) | undefined;
  const timers = new Set<number>();
  const pending = new Map<string, number>();

  const later = (key: string, f: () => void, ms: number) => {
    if (pending.has(key)) return;
    const t = window.setTimeout(() => {
      pending.delete(key);
      timers.delete(t);
      f();
    }, ms);
    pending.set(key, t);
    timers.add(t);
  };
  /// Like `later`, but a new call replaces the waiting one, so the last arguments win.
  const debounce = (key: string, f: () => void, ms: number) => {
    const old = pending.get(key);
    if (old !== undefined) {
      clearTimeout(old);
      timers.delete(old);
      pending.delete(key);
    }
    later(key, f, ms);
  };
  const failed = (e: unknown) =>
    dispatch({ type: "loadFailed", error: String(e instanceof Error ? e.message : e) });

  const loadAll = async () => {
    const projects = await get<Project[]>("/projects");
    const list = await get<ChainItem[]>("/chains");
    const chains = await Promise.all(list.map((c) => get<ChainDetail>(`/chains/${c.chain.id}`)));
    dispatch({ type: "loaded", projects, chains });
    await loadInbox();
    dispatch({ type: "agentsLoaded", agents: await get<ProjectAgents[]>("/agents") });
  };

  /// The inbox of every project, newest first.
  const loadInbox = async () => {
    const projects = await get<Project[]>("/projects");
    const lists = await Promise.all(
      projects.map((p) => get<InboxItem[]>(`/projects/${encodeURIComponent(p.key)}/inbox`)),
    );
    const items = lists.flat().sort((a, b) => b.since - a.since);
    dispatch({ type: "inboxLoaded", items });
  };

  const loadThreads = async (key: string) => {
    const threads = await get<TicketThreads>(`/tickets/${encodeURIComponent(key)}/threads`);
    dispatch({ type: "threadsLoaded", threads });
  };

  const connect = (since: number) => {
    if (stopped) return;
    ws = new WebSocket(streamUrl(since));
    ws.onopen = () => dispatch({ type: "linkOpened" });
    ws.onmessage = (m) => {
      try {
        dispatch({ type: "event", event: JSON.parse(String(m.data)) as EventRecord });
      } catch {
        /* not an event */
      }
    };
    ws.onclose = () => {
      if (!stopped) dispatch({ type: "linkClosed" });
    };
  };

  const run = (e: Effect) => {
    switch (e.type) {
      case "fetchTail":
        get<EventRecord[]>("/events?since=0&limit=1000000")
          .then((events) =>
            dispatch({ type: "journalTail", seq: events.length ? events[events.length - 1].seq : 0, events }),
          )
          .catch(() => dispatch({ type: "journalTail", seq: 0, events: [] }));
        return;
      case "loadAll":
        later("all", () => void loadAll().catch(failed), DEBOUNCE_MS);
        return;
      case "loadProjects":
        later("projects", () => {
          get<Project[]>("/projects")
            .then((projects) => dispatch({ type: "projectsLoaded", projects }))
            .catch(failed);
        }, DEBOUNCE_MS);
        return;
      case "loadAgents":
        later("agents", () => {
          get<ProjectAgents[]>("/agents")
            .then((agents) => dispatch({ type: "agentsLoaded", agents }))
            .catch(failed);
        }, DEBOUNCE_MS);
        return;
      case "loadDetail":
        later(`detail:${e.key}`, () => {
          get<TicketDetail>(`/tickets/${encodeURIComponent(e.key)}`)
            .then((detail) => dispatch({ type: "detailLoaded", detail }))
            .catch(failed);
        }, DEBOUNCE_MS);
        return;
      case "loadThreads":
        later(`threads:${e.key}`, () => void loadThreads(e.key).catch(failed), DEBOUNCE_MS);
        return;
      case "threadAction":
        void post(threadPath(e), { text: e.text })
          .then(() => loadThreads(e.key))
          .catch(failed);
        return;
      case "answer":
        void post(`/tickets/${encodeURIComponent(e.key)}/runner/answer`, { role: e.role, answer: e.answer, text: e.text })
          .then(() => loadInbox())
          .catch(failed);
        return;
      case "ticketAction":
      case "leftovers":
      case "addComment":
        void post<TicketDetail>(...cardRequest(e))
          .then((detail) => dispatch({ type: "detailLoaded", detail }))
          .catch(failed);
        return;
      case "loadChecks":
      case "loadProfiles":
      case "loadCli":
      case "installCli":
      case "addProject":
      case "saveProject":
      case "importChain":
        runSettingsEffect(e, dispatch);
        return;
      case "checkRepo":
        debounce("checkRepo", () => runSettingsEffect(e, dispatch), TYPING_MS);
        return;
      case "createChain":
      case "createCard":
      case "saveCard":
      case "moveCard":
      case "archiveProject":
      case "clearHash":
        runPlanEffect(e, dispatch);
        return;
      case "importText":
      case "importPath":
      case "loadPlans":
      case "importPlans":
      case "deleteProject":
        runImportEffect(e, dispatch);
        return;
      case "watchUpdates":
        stopWatch = runUpdateEffect(e, dispatch);
        return;
      case "checkForUpdates":
      case "restartToUpdate":
        runUpdateEffect(e, dispatch);
        return;
      case "loadUsage":
      case "saveUsageSettings":
        if (e.type === "loadUsage") later("usage", () => runUsageEffect(e, dispatch), DEBOUNCE_MS);
        else runUsageEffect(e, dispatch);
        return;
      case "loadChain":
        later(`chain:${e.id}`, () => {
          get<ChainDetail>(`/chains/${e.id}`)
            .then((chain) => dispatch({ type: "chainLoaded", chain }))
            .catch(failed);
        }, DEBOUNCE_MS);
        return;
      case "loadInbox":
        later("inbox", () => void loadInbox().catch(failed), DEBOUNCE_MS);
        return;
      case "connect":
        if (e.delayMs === 0) connect(e.since);
        else later("connect", () => connect(e.since), e.delayMs);
        return;
      case "pushUrl":
        window.history.pushState(null, "", `${e.path}${window.location.search}`);
        return;
    }
  };

  const stop = () => {
    stopped = true;
    for (const t of timers) clearTimeout(t);
    ws?.close();
    stopWatch?.();
  };
  return { run, stop };
}
