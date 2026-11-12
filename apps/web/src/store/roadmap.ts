// The roadmap of a project: its epics (chains) in order, as nodes of a sequence, and moving one.
// The order is the daemon's (`chain.position`); a move is a request the daemon judges.

import type { ChainDetail, ChainSummary, MoveChain } from "../api/types";
import type { Effect } from "./msg";
import { withChain } from "./plan";
import type { State } from "./state";

/// Where an epic stands: nothing started, work going on, every card done, or released.
export type RoadmapPhase = "planned" | "active" | "finished" | "released";

/// The phase of an epic as the roadmap says it, from the chain's computed summary.
export function roadmapPhase(s: Pick<ChainSummary, "phase" | "finish">): RoadmapPhase {
  switch (s.phase) {
    case "active":
    case "waiting":
      return "active";
    case "finished":
      return s.finish?.state === "released" ? "released" : "finished";
    case "idle":
    case "empty":
      return "planned";
  }
}

/// The project's chains in roadmap order. The daemon's list is already in order; sorting by
/// `position` keeps the page right while a move's answer and the reload are in flight.
export function roadmapOrder(chains: ChainDetail[]): ChainDetail[] {
  return chains
    .map((c, i) => ({ c, i }))
    .sort((a, b) => (a.c.chain.position ?? 0) - (b.c.chain.position ?? 0) || a.i - b.i)
    .map((x) => x.c);
}

/// The epics split for the screen: the finished ones fold into one group at the start, the rest
/// keep the roadmap order.
export function splitRoadmap(chains: ChainDetail[]): { finished: ChainDetail[]; rest: ChainDetail[] } {
  const ordered = roadmapOrder(chains);
  const done = (c: ChainDetail) => c.summary.phase === "finished";
  return { finished: ordered.filter(done), rest: ordered.filter((c) => !done(c)) };
}

/// The move that puts `id` one place earlier (`-1`) or later (`1`) among `shown`, the epics as
/// they stand on the screen; `undefined` at either end.
export function moveStep(shown: ChainDetail[], id: string, dir: -1 | 1): MoveChain | undefined {
  const i = shown.findIndex((c) => c.chain.id === id);
  const next = shown[i + dir];
  if (i < 0 || !next) return undefined;
  return dir < 0 ? { before: next.chain.id, after: null } : { before: null, after: next.chain.id };
}

/// The move of a drop: dragging `id` onto `over` puts it where `over` was, which is after it when
/// it came from before it and before it when it came from after.
export function dropMove(shown: ChainDetail[], id: string, over: string): MoveChain | undefined {
  const from = shown.findIndex((c) => c.chain.id === id);
  const to = shown.findIndex((c) => c.chain.id === over);
  if (from < 0 || to < 0 || from === to) return undefined;
  return from < to ? { before: null, after: over } : { before: over, after: null };
}

export interface RoadmapUi {
  /// The daemon's refusal of the last move.
  error: string | undefined;
}

export const initialRoadmap: RoadmapUi = { error: undefined };

export type RoadmapIntent = { type: "moveChain"; chain: string; project: string; to: MoveChain };

export type RoadmapResult =
  | { type: "chainMoved"; chain: ChainDetail }
  | { type: "chainMoveRefused"; message: string };

export type RoadmapMsg = RoadmapIntent | RoadmapResult;

export type RoadmapEffect = RoadmapIntent;

/// The reducer of the roadmap. A move changes the place of other chains too, so the answer is
/// followed by a reload of every chain; the journal would bring it as well.
export function reduceRoadmap(s: State, msg: RoadmapMsg): [State, Effect[]] {
  switch (msg.type) {
    case "moveChain":
      return [{ ...s, roadmap: { error: undefined } }, [msg]];
    case "chainMoved":
      return [{ ...withChain(s, msg.chain), roadmap: { error: undefined } }, [{ type: "loadAll" }]];
    case "chainMoveRefused":
      return [{ ...s, roadmap: { error: msg.message } }, []];
  }
}
