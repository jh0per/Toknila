// Runs the effects the reducer describes. The only place that touches the network or history.

import { get, post, streamUrl } from "../api/http";
import type { ChainDetail, ChainItem, EventRecord, Project, TicketDetail, TicketItem, TicketThreads } from "../api/types";
import type { CardCommand, Effect, Msg, ThreadAction } from "./msg";

type Dispatch = (msg: Msg) => void;

/// Coalesces bursts of refreshes (one command journals several events).
const DEBOUNCE_MS = 50;

const ACTION_PATH = { message: "messages", changes: "changes", approve: "approve" } as const;

function threadPath(e: ThreadAction): string {
  const ticket = encodeURIComponent(e.key);
  return `/tickets/${ticket}/threads/${encodeURIComponent(e.thread)}/${ACTION_PATH[e.action]}`;
}

/// A card command is a POST that answers with the card's new detail.
function cardRequest(e: CardCommand): [string, unknown] {
  const ticket = encodeURIComponent(e.key);
  return e.type === "ticketAction"
    ? [`/tickets/${ticket}/actions/${e.action}`, { text: e.text }]
    : [`/tickets/${ticket}/comments`, { kind: e.kind, text: e.text }];
}

export function effectRunner(dispatch: Dispatch): { run: (e: Effect) => void; stop: () => void } {
  let ws: WebSocket | null = null;
  let stopped = false;
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
  const failed = (e: unknown) =>
    dispatch({ type: "loadFailed", error: String(e instanceof Error ? e.message : e) });

  const loadAll = async () => {
    const projects = await get<Project[]>("/projects");
    const list = await get<ChainItem[]>("/chains");
    const chains = await Promise.all(list.map((c) => get<ChainDetail>(`/chains/${c.chain.id}`)));
    const tickets = await get<TicketItem[]>("/tickets");
    dispatch({ type: "loaded", projects, chains, tickets });
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
      case "ticketAction":
      case "addComment":
        void post<TicketDetail>(...cardRequest(e))
          .then((detail) => dispatch({ type: "detailLoaded", detail }))
          .catch(failed);
        return;
      case "loadChain":
        later(`chain:${e.id}`, () => {
          get<ChainDetail>(`/chains/${e.id}`)
            .then((chain) => dispatch({ type: "chainLoaded", chain }))
            .catch(failed);
        }, DEBOUNCE_MS);
        return;
      case "loadTickets":
        later("tickets", () => {
          get<TicketItem[]>("/tickets")
            .then((tickets) => dispatch({ type: "ticketsLoaded", tickets }))
            .catch(failed);
        }, DEBOUNCE_MS);
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
  };
  return { run, stop };
}
