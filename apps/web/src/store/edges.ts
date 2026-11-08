// Editing dependencies from the window: on the diagram (drag from a card's handle, select a line,
// move its end, remove it) and in the card's "Depends on" list. The daemon decides what is
// allowed (cycles, cards that have started); this keeps the selected line and the refusal it
// answered with, as text for the person, and never guesses the rule.

import type { ChainDetail, EdgeKind } from "../api/types";
import type { Effect } from "./msg";
import { withChain } from "./plan";
import type { State } from "./state";

/// A dependency of a chain: `from` comes first, `to` waits. Both are `nodeKey`s of the layout
/// (`ticket:<id>`, `release:<id>`).
export interface EdgeRef {
  chain: string;
  from: string;
  to: string;
}

export interface EdgesState {
  selected: EdgeRef | undefined;
  busy: boolean;
  /// The daemon's last refusal, as it said it.
  error: string | undefined;
}

export const initialEdges: EdgesState = { selected: undefined, busy: false, error: undefined };

/// The id inside a `nodeKey`.
export const idOf = (key: string): string => key.slice(key.indexOf(":") + 1);

/// What the person did. Nodes are `nodeKey`s.
export type EdgeIntent =
  | { type: "selectEdge"; edge: EdgeRef | undefined }
  | { type: "dismissEdgeError" }
  | { type: "addEdge"; chain: string; from: string; to: string }
  /// Moves one end of a dependency onto `target`: the dependency is added at its new place, then
  /// removed from the old one.
  | { type: "moveEdge"; chain: string; from: string; to: string; kind: EdgeKind; end: "from" | "to"; target: string }
  | { type: "removeEdge"; chain: string; from: string; to: string };

/// What the daemon answered.
export type EdgeResult =
  | { type: "edgesChanged"; chain: ChainDetail; selected: EdgeRef | undefined }
  | { type: "edgeRefused"; message: string };

export type EdgeMsg = EdgeIntent | EdgeResult;

export type EdgeEffect = Extract<EdgeIntent, { type: "addEdge" | "moveEdge" | "removeEdge" }>;

/// The reducer of dependency editing.
export function reduceEdges(s: State, msg: EdgeMsg): [State, Effect[]] {
  const set = (edges: Partial<EdgesState>, base: State = s): State => ({ ...base, edges: { ...base.edges, ...edges } });
  switch (msg.type) {
    case "selectEdge":
      return [set({ selected: msg.edge, error: undefined }), []];
    case "dismissEdgeError":
      return [set({ error: undefined }), []];
    case "addEdge":
    case "moveEdge":
    case "removeEdge":
      return [set({ busy: true, error: undefined }), [msg]];
    case "edgesChanged":
      return [set({ busy: false, error: undefined, selected: msg.selected }, withChain(s, msg.chain)), []];
    case "edgeRefused":
      return [set({ busy: false, error: msg.message }), []];
  }
}
