// The page's one normalized store and the selectors the views read.

import type { ChainDetail } from "../api/types";
import { type Route } from "./route";

export type Link = "connecting" | "live" | "offline";

export interface State {
  /// Chains by id, in the order the daemon lists them.
  chains: Record<string, ChainDetail>;
  chainOrder: string[];
  /// Ids of tickets waiting on a person, as the daemon computes it.
  needsHuman: Record<string, true>;
  loaded: boolean;
  error: string;
  link: Link;
  /// Newest journal seq applied; the stream resumes after it.
  lastSeq: number;
  /// Delay before the next reconnect attempt, doubled on each failure.
  retryMs: number;
  onlyHuman: boolean;
  route: Route;
}

export const FIRST_RETRY_MS = 500;
export const MAX_RETRY_MS = 10_000;

export function initialState(route: Route): State {
  return {
    chains: {},
    chainOrder: [],
    needsHuman: {},
    loaded: false,
    error: "",
    link: "connecting",
    lastSeq: 0,
    retryMs: FIRST_RETRY_MS,
    onlyHuman: false,
    route,
  };
}

/// Chains the current route shows.
export function visibleChains(s: State): ChainDetail[] {
  const all = s.chainOrder.map((id) => s.chains[id]).filter((c): c is ChainDetail => !!c);
  return s.route.name === "chain" ? all.filter((c) => c.chain.name === (s.route as { chain: string }).chain) : all;
}

/// Whether only tickets waiting on a person are shown (the filter, or the inbox route).
export function humanOnly(s: State): boolean {
  return s.onlyHuman || s.route.name === "inbox";
}

/// The chain a ticket belongs to, if the store has it.
export function chainOfTicket(s: State, ticketId: string): string | undefined {
  return s.chainOrder.find((id) => s.chains[id]?.tickets.some((t) => t.id === ticketId));
}
