// Runs the effects the reducer describes. The only place that touches the network or history.

import { get, streamUrl } from "../api/http";
import type { ChainDetail, ChainItem, EventRecord, TicketItem } from "../api/types";
import type { Effect, Msg } from "./msg";

type Dispatch = (msg: Msg) => void;

/// Coalesces bursts of refreshes (one command journals several events).
const DEBOUNCE_MS = 50;

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
    const list = await get<ChainItem[]>("/chains");
    const chains = await Promise.all(list.map((c) => get<ChainDetail>(`/chains/${c.chain.id}`)));
    const tickets = await get<TicketItem[]>("/tickets");
    dispatch({ type: "loaded", chains, tickets });
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
          .then((evs) => dispatch({ type: "journalTail", seq: evs.length ? evs[evs.length - 1].seq : 0 }))
          .catch(() => dispatch({ type: "journalTail", seq: 0 }));
        return;
      case "loadAll":
        later("all", () => void loadAll().catch(failed), DEBOUNCE_MS);
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
