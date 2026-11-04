// The page's state machine: reduce(state, msg) -> [state, effects]. Pure: no DOM, no network.

import type { EventRecord } from "../api/types";
import type { Effect, Msg } from "./msg";
import { routePath } from "./route";
import { FIRST_RETRY_MS, MAX_RETRY_MS, type State, chainOfTicket } from "./state";

export function reduce(s: State, msg: Msg): [State, Effect[]] {
  switch (msg.type) {
    case "started":
      return [s, [{ type: "fetchTail" }]];
    case "journalTail":
      // The first load already has the state up to this event; the stream starts after it.
      return [{ ...s, lastSeq: msg.seq }, [{ type: "connect", since: msg.seq, delayMs: 0 }]];
    case "linkOpened":
      return [{ ...s, link: "live", retryMs: FIRST_RETRY_MS }, [{ type: "loadAll" }]];
    case "linkClosed":
      return [
        { ...s, link: "offline", retryMs: Math.min(s.retryMs * 2, MAX_RETRY_MS) },
        [{ type: "connect", since: s.lastSeq, delayMs: s.retryMs }],
      ];
    case "loaded": {
      const chains = Object.fromEntries(msg.chains.map((c) => [c.chain.id, c]));
      const next = { ...s, chains, chainOrder: msg.chains.map((c) => c.chain.id), loaded: true, error: "" };
      return [withTickets(next, msg.tickets), []];
    }
    case "chainLoaded": {
      const id = msg.chain.chain.id;
      const chainOrder = s.chainOrder.includes(id) ? s.chainOrder : [...s.chainOrder, id];
      return [{ ...s, chains: { ...s.chains, [id]: msg.chain }, chainOrder, error: "" }, []];
    }
    case "ticketsLoaded":
      return [withTickets(s, msg.tickets), []];
    case "loadFailed":
      return [{ ...s, error: msg.error }, []];
    case "event":
      return applyEvent(s, msg.event);
    case "toggleNeedsHuman":
      return [{ ...s, onlyHuman: msg.on }, []];
    case "navigate":
      return [{ ...s, route: msg.route }, [{ type: "pushUrl", path: routePath(msg.route) }]];
    case "urlChanged":
      return [{ ...s, route: msg.route }, []];
  }
}

function withTickets(s: State, tickets: { ticket: { id: string }; needs_human: boolean }[]): State {
  const needsHuman: Record<string, true> = {};
  for (const t of tickets) if (t.needs_human) needsHuman[t.ticket.id] = true;
  return { ...s, needsHuman };
}

/// A journal event refreshes what it touches: its chain and the inbox. A gap in `seq` or an
/// event the page cannot place reloads everything.
function applyEvent(s: State, e: EventRecord): [State, Effect[]] {
  if (e.seq <= s.lastSeq) return [s, []];
  const next = { ...s, lastSeq: e.seq };
  if (e.seq !== s.lastSeq + 1) return [next, [{ type: "loadAll" }]];
  if (e.entity === "project") return [next, []];
  const chain = chainOf(s, e);
  if (!chain) return [next, [{ type: "loadAll" }]];
  return [next, [{ type: "loadChain", id: chain }, { type: "loadTickets" }]];
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
    case "comment": {
      const ticket = str(p.comment?.ticket_id);
      return ticket && chainOfTicket(s, ticket);
    }
    case "subtask": {
      const ticket = str(p.subtask?.ticket_id);
      return ticket && chainOfTicket(s, ticket);
    }
    default:
      return undefined;
  }
}
